// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Test fixtures shared by the journal-figure modules.

use dioptra_core::api::{Command, Response, Session};
use serde_json::json;

/// Run a JSON command and return the reply, panicking on an error.
pub fn ok(s: &mut Session, cmd: serde_json::Value) -> serde_json::Value {
    let r: serde_json::Value = serde_json::from_str(&s.dispatch_json(&cmd.to_string())).unwrap();
    assert_ne!(r["type"], "error", "{cmd}: {r}");
    r
}

/// Turn Lab on (the test key on a day inside its term) or off (a day after it lapsed).
pub fn licensed(s: &mut Session, on: bool) {
    let today = if on { "2026-01-15" } else { "2026-10-14" };
    match s.dispatch(Command::SetLicense {
        key: Some(dioptra_license::TEST_KEY.into()),
        today: today.into(),
    }) {
        Response::License(v) => assert_eq!(v.lab_enabled, on),
        r => panic!("unexpected {r:?}"),
    }
}

/// A column table with three groups.
pub fn column_table(s: &mut Session) -> String {
    let t = ok(
        s,
        json!({"type": "create_table", "kind": "column", "title": "Plasma IL-6",
               "datasets": 3, "format": {"kind": "values", "n": 1}, "rows": 6}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let vals = [
        ["4.1", "6.2", "8.9"],
        ["3.8", "6.8", "9.4"],
        ["4.6", "5.9", "8.1"],
        ["4.0", "7.1", "9.9"],
        ["3.5", "6.5", "8.7"],
        ["4.4", "6.0", "9.2"],
    ];
    let values: Vec<Vec<String>> = vals
        .iter()
        .map(|r| r.iter().map(|v| v.to_string()).collect())
        .collect();
    ok(
        s,
        json!({"type": "set_cells", "table": t, "row0": 0, "col0": 0, "values": values}),
    );
    t
}

/// A session with a column table, two graphs of it and a layout of both.
pub fn project_with_graphs() -> (Session, Vec<String>, String) {
    let mut s = Session::new();
    let t = column_table(&mut s);
    let mut graphs = Vec::new();
    for family in ["column", "box"] {
        let r = ok(
            &mut s,
            json!({"type": "create_graph", "source": t, "family": family}),
        );
        graphs.push(r["changed"]["created"].as_str().unwrap().to_string());
    }
    let l = ok(
        &mut s,
        json!({"type": "create_layout", "graphs": graphs, "columns": 2}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    (s, graphs, l)
}
