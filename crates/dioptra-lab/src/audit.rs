// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The audit trail of a regulated project: `extensions/lab-audit.jsonl` in the project file,
//! one JSON entry per line, each holding the SHA-256 of the previous entry (`prev`) and of
//! its own canonical JSON (`hash`). Every entry also records the hash of the project content
//! after it, so a change made outside the trail shows when the file is verified.
//!
//! [`intercept`] is the hook in `dispatch_json`: on a regulated project every command that
//! changes the project needs a signed-in user and a reason (`lab_with_reason`), is refused
//! while the project is signed, and is appended to the trail. None of this needs a license:
//! a lapsed license never loosens the controls on a regulated record.

use dioptra_core::api::Session;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

use crate::sign;
use crate::users::UserView;

/// Name of the trail inside the project file (`extensions/<name>`).
pub const TRAIL: &str = "lab-audit.jsonl";
/// `prev` of the first entry.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// Start of the error for a change without a reason (the UI asks for one and retries).
pub const REASON_REQUIRED: &str = "A reason is required for every change to a regulated project.";
/// Start of the error for a change without a signed-in user (the UI offers to sign in).
pub const SIGN_IN_REQUIRED: &str = "Sign in to change a regulated project.";
/// Start of the error for a change to a signed project.
pub const LOCKED: &str = "This project is signed and locked.";

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Canonical JSON: object keys sorted, no whitespace.
pub fn canonical(v: &Json) -> String {
    let mut s = String::new();
    write_canonical(v, &mut s);
    s
}

fn write_canonical(v: &Json, out: &mut String) {
    match v {
        Json::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).expect("key"));
                out.push(':');
                write_canonical(&m[k], out);
            }
            out.push('}');
        }
        Json::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        _ => out.push_str(&serde_json::to_string(v).expect("scalar")),
    }
}

/// What an entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The audit trail was turned on.
    Regulated,
    /// A change to the project.
    Change,
    /// An electronic signature.
    Signature,
    /// A new version of a signed project.
    NewVersion,
}

/// The signature part of a `signature` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureData {
    /// `authored`, `reviewed` or `approved`.
    pub meaning: sign::Meaning,
    /// SHA-256 of the project content signed.
    pub content_hash: String,
    /// Hash of the last trail entry before the signature.
    pub audit_head: String,
}

/// One trail entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// 1, 2, 3, ...
    pub seq: u64,
    /// When (UTC).
    pub at: String,
    /// What it records.
    pub kind: EntryKind,
    /// User ID.
    pub user: String,
    /// Full name.
    pub name: String,
    /// What was done, in words.
    pub action: String,
    /// The command as JSON (bulky payloads summarised).
    #[serde(default, skip_serializing_if = "Json::is_null")]
    pub command: Json,
    /// Objects affected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<String>,
    /// Why.
    #[serde(default)]
    pub reason: String,
    /// Record version (1, then one more after each new version).
    pub version: u32,
    /// SHA-256 of the project content after this entry.
    pub content: String,
    /// The signature, for `signature` entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureData>,
    /// Hash of the previous entry.
    pub prev: String,
    /// SHA-256 of the canonical JSON of this entry without `hash`.
    pub hash: String,
}

/// Where the chain first fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Break {
    /// Line of the trail (1-based).
    pub line: usize,
    /// What is wrong.
    pub problem: String,
}

/// The trail as read from the project, checked line by line.
#[derive(Debug, Clone, Default)]
pub struct Trail {
    /// Entries that could be read, in file order.
    pub entries: Vec<AuditEntry>,
    /// First failure, if any.
    pub first_break: Option<Break>,
}

fn hash_of(v: &Json) -> String {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("hash");
    }
    sha256_hex(canonical(&v).as_bytes())
}

/// Whether the open project is regulated.
pub fn is_regulated(session: &Session) -> bool {
    session.extension(TRAIL).is_some_and(|b| !b.is_empty())
}

/// Read and check the trail.
pub fn read_trail(session: &Session) -> Trail {
    let mut t = Trail::default();
    let Some(bytes) = session.extension(TRAIL) else {
        return t;
    };
    let text = String::from_utf8_lossy(bytes);
    let mut prev = GENESIS.to_string();
    let fail = |t: &mut Trail, line: usize, problem: String| {
        if t.first_break.is_none() {
            t.first_break = Some(Break { line, problem });
        }
    };
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.trim().is_empty() {
            continue;
        }
        let v: Json = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                fail(&mut t, n, "the entry cannot be read".into());
                continue;
            }
        };
        let stored = v
            .get("hash")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        if hash_of(&v) != stored {
            fail(
                &mut t,
                n,
                "the entry was changed after it was written".into(),
            );
        }
        let e: AuditEntry = match serde_json::from_value(v) {
            Ok(e) => e,
            Err(_) => {
                fail(&mut t, n, "the entry cannot be read".into());
                continue;
            }
        };
        if e.prev != prev {
            fail(
                &mut t,
                n,
                "an entry before it is missing, or the entries are out of order".into(),
            );
        }
        let have = t.entries.len();
        if e.seq != have as u64 + 1 {
            fail(&mut t, n, format!("entry {} follows entry {have}", e.seq));
        }
        prev = stored;
        t.entries.push(e);
    }
    t
}

/// Version, lock and signature state derived from the entries.
pub fn version_of(entries: &[AuditEntry]) -> u32 {
    entries.last().map_or(1, |e| e.version)
}

/// Whether the latest signature or new version is a signature.
pub fn locked(entries: &[AuditEntry]) -> bool {
    entries
        .iter()
        .rev()
        .find(|e| matches!(e.kind, EntryKind::Signature | EntryKind::NewVersion))
        .is_some_and(|e| e.kind == EntryKind::Signature)
}

/// Append an entry to the trail (`seq`, `prev`, `version`, `content` and `hash` are filled
/// in here).
pub fn append(session: &mut Session, mut e: AuditEntry, version: Option<u32>) {
    let trail = read_trail(session);
    let last = trail.entries.last();
    e.seq = last.map_or(1, |l| l.seq + 1);
    e.prev = last.map_or_else(|| GENESIS.to_string(), |l| l.hash.clone());
    e.version = version.unwrap_or_else(|| version_of(&trail.entries));
    e.content = sign::content_hash(session);
    let mut v = serde_json::to_value(&e).expect("entry");
    let h = hash_of(&v);
    v["hash"] = Json::String(h);
    let mut bytes = session
        .extension(TRAIL)
        .map(<[u8]>::to_vec)
        .unwrap_or_default();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.extend(serde_json::to_string(&v).expect("entry").as_bytes());
    bytes.push(b'\n');
    session.set_extension(TRAIL, bytes);
}

/// A new entry for the signed-in user; `append` fills in the chain fields.
pub fn entry(user: &UserView, kind: EntryKind, action: String, reason: String) -> AuditEntry {
    AuditEntry {
        seq: 0,
        at: dioptra_core::util::now(),
        kind,
        user: user.id.clone(),
        name: user.name.clone(),
        action,
        command: Json::Null,
        objects: Vec::new(),
        reason,
        version: 1,
        content: String::new(),
        signature: None,
        prev: String::new(),
        hash: String::new(),
    }
}

/// Whether a command changes the project, by its type. Undo and redo are changes. Lab
/// commands are listed one by one; a Lab command missing from the list counts as a change.
pub fn is_change(kind: &str) -> bool {
    const CORE_READS: &[&str] = &[
        "get_", "list_", "render_", "export_", "save_", "open_", "default_",
    ];
    const CORE_EXACT: &[&str] = &[
        "ping",
        "new_project",
        "recompute_all",
        "set_license",
        "get_license",
        "inspect_plate",
    ];
    const LAB_READS: &[&str] = &[
        "lab_features",
        "lab_journal_presets",
        "lab_check_figure",
        "lab_export_tiff",
        "lab_methods",
        "lab_report",
        "lab_export_pzfx",
        "lab_batch_run",
        "lab_template_info",
        "lab_template",
        "lab_run_oq",
        "lab_oq_report",
        "lab_zip",
        "lab_verify_records",
        "lab_audit_report",
        "lab_audit_trail",
    ];
    if kind.starts_with("lab_") {
        return !LAB_READS.contains(&kind);
    }
    !(CORE_EXACT.contains(&kind) || CORE_READS.iter().any(|p| kind.starts_with(p)))
}

/// The command as JSON for the trail, with bulky payloads summarised (as the history does).
pub fn summarise(cmd: &Json) -> Json {
    let mut j = cmd.clone();
    if let Some(o) = j.as_object_mut() {
        for key in ["text", "base64"] {
            if let Some(Json::String(s)) = o.get(key) {
                if s.len() > 4096 {
                    let n = s.len();
                    o.insert(key.into(), json!({ "omitted_bytes": n }));
                }
            }
        }
        if let Some(Json::Array(v)) = o.get("values") {
            if v.len() > 200 {
                let n = v.len();
                o.insert("values".into(), json!({ "omitted_rows": n }));
            }
        }
        o.remove("password");
    }
    j
}

fn error(message: impl Into<String>) -> String {
    serde_json::to_string(&json!({ "type": "error", "message": message.into() }))
        .expect("error serialises")
}

/// The hook in `dispatch_json`. Returns the reply when this module handles the command.
pub fn intercept(session: &mut Session, json: &str) -> Option<String> {
    let v: Json = serde_json::from_str(json).ok()?;
    let kind = v.get("type")?.as_str()?.to_string();
    if sign::is_records_command(&kind) {
        return Some(sign::dispatch_json(session, json));
    }
    if kind == "lab_with_reason" {
        let reason = v
            .get("reason")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string();
        let Some(inner) = v.get("command").cloned() else {
            return Some(error("lab_with_reason needs a command."));
        };
        return Some(run_change(session, inner, Some(reason)));
    }
    if kind.starts_with("open_project") {
        let out = crate::dispatch_plain(session, json);
        return Some(after_open(session, out));
    }
    if is_regulated(session) && is_change(&kind) {
        return Some(run_change(session, v, None));
    }
    None
}

/// Run a change on a regulated project: check, run, append to the trail.
fn run_change(session: &mut Session, cmd: Json, reason: Option<String>) -> String {
    let kind = cmd
        .get("type")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_string();
    if kind == "lab_with_reason" || sign::is_records_command(&kind) {
        return error(format!("{kind} cannot be given a reason."));
    }
    let text = cmd.to_string();
    if !is_regulated(session) || !is_change(&kind) {
        return crate::dispatch_plain(session, &text);
    }
    let trail = read_trail(session);
    if locked(&trail.entries) {
        return error(format!(
            "{LOCKED} Start a new version (Records → New version…) to change it."
        ));
    }
    let Some(user) = crate::sign::current_user(session) else {
        return error(format!("{SIGN_IN_REQUIRED} (Records → Sign in…)"));
    };
    let reason = reason.map(|r| r.trim().to_string()).unwrap_or_default();
    if reason.is_empty() {
        return error(REASON_REQUIRED);
    }
    let before = session.history_log().len();
    let out = crate::dispatch_plain(session, &text);
    let reply: Json = serde_json::from_str(&out).unwrap_or(Json::Null);
    if reply.get("type").and_then(Json::as_str) == Some("error") {
        return out;
    }
    let labels: Vec<String> = session.history_log()[before.min(session.history_log().len())..]
        .iter()
        .map(|h| h.label.clone())
        .collect();
    let action = match labels.len() {
        0 => kind.replace('_', " "),
        1 => labels[0].clone(),
        n => format!("{} (and {} more steps)", labels[0], n - 1),
    };
    let mut objects: Vec<String> = Vec::new();
    if let Some(c) = reply.get("changed") {
        let ids = c
            .get("ids")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        for id in ids.iter().chain(c.get("created")) {
            if let Some(s) = id.as_str() {
                if !objects.iter().any(|o| o == s) {
                    objects.push(s.to_string());
                }
            }
        }
    }
    let mut e = entry(&user, EntryKind::Change, action, reason);
    e.command = summarise(&cmd);
    e.objects = objects;
    append(session, e, None);
    out
}

/// After opening a regulated project, check its trail and add a `warning` to the reply when
/// it is broken or the content differs from it.
fn after_open(session: &mut Session, out: String) -> String {
    if !is_regulated(session) {
        return out;
    }
    let mut reply: Json = match serde_json::from_str(&out) {
        Ok(v) => v,
        Err(_) => return out,
    };
    if reply.get("type").and_then(Json::as_str) == Some("error") {
        return out;
    }
    let status = sign::status(session);
    if let Some(w) = status.warning() {
        reply["warning"] = Json::String(w);
    }
    serde_json::to_string(&reply).expect("reply serialises")
}

#[cfg(test)]
mod change_tests {
    use super::is_change;

    #[test]
    fn reads_and_changes() {
        for k in [
            "get_cells",
            "render_graph",
            "export_table",
            "inspect_plate",
            "lab_check_figure",
            "lab_export_pzfx",
            "lab_batch_run",
            "lab_template",
            "lab_journal_presets",
        ] {
            assert!(!is_change(k), "{k}");
        }
        for k in [
            "set_cell",
            "paste",
            "undo",
            "redo",
            "replace_table_data",
            "import_bytes",
            "lab_apply_journal_preset",
            "lab_something_new",
        ] {
            assert!(is_change(k), "{k}");
        }
    }
}
