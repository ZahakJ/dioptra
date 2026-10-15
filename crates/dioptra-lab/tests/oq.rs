// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The automated OQ end to end through `dispatch_json`: every check passes, the comparisons of
//! each oracle family match the family table of docs/CORRECTNESS.md, all 58 NIST StRD sets
//! run, and the
//! report renders as CSV and as a PDF signed by the signed-in user.
#![cfg(not(target_arch = "wasm32"))]

use std::time::Instant;

use dioptra_core::api::{Command, Session};
use dioptra_lab::dispatch_json;
use dioptra_lab::oq::{self, OqReport};
use serde_json::{json, Value};

fn send(s: &mut Session, v: Value) -> Value {
    serde_json::from_str(&dispatch_json(s, &v.to_string())).unwrap()
}

#[test]
fn oq_runs_and_every_check_passes() {
    let mut s = Session::new();
    let r = send(&mut s, json!({"type": "lab_run_oq", "step": 0}));
    assert_eq!(r["type"], "error");
    assert!(
        r["message"].as_str().unwrap().contains("Dioptra Lab"),
        "{r}"
    );
    s.dispatch(Command::SetLicense {
        key: Some(dioptra_license::TEST_KEY.into()),
        today: "2026-01-15".into(),
    });
    let r = send(&mut s, json!({"type": "lab_oq_report", "format": "csv"}));
    assert!(
        r["message"]
            .as_str()
            .unwrap()
            .starts_with("Run the OQ first"),
        "{r}"
    );

    // Step by step, as the app runs it.
    let t0 = Instant::now();
    let n = oq::steps().len();
    let mut report = None;
    for i in 0..n {
        let r = send(&mut s, json!({"type": "lab_run_oq", "step": i}));
        assert_eq!(r["type"], "lab_oq", "{r}");
        assert_eq!(r["step"], i + 1);
        if i + 1 == n {
            report = Some(r["report"].clone());
        } else {
            assert!(r["report"].is_null());
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    let report: OqReport = serde_json::from_value(report.unwrap()).unwrap();
    println!(
        "OQ: {} checks in {:.1} s ({wall:.1} s with the dispatch round trips)",
        report.totals.checks, report.seconds
    );
    for sec in &report.sections {
        println!(
            "  {:12} {:5} checks {:6.2} s  {}",
            sec.code, sec.checks, sec.seconds, sec.title
        );
    }
    let failed: Vec<String> = report
        .checks
        .iter()
        .filter(|c| !c.pass)
        .take(40)
        .map(|c| format!("{} {} :: {}: {}", c.id, c.reference, c.quantity, c.note))
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
    assert!(report.pass);
    assert_eq!(report.totals.oracle_checks, oq::ORACLE_COMPARISONS);
    assert_eq!(report.totals.nist_checks, oq::NIST_CHECKS);
    assert_eq!(report.totals.nist_sets, oq::NIST_SETS);
    assert_eq!(report.totals.checks, 7841 + 371);
    // Family by family, the counts of docs/CORRECTNESS.md (same order).
    let doc = include_str!("../../../docs/CORRECTNESS.md");
    let table: Vec<usize> = doc
        .split("## Oracle fixtures")
        .nth(1)
        .and_then(|t| t.split("\n## ").next())
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Family"))
        .map(|l| l.split('|').nth(3).unwrap().trim().parse().unwrap())
        .collect();
    let ours: Vec<usize> = report
        .sections
        .iter()
        .filter(|s| s.code.starts_with("ORC-"))
        .map(|s| s.checks)
        .collect();
    assert_eq!(ours, table);
    let mut ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), report.checks.len(), "check IDs are unique");
    assert_eq!(report.report_sha256, oq::report_hash(&report));
    assert_eq!(report.fixtures_sha256, oq::fixture_set_sha256());

    // CSV: identification lines, a header, one row per check.
    let r = send(&mut s, json!({"type": "lab_oq_report", "format": "csv"}));
    assert_eq!(r["type"], "lab_file", "{r}");
    let b64 = r["base64"].as_str().unwrap();
    let text = String::from_utf8(dioptra_core::util::base64_decode(b64).unwrap()).unwrap();
    assert_eq!(
        text.lines().filter(|l| !l.starts_with('#')).count(),
        report.checks.len() + 1
    );
    assert!(text.contains(&report.report_sha256));

    // PDF: unsigned (manual block), then signed as executed by the signed-in user.
    let r = send(&mut s, json!({"type": "lab_oq_report", "format": "pdf"}));
    assert_eq!(r["type"], "lab_file", "{r}");
    send(&mut s, json!({"type": "lab_users_load"}));
    let r = send(
        &mut s,
        json!({"type": "lab_users_setup", "id": "ada", "name": "Ada Lovelace", "password": "analytical"}),
    );
    assert_eq!(r["type"], "lab_users", "{r}");
    let r = send(
        &mut s,
        json!({"type": "lab_oq_report", "format": "pdf", "password": "wrong-one"}),
    );
    assert_eq!(r["type"], "error", "{r}");
    let t1 = Instant::now();
    let r = send(
        &mut s,
        json!({"type": "lab_oq_report", "format": "pdf", "password": "analytical"}),
    );
    assert_eq!(r["type"], "lab_file", "{r}");
    let pdf = dioptra_core::util::base64_decode(r["base64"].as_str().unwrap()).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    if let Ok(path) = std::env::var("DIOPTRA_OQ_PDF") {
        std::fs::write(path, &pdf).unwrap();
    }
    println!(
        "signed PDF: {} bytes in {:.2} s",
        pdf.len(),
        t1.elapsed().as_secs_f64()
    );
}

#[test]
fn oq_is_not_a_change_to_a_regulated_project() {
    assert!(!dioptra_lab::audit::is_change("lab_run_oq"));
    assert!(!dioptra_lab::audit::is_change("lab_oq_report"));
}
