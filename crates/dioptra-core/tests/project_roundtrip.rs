//! A project with every table kind, formulas, excluded and text cells, analyses and graphs
//! survives save and open unchanged.

mod common;

use common::*;
use dioptra_core::api::Session;
use dioptra_core::file;
use serde_json::json;

#[test]
fn every_kind_round_trips() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    let kinds = [
        "xy",
        "column",
        "grouped",
        "contingency",
        "survival",
        "parts",
        "variables",
        "nested",
    ];
    let mut ids = Vec::new();
    for k in kinds {
        let fmt = if k == "grouped" {
            json!({"kind": "mean_sd_n"})
        } else {
            json!({"kind": "values", "n": 2})
        };
        let t = create(&mut s, k, 2, fmt, 0);
        ok(
            &mut s,
            json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "1\t2,5\t-3\n4\tn/a\t6\n7\tabc\t9.25", "options": {"decimal": "comma", "separator": "tab"}}),
        );
        ok(
            &mut s,
            json!({"type": "exclude_cells", "table": t, "range": {"row0": 1, "row1": 2, "col0": 0, "col1": 1}, "excluded": true}),
        );
        ok(
            &mut s,
            json!({"type": "set_row_title", "table": t, "row": 0, "title": "first"}),
        );
        ok(
            &mut s,
            json!({"type": "set_column_decimals", "table": t, "col": 0, "decimals": 2}),
        );
        ids.push(t);
    }
    let t0 = ids[1].clone();
    ok(
        &mut s,
        json!({"type": "add_dataset", "table": t0, "name": "Computed"}),
    );
    let meta = ok(&mut s, json!({"type": "get_table_meta", "table": t0}));
    let last = meta["columns"].as_array().unwrap().len() - 2;
    ok(
        &mut s,
        json!({"type": "set_column_formula", "table": t0, "col": last, "expr": "log10(A) + `Data set B`"}),
    );
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t0, "row": 0, "col": last + 1, "input": "=A1 * 2"}),
    );
    ok(
        &mut s,
        json!({"type": "set_variable_type", "table": ids[6], "dataset": 1, "var_type": "category"}),
    );
    ok(
        &mut s,
        json!({"type": "transform", "table": t0, "op": {"op": "sort", "col": 0, "descending": true}}),
    );
    let a = ok(&mut s, json!({"type": "create_analysis", "table": t0, "spec": {"kind": "descriptive", "percentiles": [10, 90]}}))["changed"]["created"].as_str().unwrap().to_string();
    ok(&mut s, json!({"type": "get_results", "id": a}));
    ok(
        &mut s,
        json!({"type": "create_analysis", "table": ids[3], "spec": {"kind": "chi_square", "yates": true}}),
    );
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": ids[0], "family": "scatter"}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        &mut s,
        json!({"type": "patch_graph", "id": g, "patch": {"annotations": [{"kind": "significance_bracket", "from": 0, "to": 1, "text": "**"}]}}),
    );
    ok(
        &mut s,
        json!({"type": "create_layout", "title": "Figure 1", "graphs": [g], "columns": 1}),
    );
    ok(
        &mut s,
        json!({"type": "set_p_value_style", "style": "nejm"}),
    );
    // Bring everything up to date before comparing.
    ok(&mut s, json!({"type": "recompute_all"}));

    let saved = ok(&mut s, json!({"type": "save_project_bytes"}));
    let bytes = dioptra_core::util::base64_decode(saved["base64"].as_str().unwrap()).unwrap();
    let (p2, log) = file::open_bytes(&bytes).unwrap();
    assert!(
        s.project().same_content(&p2),
        "project differs after round trip"
    );
    assert!(!log.is_empty());
    // Saving the opened project again gives the same table files byte for byte.
    let again = file::save_bytes(&p2, &log).unwrap();
    let names = |b: &[u8]| {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(b.to_vec())).unwrap();
        let mut out = Vec::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut s = String::new();
            std::io::Read::read_to_string(&mut f, &mut s).unwrap();
            if !name.starts_with("manifest") {
                out.push((name, s));
            }
        }
        out.sort();
        out
    };
    assert_eq!(names(&bytes), names(&again));
    // The files are what docs/FILE-FORMAT.md promises.
    let files = names(&bytes);
    let table = &files
        .iter()
        .find(|(n, _)| n == &format!("tables/{t0}.json"))
        .unwrap()
        .1;
    let v: serde_json::Value = serde_json::from_str(table).unwrap();
    assert_eq!(v["kind"], "column");
    assert_eq!(v["datasets"][0]["subcolumns"][0]["excluded"], json!([1]));
    assert_eq!(v["datasets"][0]["subcolumns"][1]["text"]["2"], "abc");
    assert_eq!(
        v["datasets"][0]["subcolumns"][1]["values"][1],
        serde_json::Value::Null
    );
    assert!(table.contains("\n  \"id\""), "pretty-printed");
    assert!(files.iter().any(|(n, _)| n == "history.jsonl"));
    assert!(files.iter().any(|(n, _)| n == &format!("results/{a}.json")));
}

#[test]
fn rejects_newer_major_format() {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    z.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    z.write_all(br#"{"format":"dioptra/2","app":"9","created":"","modified":""}"#)
        .unwrap();
    let bytes = z.finish().unwrap().into_inner();
    assert!(file::open_bytes(&bytes).unwrap_err().contains("newer"));
    let mut s = Session::new();
    let r = call(
        &mut s,
        json!({"type": "open_project_bytes", "base64": dioptra_core::util::base64_encode(&bytes), "name": null}),
    );
    assert_eq!(r["type"], "error");
}
