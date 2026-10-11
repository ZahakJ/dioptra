//! Undo and redo across every kind of mutation.

mod common;

use common::*;
use dioptra_core::api::Session;
use serde_json::{json, Value};

fn grid(s: &mut Session, t: &str) -> Vec<Vec<String>> {
    cells(s, t, 0, 1000, 0, 1000)
}

fn undo(s: &mut Session) -> Value {
    ok(s, json!({"type": "undo"}))
}

fn redo(s: &mut Session) -> Value {
    ok(s, json!({"type": "redo"}))
}

#[test]
fn edit_paste_formula_delete() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 2);
    let g0 = grid(&mut s, &t);

    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 0, "input": "1"}),
    );
    let g1 = grid(&mut s, &t);
    // A paste that grows the table and adds datasets.
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 1, "col0": 0, "text": "2\t20\n3\t30\n4\t40", "options": {}}),
    );
    let g2 = grid(&mut s, &t);
    assert_eq!(g2.len(), 4);
    ok(
        &mut s,
        json!({"type": "set_column_formula", "table": t, "col": 1, "expr": "A * 100"}),
    );
    let g3 = grid(&mut s, &t);
    assert_eq!(g3[3], vec!["4", "400"]);
    ok(
        &mut s,
        json!({"type": "delete_rows", "table": t, "at": 0, "count": 2}),
    );
    let g4 = grid(&mut s, &t);
    assert_eq!(g4, vec![vec!["3", "300"], vec!["4", "400"]]);
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 10, "col": 0, "input": "x"}),
    );
    let g5 = grid(&mut s, &t);
    assert_eq!(g5.len(), 11);

    for want in [&g4, &g3, &g2, &g1, &g0] {
        undo(&mut s);
        assert_eq!(&grid(&mut s, &t), want);
    }
    // create_table itself is undoable too: undo it, then redo it.
    undo(&mut s);
    assert!(call(&mut s, json!({"type": "get_table_meta", "table": t}))["type"] == "error");
    assert!(call(&mut s, json!({"type": "undo"}))["message"]
        .as_str()
        .unwrap()
        .contains("nothing to undo"));
    redo(&mut s);
    for want in [&g0, &g1, &g2, &g3, &g4, &g5] {
        assert_eq!(&grid(&mut s, &t), want);
        if want != &g5 {
            redo(&mut s);
        }
    }
    assert!(call(&mut s, json!({"type": "redo"}))["message"]
        .as_str()
        .unwrap()
        .contains("nothing to redo"));
    // A new edit clears redo.
    undo(&mut s);
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 0, "input": "9"}),
    );
    assert_eq!(call(&mut s, json!({"type": "redo"}))["type"], "error");
    let p = ok(&mut s, json!({"type": "get_project"}));
    assert_eq!(p["can_undo"], true);
    assert_eq!(p["can_redo"], false);
}

#[test]
fn objects_exclusion_and_settings() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 2, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\t2\n3\t4", "options": {}}),
    );
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": {"kind": "descriptive"}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "column"}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let before = ok(&mut s, json!({"type": "get_project"}));

    ok(
        &mut s,
        json!({"type": "exclude_cells", "table": t, "range": {"row0": 0, "row1": 2, "col0": 0, "col1": 1}, "excluded": true}),
    );
    let c = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 0, "row1": 2, "col0": 0, "col1": 1}}),
    );
    assert_eq!(c["rows"][1][0]["x"], true);
    undo(&mut s);
    let c = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 0, "row1": 2, "col0": 0, "col1": 1}}),
    );
    assert!(c["rows"][1][0].get("x").is_none());

    ok(
        &mut s,
        json!({"type": "rename_object", "id": t, "title": "Renamed"}),
    );
    ok(&mut s, json!({"type": "set_p_value_style", "style": "gp"}));
    ok(
        &mut s,
        json!({"type": "update_analysis", "id": a, "spec": {"kind": "descriptive", "ci_level": 0.99}}),
    );
    ok(
        &mut s,
        json!({"type": "patch_graph", "id": g, "patch": {"title": "Means"}}),
    );
    // Deleting the table removes the analysis and the graph that depend on it.
    let d = ok(&mut s, json!({"type": "delete_object", "id": t}));
    assert_eq!(d["changed"]["ids"].as_array().unwrap().len(), 3);
    assert!(ok(&mut s, json!({"type": "get_project"}))["items"]
        .as_array()
        .unwrap()
        .is_empty());
    undo(&mut s);
    let back = ok(&mut s, json!({"type": "get_project"}));
    assert_eq!(back["items"].as_array().unwrap().len(), 3);
    assert_eq!(back["items"][0]["title"], "Renamed");
    assert_eq!(back["p_value_style"], "gp");
    let spec = ok(&mut s, json!({"type": "get_analysis_spec", "id": a}));
    assert_eq!(spec["ci_level"], 0.99);
    for _ in 0..4 {
        undo(&mut s);
    }
    let after = ok(&mut s, json!({"type": "get_project"}));
    assert_eq!(after["items"], before["items"]);
    assert_eq!(after["p_value_style"], "apa");
    let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
    assert!(spec.get("title").is_none());
    // History lists undo and redo too.
    let h = ok(&mut s, json!({"type": "get_history", "limit": 3}));
    assert!(h["entries"][0]["label"]
        .as_str()
        .unwrap()
        .starts_with("Undo: "));
}

#[test]
fn large_paste_undo_uses_table_snapshot() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 0);
    let text: String = (0..300)
        .map(|r| {
            (0..300)
                .map(|c| format!("{}", r * 300 + c))
                .collect::<Vec<_>>()
                .join("\t")
                + "\n"
        })
        .collect();
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {}}),
    );
    assert_eq!(
        ok(&mut s, json!({"type": "get_table_meta", "table": t}))["rows"],
        300
    );
    // The audit log summarises the big text.
    let h = ok(&mut s, json!({"type": "get_history", "limit": 1}));
    assert!(
        h["entries"][0]["command"]["text"]["omitted_bytes"]
            .as_u64()
            .unwrap()
            > 4096
    );
    undo(&mut s);
    let m = ok(&mut s, json!({"type": "get_table_meta", "table": t}));
    assert_eq!(m["rows"], 0);
    assert_eq!(m["columns"].as_array().unwrap().len(), 1);
    redo(&mut s);
    assert_eq!(cells(&mut s, &t, 299, 300, 299, 300), vec![vec!["89999"]]);
}

#[test]
fn undo_is_capped() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 1);
    for i in 0..250 {
        ok(
            &mut s,
            json!({"type": "set_cell", "table": t, "row": 0, "col": 0, "input": i.to_string()}),
        );
    }
    let mut n = 0;
    while call(&mut s, json!({"type": "undo"}))["type"] != "error" {
        n += 1;
    }
    assert_eq!(n, 200);
    assert_eq!(cells(&mut s, &t, 0, 1, 0, 1), vec![vec!["49"]]);
}
