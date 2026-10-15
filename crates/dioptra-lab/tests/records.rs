// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Records for regulated labs, end to end through `dispatch_json`.

use dioptra_core::api::{Command, Session};
use dioptra_lab::dispatch_json;
use serde_json::{json, Value};

fn send(s: &mut Session, v: Value) -> Value {
    serde_json::from_str(&dispatch_json(s, &v.to_string())).unwrap()
}

fn ok(s: &mut Session, v: Value) -> Value {
    let r = send(s, v.clone());
    assert_ne!(r["type"], "error", "{v} -> {r}");
    r
}

fn err(s: &mut Session, v: Value) -> String {
    let r = send(s, v.clone());
    assert_eq!(r["type"], "error", "{v} -> {r}");
    r["message"].as_str().unwrap().to_string()
}

fn license(s: &mut Session, today: &str) {
    s.dispatch(Command::SetLicense {
        key: Some(dioptra_license::TEST_KEY.into()),
        today: today.into(),
    });
}

/// A licensed session with an admin (signed in), a reviewer, and a regulated project
/// holding one table.
fn regulated() -> (Session, String) {
    let mut s = Session::new();
    license(&mut s, "2026-01-15");
    ok(&mut s, json!({"type": "lab_users_load"}));
    ok(
        &mut s,
        json!({"type": "lab_users_setup", "id": "ada", "name": "Ada Lovelace", "password": "analytical"}),
    );
    ok(
        &mut s,
        json!({"type": "lab_user_add", "id": "rev", "name": "Rosalind Franklin", "role": "reviewer", "password": "photograph51"}),
    );
    let r = ok(
        &mut s,
        json!({"type": "create_table", "kind": "column", "title": "Plasma", "datasets": 2, "format": {"kind": "values", "n": 1}, "rows": 3}),
    );
    let table = r["changed"]["created"].as_str().unwrap().to_string();
    let r = ok(
        &mut s,
        json!({"type": "lab_make_regulated", "reason": "Study 12 under GLP"}),
    );
    assert_eq!(r["regulated"], true);
    (s, table)
}

fn set_cell(table: &str, input: &str) -> Value {
    json!({"type": "set_cell", "table": table, "row": 0, "col": 0, "input": input})
}

fn with_reason(cmd: Value, reason: &str) -> Value {
    json!({"type": "lab_with_reason", "reason": reason, "command": cmd})
}

fn trail(s: &Session) -> String {
    String::from_utf8(s.extension("lab-audit.jsonl").unwrap().to_vec()).unwrap()
}

fn verify(s: &mut Session) -> Value {
    ok(s, json!({"type": "lab_verify_records"}))
}

fn reopen(s: &mut Session, bytes_b64: String) {
    ok(
        s,
        json!({"type": "open_project_bytes", "base64": bytes_b64, "name": null}),
    );
}

fn saved(s: &mut Session) -> String {
    ok(s, json!({"type": "save_project_bytes"}))["base64"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn changes_need_a_reason_and_are_chained() {
    let (mut s, t) = regulated();
    assert!(err(&mut s, set_cell(&t, "1.5")).starts_with("A reason is required"));
    assert!(err(&mut s, with_reason(set_cell(&t, "1.5"), "  ")).starts_with("A reason is required"));
    ok(&mut s, with_reason(set_cell(&t, "1.5"), "Typo in entry"));
    // Undo is a change too.
    assert!(err(&mut s, json!({"type": "undo"})).starts_with("A reason"));
    ok(&mut s, with_reason(json!({"type": "undo"}), "Wrong cell"));
    // Reading needs nothing.
    ok(&mut s, json!({"type": "get_project"}));
    let lines: Vec<Value> = trail(&s)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1]["reason"], "Typo in entry");
    assert_eq!(lines[1]["user"], "ada");
    assert_eq!(lines[1]["name"], "Ada Lovelace");
    assert_eq!(lines[1]["objects"][0], t.as_str());
    assert_eq!(lines[1]["command"]["input"], "1.5");
    assert_eq!(lines[1]["prev"], lines[0]["hash"]);
    assert!(lines[2]["action"].as_str().unwrap().starts_with("Undo"));
    let v = verify(&mut s);
    assert_eq!(v["intact"], true);
    assert_eq!(v["content_matches"], true);
    assert_eq!(v["entries"], 3);
}

#[test]
fn signed_out_users_cannot_change() {
    let (mut s, t) = regulated();
    ok(&mut s, json!({"type": "lab_logout"}));
    assert!(err(&mut s, with_reason(set_cell(&t, "2"), "x")).starts_with("Sign in"));
    let e = err(
        &mut s,
        json!({"type": "lab_login", "user": "ada", "password": "wrong-one"}),
    );
    assert!(e.contains("4 attempts left"), "{e}");
    // The failure is in the users file to store.
    let r = ok(&mut s, json!({"type": "lab_users_list"}));
    assert_eq!(r["save"], true);
    ok(
        &mut s,
        json!({"type": "lab_login", "user": "ADA", "password": "analytical"}),
    );
    ok(&mut s, with_reason(set_cell(&t, "2"), "x"));
}

#[test]
fn lockout_through_the_engine() {
    let (mut s, _) = regulated();
    for _ in 0..5 {
        err(
            &mut s,
            json!({"type": "lab_login", "user": "rev", "password": "nope-nope"}),
        );
    }
    let e = err(
        &mut s,
        json!({"type": "lab_login", "user": "rev", "password": "photograph51"}),
    );
    assert!(e.contains("locked"), "{e}");
    // ada is still signed in; she is an admin and can reset it.
    ok(
        &mut s,
        json!({"type": "lab_user_reset_password", "id": "rev", "password": "doublehelix"}),
    );
    ok(
        &mut s,
        json!({"type": "lab_login", "user": "rev", "password": "doublehelix"}),
    );
    // Not an admin.
    let e = err(&mut s, json!({"type": "lab_user_disable", "id": "ada"}));
    assert!(e.contains("administrator"), "{e}");
}

#[test]
fn the_users_file_round_trips_through_the_shell() {
    let (mut s, _) = regulated();
    let r = ok(&mut s, json!({"type": "lab_users_list"}));
    let file = r["file"].as_str().unwrap().to_string();
    let mut s2 = Session::new();
    let r = ok(&mut s2, json!({"type": "lab_users_load", "file": file}));
    assert_eq!(r["users"].as_array().unwrap().len(), 2);
    assert!(
        !r["file"].as_str().unwrap().contains("analytical"),
        "no plain passwords"
    );
    let tampered = file.replace("\"reviewer\"", "\"admin\"");
    let e = err(&mut s2, json!({"type": "lab_users_load", "file": tampered}));
    assert!(e.contains("outside Dioptra"), "{e}");
}

#[test]
fn signing_locks_and_a_new_version_unlocks() {
    let (mut s, t) = regulated();
    ok(&mut s, with_reason(set_cell(&t, "3"), "Entry"));
    let e = err(
        &mut s,
        json!({"type": "lab_sign", "meaning": "authored", "password": "bad-password"}),
    );
    assert!(e.contains("attempts left"), "{e}");
    let r = ok(
        &mut s,
        json!({"type": "lab_sign", "meaning": "authored", "password": "analytical"}),
    );
    assert_eq!(r["locked"], true);
    assert_eq!(r["signatures"][0]["status"], "valid");
    assert_eq!(r["signatures"][0]["name"], "Ada Lovelace");
    let e = err(&mut s, with_reason(set_cell(&t, "4"), "Change"));
    assert!(e.starts_with("This project is signed"), "{e}");
    // A reviewer signs the same version.
    ok(
        &mut s,
        json!({"type": "lab_login", "user": "rev", "password": "photograph51"}),
    );
    ok(
        &mut s,
        json!({"type": "lab_sign", "meaning": "reviewed", "password": "photograph51"}),
    );
    assert!(err(&mut s, json!({"type": "lab_new_version", "reason": ""})).starts_with("A reason"));
    let r = ok(
        &mut s,
        json!({"type": "lab_new_version", "reason": "Reanalysis requested by QA"}),
    );
    assert_eq!(r["locked"], false);
    assert_eq!(r["version"], 2);
    assert_eq!(r["signatures"][0]["status"], "superseded");
    assert_eq!(r["signatures"][1]["status"], "superseded");
    ok(&mut s, with_reason(set_cell(&t, "4"), "Reanalysis"));
}

#[test]
fn analysts_sign_only_as_author() {
    let (mut s, _) = regulated();
    ok(
        &mut s,
        json!({"type": "lab_user_add", "id": "bob", "name": "Bob", "role": "analyst", "password": "benchwork"}),
    );
    ok(
        &mut s,
        json!({"type": "lab_login", "user": "bob", "password": "benchwork"}),
    );
    let e = err(
        &mut s,
        json!({"type": "lab_sign", "meaning": "approved", "password": "benchwork"}),
    );
    assert!(e.contains("reviewer"), "{e}");
}

#[test]
fn verification_and_locks_work_with_lab_lapsed() {
    let (mut s, t) = regulated();
    ok(
        &mut s,
        json!({"type": "lab_sign", "meaning": "authored", "password": "analytical"}),
    );
    license(&mut s, "2026-10-14");
    let v = verify(&mut s);
    assert_eq!(v["intact"], true);
    assert_eq!(v["signatures"][0]["status"], "valid");
    ok(&mut s, json!({"type": "lab_audit_trail"}));
    ok(&mut s, json!({"type": "lab_audit_report", "format": "csv"}));
    // Still locked; a lapsed license cannot start a new version.
    assert!(err(&mut s, with_reason(set_cell(&t, "9"), "x")).starts_with("This project is signed"));
    assert!(err(&mut s, json!({"type": "lab_new_version", "reason": "x"})).contains("Dioptra Lab"));
    assert!(err(&mut s, json!({"type": "lab_user_add", "id": "z", "name": "Z", "role": "analyst", "password": "zzzzzzzz"})).contains("Dioptra Lab"));
    // A fresh session without any license can verify a saved file.
    let b64 = saved(&mut s);
    let mut fresh = Session::new();
    reopen(&mut fresh, b64);
    let v = verify(&mut fresh);
    assert_eq!(v["regulated"], true);
    assert_eq!(v["intact"], true);
    assert_eq!(v["locked"], true);
    // Without users here, nobody can change it.
    assert!(
        err(&mut fresh, with_reason(set_cell(&t, "9"), "x")).starts_with("This project is signed")
    );
}

#[test]
fn regulating_and_signing_need_the_license() {
    let mut s = Session::new();
    license(&mut s, "2026-10-14");
    let e = err(
        &mut s,
        json!({"type": "lab_users_setup", "id": "a", "name": "A", "password": "aaaaaaaa"}),
    );
    assert!(e.contains("Dioptra Lab"), "{e}");
    assert!(err(&mut s, json!({"type": "lab_make_regulated"})).contains("Dioptra Lab"));
}

#[test]
fn save_and_open_keep_the_trail() {
    let (mut s, t) = regulated();
    ok(&mut s, with_reason(set_cell(&t, "=1+1"), "Formula"));
    ok(
        &mut s,
        with_reason(
            json!({"type": "set_column_formula", "table": t, "col": 1, "expr": "2*1"}),
            "Derived column",
        ),
    );
    let before = trail(&s);
    let b64 = saved(&mut s);
    let mut s2 = Session::new();
    let r = send(
        &mut s2,
        json!({"type": "open_project_bytes", "base64": b64, "name": null}),
    );
    assert!(r.get("warning").is_none(), "{r}");
    assert_eq!(trail(&s2), before);
    let v = verify(&mut s2);
    assert_eq!(v["intact"], true);
    assert_eq!(v["content_matches"], true, "{v}");
    // Still regulated after reopening: a bare change is refused.
    assert!(err(&mut s2, set_cell(&t, "5")).starts_with("Sign in"));
}

fn tampered(s: &mut Session, edit: impl Fn(Vec<String>) -> Vec<String>) -> Value {
    let lines: Vec<String> = trail(s).lines().map(str::to_string).collect();
    let mut text = edit(lines).join("\n");
    text.push('\n');
    s.set_extension("lab-audit.jsonl", text.into_bytes());
    let b64 = saved(s);
    let mut s2 = Session::new();
    send(
        &mut s2,
        json!({"type": "open_project_bytes", "base64": b64, "name": null}),
    )
}

#[test]
fn tampering_shows() {
    let (mut s, t) = regulated();
    for (i, v) in ["1", "2", "3"].iter().enumerate() {
        ok(&mut s, with_reason(set_cell(&t, v), &format!("step {i}")));
    }
    // One byte changed.
    let mut a = s.clone_for_test();
    let r = tampered(&mut a, |mut l| {
        l[2] = l[2].replacen("step 1", "step 7", 1);
        l
    });
    assert!(
        r["warning"].as_str().unwrap().contains("broken at entry 3"),
        "{r}"
    );
    // An entry deleted.
    let mut b = s.clone_for_test();
    let r = tampered(&mut b, |mut l| {
        l.remove(1);
        l
    });
    assert!(
        r["warning"].as_str().unwrap().contains("broken at entry 2"),
        "{r}"
    );
    // Two entries swapped.
    let mut c = s.clone_for_test();
    let r = tampered(&mut c, |mut l| {
        l.swap(1, 2);
        l
    });
    assert!(
        r["warning"].as_str().unwrap().contains("broken at entry 2"),
        "{r}"
    );
    // Data changed without the trail.
    let mut d = s.clone_for_test();
    d.dispatch(Command::SetCell {
        table: t.clone(),
        row: 1,
        col: 0,
        input: "99".into(),
    });
    let r = tampered(&mut d, |l| l);
    assert!(
        r["warning"].as_str().unwrap().contains("changed outside"),
        "{r}"
    );
    let v = verify(&mut d);
    assert_eq!(v["intact"], true);
    assert_eq!(v["content_matches"], false);
}

#[test]
fn audit_report_pdf() {
    let (mut s, t) = regulated();
    for i in 0..60 {
        ok(
            &mut s,
            with_reason(
                set_cell(&t, &i.to_string()),
                "Bulk entry from the notebook, page 4",
            ),
        );
    }
    ok(
        &mut s,
        json!({"type": "lab_sign", "meaning": "authored", "password": "analytical"}),
    );
    let r = ok(&mut s, json!({"type": "lab_audit_report", "format": "pdf"}));
    let pdf = dioptra_core::util::base64_decode(r["base64"].as_str().unwrap()).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let r = ok(&mut s, json!({"type": "lab_audit_report", "format": "csv"}));
    let csv = String::from_utf8(
        dioptra_core::util::base64_decode(r["base64"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(csv.lines().count(), 1 + 1 + 60 + 1);
}

trait CloneForTest {
    fn clone_for_test(&mut self) -> Session;
}
impl CloneForTest for Session {
    /// A copy of the project (through save and open) with the same users signed in.
    fn clone_for_test(&mut self) -> Session {
        let b64 = saved(self);
        let mut s = Session::new();
        reopen(&mut s, b64);
        s
    }
}
