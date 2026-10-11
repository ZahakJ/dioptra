//! End-to-end through `dispatch_json`, the way the UI drives the engine.

mod common;

use common::*;
use dioptra_core::api::Session;
use serde_json::json;

#[test]
fn ping() {
    let mut s = Session::new();
    let r = ok(&mut s, json!({"type": "ping"}));
    assert_eq!(r["type"], "pong");
    assert!(call(&mut s, json!({"type": "nonsense"}))["message"]
        .as_str()
        .unwrap()
        .starts_with("bad command"));
}

#[test]
fn table_paste_formula_analysis_save_open() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 0);
    // Paste three columns with a header, European decimals.
    let r = ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0,
        "text": "Control\tDrug\tNote\n1,5\t2,5\tok\n2,5\t3,5\tok\n3,5\t5,5\tredo\n", "options": {}}),
    );
    assert_eq!(r["type"], "paste_report");
    assert_eq!(r["decimal"], "comma");
    assert_eq!(r["header_detected"], true);
    assert_eq!(r["text_cells"], 3);
    assert_eq!(r["changed"]["tree"], true);
    let meta = ok(&mut s, json!({"type": "get_table_meta", "table": t}));
    assert_eq!(meta["rows"], 3);
    assert_eq!(meta["datasets"][1]["name"], "Drug");
    // A column formula in a fourth dataset.
    ok(
        &mut s,
        json!({"type": "add_dataset", "table": t, "name": "Ratio"}),
    );
    ok(
        &mut s,
        json!({"type": "set_column_formula", "table": t, "col": 3, "expr": "B / A"}),
    );
    assert_eq!(
        cells(&mut s, &t, 0, 3, 3, 4),
        vec![vec!["1.66667"], vec!["1.4"], vec!["1.57143"]]
    );
    // Edits propagate on the next read.
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "3"}),
    );
    assert_eq!(cells(&mut s, &t, 0, 1, 3, 4), vec![vec!["2"]]);
    // A cycle is refused at entry with the loop named.
    let m = err(
        &mut s,
        json!({"type": "set_column_formula", "table": t, "col": 1, "expr": "D * 2"}),
    );
    assert!(
        m.contains("Circular reference") && m.contains('B') && m.contains('D'),
        "{m}"
    );
    // Descriptive statistics.
    let r = ok(
        &mut s,
        json!({"type": "list_analyses", "table": t, "range": null}),
    );
    let desc = r["analyses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["kind"] == "descriptive")
        .unwrap()
        .clone();
    assert_eq!(desc["fits"], true);
    assert_eq!(desc["implemented"], true);
    let spec = ok(
        &mut s,
        json!({"type": "default_analysis_spec", "table": t, "kind": "descriptive"}),
    );
    assert_eq!(spec["kind"], "descriptive");
    let mut spec = spec.clone();
    spec.as_object_mut().unwrap().remove("type");
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": spec}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let res = ok(&mut s, json!({"type": "get_results", "id": a}));
    assert_eq!(res["type"], "results");
    let rows = res["sections"][0]["table"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "Control, Drug, Note and Ratio");
    assert_eq!(rows[0][2]["number"]["v"], 2.5);
    assert_eq!(
        rows[2][1]["number"]["v"], 0.0,
        "Note holds only text: n = 0"
    );
    assert!(res["assumptions"][0]["detail"]
        .as_str()
        .unwrap()
        .contains("Note: 3 text cells were not used"));
    let md = ok(
        &mut s,
        json!({"type": "export_results", "id": a, "format": "markdown"}),
    );
    assert!(md["text"]
        .as_str()
        .unwrap()
        .contains("| Data set | N | Mean"));
    // A graph renders through the fallback renderer.
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": t, "family": "column", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let svg = ok(&mut s, json!({"type": "render_graph", "id": g}));
    assert!(svg["svg"].as_str().unwrap().contains("<circle"));
    // Save, open in a new session, results are equal.
    let saved = ok(&mut s, json!({"type": "save_project_bytes"}));
    assert_eq!(saved["type"], "bytes");
    let b64 = saved["base64"].as_str().unwrap().to_string();
    let mut s2 = Session::new();
    ok(
        &mut s2,
        json!({"type": "open_project_bytes", "base64": b64, "name": "x.dioptra"}),
    );
    let res2 = ok(&mut s2, json!({"type": "get_results", "id": a}));
    assert_eq!(res, res2);
    assert_eq!(
        cells(&mut s2, &t, 0, 3, 0, 4),
        cells(&mut s, &t, 0, 3, 0, 4)
    );
    let tree = ok(&mut s2, json!({"type": "get_project"}));
    assert_eq!(tree["items"].as_array().unwrap().len(), 3);
    assert_eq!(tree["dirty"], false);
    // The audit log came along.
    let h = ok(&mut s2, json!({"type": "get_history", "limit": 100}));
    let entries = h["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e["label"].as_str().unwrap().starts_with("Paste 3 × 3")),
        "{h}"
    );
    assert_eq!(entries[0]["at"], "2026-10-11T09:00:00Z");
}

#[test]
fn results_follow_data_changes() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\n2\n3", "options": {}}),
    );
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": {"kind": "descriptive"}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let r = ok(&mut s, json!({"type": "get_results", "id": a}));
    assert_eq!(r["sections"][0]["table"]["rows"][0][2]["number"]["v"], 2.0);
    let c = ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 2, "col": 0, "input": "9"}),
    );
    assert!(
        c["changed"]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i == &json!(a)),
        "analysis listed as changed"
    );
    let tree = ok(&mut s, json!({"type": "get_project"}));
    assert_eq!(tree["items"][1]["stale"], true);
    let r = ok(&mut s, json!({"type": "get_results", "id": a}));
    assert_eq!(r["sections"][0]["table"]["rows"][0][2]["number"]["v"], 4.0);
    // Exclusion removes a value from the analysis.
    ok(
        &mut s,
        json!({"type": "exclude_cells", "table": t, "range": {"row0": 2, "row1": 3, "col0": 0, "col1": 1}, "excluded": true}),
    );
    let r = ok(&mut s, json!({"type": "get_results", "id": a}));
    assert_eq!(r["sections"][0]["table"]["rows"][0][2]["number"]["v"], 1.5);
    let cv = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 2, "row1": 3, "col0": 0, "col1": 1}}),
    );
    assert_eq!(cv["rows"][0][0]["x"], true);
}

#[test]
fn derived_tables_recompute_and_are_read_only() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 2, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\t10\n2\t20\n3\t30", "options": {}}),
    );
    let d = ok(&mut s, json!({"type": "transform", "table": t, "op": {"op": "filter_rows", "predicate": "A >= 2"}}))["changed"]["created"].as_str().unwrap().to_string();
    assert_eq!(
        cells(&mut s, &d, 0, 5, 0, 2),
        vec![vec!["2", "20"], vec!["3", "30"]]
    );
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 0, "input": "5"}),
    );
    assert_eq!(
        cells(&mut s, &d, 0, 5, 0, 2),
        vec![vec!["5", "10"], vec!["2", "20"], vec!["3", "30"]]
    );
    let m = err(
        &mut s,
        json!({"type": "set_cell", "table": d, "row": 0, "col": 0, "input": "1"}),
    );
    assert!(m.contains("read-only"), "{m}");
    let meta = ok(&mut s, json!({"type": "get_table_meta", "table": d}));
    assert_eq!(meta["derived_from"], json!(t));
    // Deleting the source deletes what was derived from it.
    let r = ok(&mut s, json!({"type": "delete_object", "id": t}));
    assert_eq!(r["changed"]["ids"].as_array().unwrap().len(), 2);
    assert_eq!(
        ok(&mut s, json!({"type": "get_project"}))["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn cells_viewport_clips_and_provenance() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    let t = create(&mut s, "xy", 1, json!({"kind": "values", "n": 2}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\t2\t3\n4\t5\t6", "options": {}}),
    );
    let r = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 1, "row1": 100, "col0": 1, "col1": 100}}),
    );
    assert_eq!(
        r["range"],
        json!({"row0": 1, "row1": 2, "col0": 1, "col1": 3})
    );
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 1, "col": 2, "input": "7"}),
    );
    let info = ok(
        &mut s,
        json!({"type": "get_cell_info", "table": t, "row": 1, "col": 2}),
    );
    assert_eq!(info["address"], "A.2[2]");
    let p = info["provenance"].as_str().unwrap();
    assert!(
        p.contains("Pasted from the clipboard at 2026-10-11T09:00:00Z"),
        "{p}"
    );
    assert!(p.contains("from 6 to 7"), "{p}");
    // Text typed into a numeric column is kept (P6).
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "<LOD"}),
    );
    let c = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 0, "row1": 1, "col0": 1, "col1": 2}}),
    );
    assert_eq!(c["rows"][0][0]["t"], "<LOD");
}

#[test]
fn cell_formulas_and_fill_down() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 2, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\n2\n3\n4", "options": {}}),
    );
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "=A1 * 10"}),
    );
    ok(
        &mut s,
        json!({"type": "fill_down", "table": t, "range": {"row0": 0, "row1": 4, "col0": 1, "col1": 2}}),
    );
    assert_eq!(
        cells(&mut s, &t, 0, 4, 1, 2),
        vec![vec!["10"], vec!["20"], vec!["30"], vec!["40"]]
    );
    let info = ok(
        &mut s,
        json!({"type": "get_cell_info", "table": t, "row": 3, "col": 1}),
    );
    assert_eq!(info["cell_formula"], "A4 * 10");
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 1, "col": 1, "input": "=B1 + 1"}),
    );
    let m = err(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "=B2"}),
    );
    assert!(m.contains("Circular"), "{m}");
    // The refused formula left the old one in place.
    assert_eq!(cells(&mut s, &t, 0, 2, 1, 2), vec![vec!["10"], vec!["11"]]);
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "=sqrt(-1)"}),
    );
    let c = ok(
        &mut s,
        json!({"type": "get_cells", "table": t, "range": {"row0": 0, "row1": 1, "col0": 1, "col1": 2}}),
    );
    assert!(c["rows"][0][0]["e"].as_str().unwrap().contains("negative"));
}

#[test]
fn imports_from_bytes() {
    let mut s = Session::new();
    let csv = "subject,group,score\n1,ctrl,1.5\n2,drug,2.5\n";
    let b64 = dioptra_core::util::base64_encode(csv.as_bytes());
    let r = ok(
        &mut s,
        json!({"type": "import_bytes", "name": "trial.csv", "base64": b64, "kind": null}),
    );
    let id = r["changed"]["created"].as_str().unwrap().to_string();
    let meta = ok(&mut s, json!({"type": "get_table_meta", "table": id}));
    assert_eq!(meta["kind"], "variables");
    assert_eq!(meta["title"], "trial");
    assert_eq!(meta["datasets"][1]["var_type"], "category");
    assert_eq!(meta["columns"][2]["source"]["kind"], "imported");
    // Export as xlsx and import it back.
    let x = ok(
        &mut s,
        json!({"type": "export_table", "table": id, "format": "xlsx"}),
    );
    let r = ok(
        &mut s,
        json!({"type": "import_bytes", "name": "back.xlsx", "base64": x["base64"], "kind": "variables"}),
    );
    let id2 = r["changed"]["created"].as_str().unwrap().to_string();
    assert_eq!(
        cells(&mut s, &id2, 0, 2, 0, 3),
        cells(&mut s, &id, 0, 2, 0, 3)
    );
    let t = ok(
        &mut s,
        json!({"type": "import_text", "text": "x\ty\n1\t2", "kind": "xy", "title": "Curve", "options": {}}),
    );
    assert_eq!(t["type"], "paste_report");
    let csvout = ok(
        &mut s,
        json!({"type": "export_table", "table": t["changed"]["created"], "format": "csv"}),
    );
    let text = String::from_utf8(
        dioptra_core::util::base64_decode(csvout["base64"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(text, "x,y\r\n1,2\r\n");
}

#[test]
fn graphs_patch_and_layouts() {
    let mut s = Session::new();
    let t = create(&mut s, "xy", 1, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\t2\n2\t4", "options": {}}),
    );
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": t, "family": "scatter"}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        &mut s,
        json!({"type": "patch_graph", "id": g, "patch": {"style": {"size_mm": {"width": 174.0}}, "axes": {"y": {"scale": "log10"}}}}),
    );
    let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
    assert_eq!(spec["style"]["size_mm"]["width"], 174.0);
    assert_eq!(spec["style"]["size_mm"]["height"], 65.0);
    assert_eq!(spec["axes"]["y"]["scale"], "log10");
    let r = ok(&mut s, json!({"type": "render_graph", "id": g}));
    assert_eq!(r["width_mm"], 174.0);
    let l = ok(
        &mut s,
        json!({"type": "create_layout", "title": null, "graphs": [g, g], "columns": 2}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let svg = ok(&mut s, json!({"type": "render_layout", "id": l}));
    assert_eq!(svg["svg"].as_str().unwrap().matches("<circle").count(), 4);
    let e = err(
        &mut s,
        json!({"type": "export_graph", "id": g, "format": {"format": "pdf"}, "size": null}),
    );
    assert!(e.contains("SVG"));
    let b = ok(
        &mut s,
        json!({"type": "export_graph", "id": g, "format": {"format": "svg"}, "size": null}),
    );
    assert_eq!(b["mime"], "image/svg+xml");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn save_and_open_by_path() {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let path = dir.path().join("p.dioptra").to_string_lossy().to_string();
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 0, "input": "42"}),
    );
    ok(&mut s, json!({"type": "save_project_path", "path": path}));
    let mut s2 = Session::new();
    ok(&mut s2, json!({"type": "open_project_path", "path": path}));
    assert_eq!(cells(&mut s2, &t, 0, 1, 0, 1), vec![vec!["42"]]);
    let v = ok(&mut s2, json!({"type": "get_project"}));
    assert_eq!(v["path"], json!(path));
}
