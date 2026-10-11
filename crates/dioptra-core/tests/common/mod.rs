//! Helpers shared by the integration tests.
#![allow(dead_code)]

use dioptra_core::api::Session;
use serde_json::{json, Value};

/// Send one command as JSON and parse the reply.
pub fn call(s: &mut Session, cmd: Value) -> Value {
    let out = s.dispatch_json(&cmd.to_string());
    serde_json::from_str(&out).expect("valid JSON reply")
}

/// Send a command that must succeed.
pub fn ok(s: &mut Session, cmd: Value) -> Value {
    let r = call(s, cmd.clone());
    assert_ne!(r["type"], "error", "command {cmd} failed: {r}");
    r
}

/// Send a command that must fail; returns the message.
pub fn err(s: &mut Session, cmd: Value) -> String {
    let r = call(s, cmd.clone());
    assert_eq!(r["type"], "error", "command {cmd} should fail: {r}");
    r["message"].as_str().unwrap().to_string()
}

/// Create a table and return its id.
pub fn create(s: &mut Session, kind: &str, datasets: usize, format: Value, rows: usize) -> String {
    let r = ok(
        s,
        json!({"type": "create_table", "kind": kind, "title": "", "datasets": datasets, "format": format, "rows": rows}),
    );
    r["changed"]["created"].as_str().unwrap().to_string()
}

/// Cell display strings of a range.
pub fn cells(
    s: &mut Session,
    table: &str,
    row0: usize,
    row1: usize,
    col0: usize,
    col1: usize,
) -> Vec<Vec<String>> {
    let r = ok(
        s,
        json!({"type": "get_cells", "table": table, "range": {"row0": row0, "row1": row1, "col0": col0, "col1": col1}}),
    );
    r["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|c| c["d"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .collect()
}

/// Fixed clock so files and logs are deterministic.
pub fn fixed_clock(s: &mut Session) {
    s.set_clock(Some(Box::new(|| "2026-10-11T09:00:00Z".to_string())));
}
