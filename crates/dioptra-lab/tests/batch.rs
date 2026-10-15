// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Batch analysis: a nonlinear-fit template applied to three files, and license gating.

use dioptra_core::api::Session;
use dioptra_core::util::{base64_decode, base64_encode};
use serde_json::{json, Value};

fn call(s: &mut Session, v: Value) -> Value {
    let out: Value = serde_json::from_str(&dioptra_lab::dispatch_json(s, &v.to_string())).unwrap();
    assert_ne!(out["type"], "error", "{v}: {out}");
    out
}

fn logistic(x: f64, log_ec50: f64) -> f64 {
    10.0 + 90.0 / (1.0 + 10f64.powf(log_ec50 - x))
}

/// A dose-response CSV with duplicates; tiny fixed offsets keep the fit honest.
fn csv(log_ec50: f64) -> String {
    let mut s = String::from("log dose,rep 1,rep 2\n");
    for i in 0..11 {
        let x = -9.0 + 0.5 * i as f64;
        let y = logistic(x, log_ec50);
        s.push_str(&format!("{x},{},{}\n", y + 0.4, y - 0.4));
    }
    s
}

/// A template: an XY table with a four-parameter fit and a graph of it.
fn template() -> String {
    let mut s = Session::new();
    let t = call(
        &mut s,
        json!({"type": "create_table", "kind": "xy", "title": "Dose response", "datasets": 1,
               "format": {"kind": "values", "n": 2}, "rows": 0}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let text = csv(-7.0)
        .lines()
        .skip(1)
        .map(|l| l.replace(',', "\t"))
        .collect::<Vec<_>>()
        .join("\n");
    call(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {}}),
    );
    let a = call(
        &mut s,
        json!({"type": "create_analysis", "table": t,
               "spec": {"kind": "nonlinear_regression", "equation": {"name": "four_parameter_logistic"}}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut s,
        json!({"type": "rename_object", "id": a, "title": "Fit"}),
    );
    call(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "scatter", "title": "Curve"}),
    );
    call(&mut s, json!({"type": "save_project_bytes"}))["base64"]
        .as_str()
        .unwrap()
        .to_string()
}

fn license(s: &mut Session, today: &str) {
    call(
        s,
        json!({"type": "set_license", "key": dioptra_license::TEST_KEY, "today": today}),
    );
}

fn run(s: &mut Session, template: &str) -> Value {
    let inputs: Vec<Value> = [-7.0, -6.5, -6.0]
        .iter()
        .enumerate()
        .map(|(i, e)| json!({"name": format!("plate{}.csv", i + 1), "base64": base64_encode(csv(*e).as_bytes())}))
        .collect();
    let cmd = json!({"type": "lab_batch_run", "template_base64": template, "table": "Dose response",
                     "inputs": inputs, "options": {"format": "svg"}});
    serde_json::from_str(&dioptra_lab::dispatch_json(s, &cmd.to_string())).unwrap()
}

#[test]
fn batch_over_three_files_needs_lab_and_recomputes_each() {
    let template = template();
    let mut s = Session::new();
    // No key, then a lapsed key: refused with the Lab message.
    let r = run(&mut s, &template);
    assert_eq!(r["type"], "error");
    assert!(r["message"]
        .as_str()
        .unwrap()
        .contains("Batch analysis is part of Dioptra Lab"));
    license(&mut s, "2026-10-14");
    assert!(run(&mut s, &template)["message"]
        .as_str()
        .unwrap()
        .contains("part of Dioptra Lab"));

    license(&mut s, "2026-01-15");
    let r = run(&mut s, &template);
    assert_eq!(r["type"], "lab_batch", "{r}");
    let items = r["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    for (item, truth) in items.iter().zip([-7.0, -6.5, -6.0]) {
        assert!(item.get("error").is_none(), "{item}");
        let h = item["headlines"].as_array().unwrap();
        let get = |name: &str| {
            h.iter()
                .find(|x| x["name"] == name)
                .unwrap_or_else(|| panic!("no {name} in {h:?}"))["value"]
                .as_f64()
                .unwrap()
        };
        assert!((get("Fit · Data set A · LogEC50") - truth).abs() < 1e-6);
        assert!((get("Fit · Data set A · Top") - 100.0).abs() < 1e-6);
        assert!((get("Fit · Data set A · Bottom") - 10.0).abs() < 1e-6);
        assert!((get("Fit · Data set A · EC50") / 10f64.powf(truth) - 1.0).abs() < 1e-6);
        let files = item["files"].as_array().unwrap();
        let names: Vec<&str> = files.iter().map(|f| f["name"].as_str().unwrap()).collect();
        let stem = item["name"].as_str().unwrap().trim_end_matches(".csv");
        assert_eq!(
            names,
            [format!("{stem}.dioptra"), format!("{stem} - Curve.svg")]
        );
        let svg = base64_decode(files[1]["base64"].as_str().unwrap()).unwrap();
        assert!(String::from_utf8(svg).unwrap().contains("<svg"));
    }
    // The recomputed project opens with the new data.
    let proj = items[2]["files"][0]["base64"].as_str().unwrap();
    let mut o = Session::new();
    call(
        &mut o,
        json!({"type": "open_project_bytes", "base64": proj, "name": null}),
    );
    let p = o.project();
    let t = p.tables.values().next().unwrap();
    assert_eq!(t.title, "Dose response");
    assert_eq!(
        t.datasets[0].subcolumns[0].values[0],
        logistic(-9.0, -6.0) + 0.4
    );
    // The summary: one row per file, File first and Error last.
    let cols: Vec<&str> = r["summary"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(cols[0], "File");
    assert_eq!(*cols.last().unwrap(), "Error");
    let k = cols
        .iter()
        .position(|c| *c == "Fit · Data set A · LogEC50")
        .unwrap();
    let rows = r["summary"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1][0], "plate2.csv");
    assert!((rows[1][k].as_f64().unwrap() + 6.5).abs() < 1e-6);
}

#[test]
fn errors_are_reported_per_file() {
    let template = template();
    let t = base64_decode(&template).unwrap();
    let opts = dioptra_lab::batch::BatchOptions::default();
    let bad = dioptra_lab::batch::run_one(&t, "Dose response", "empty.csv", b"", &opts);
    assert!(bad.error.is_some() && bad.files.is_empty());
    let none =
        dioptra_lab::batch::run_one(&t, "No such table", "a.csv", csv(-7.0).as_bytes(), &opts);
    assert!(none.error.unwrap().contains("no table called"));
    let good =
        dioptra_lab::batch::run_one(&t, "dose RESPONSE", "a.csv", csv(-7.0).as_bytes(), &opts);
    assert!(good.error.is_none());
    assert!(good.files[1].name.ends_with(".pdf"));
    let s = dioptra_lab::batch::summarise(&[good, bad]);
    let text = dioptra_lab::batch::summary_csv(&s);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("File,"));
    assert!(lines[2].starts_with("empty.csv,"));
}
