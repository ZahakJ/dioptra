//! The `dioptra` binary end to end: each subcommand on real files, and its errors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dioptra_core::api::Session;
use serde_json::{json, Value};

fn dioptra(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dioptra"))
        .args(args)
        .output()
        .expect("run dioptra")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn ok(o: Output) -> String {
    assert!(
        o.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        o.status.code(),
        stdout(&o),
        stderr(&o)
    );
    stdout(&o)
}

fn fails(o: Output) -> String {
    assert!(
        !o.status.success(),
        "expected failure; stdout:\n{}",
        stdout(&o)
    );
    assert_eq!(o.status.code(), Some(1));
    stderr(&o)
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("temp dir")
}

fn p(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

const GROUPS_CSV: &str =
    "Control,Low dose,High dose\n4.1,5.2,7.9\n3.8,5.6,8.4\n4.4,4.9,7.6\n4.0,5.3,8.8\n3.6,5.8,8.1\n";

fn call(s: &mut Session, cmd: Value) -> Value {
    let v: Value = serde_json::from_str(&s.dispatch_json(&cmd.to_string())).unwrap();
    assert_ne!(v["type"], "error", "{cmd}: {v}");
    v
}

/// A project with a column table, a one-way ANOVA, a graph of it and a layout.
fn project(dir: &Path) -> PathBuf {
    let mut ses = Session::new();
    let t = call(
        &mut ses,
        json!({"type": "import_text", "text": GROUPS_CSV, "kind": "column", "title": "Plasma IL-6", "options": {}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let spec = {
        let mut v = call(
            &mut ses,
            json!({"type": "default_analysis_spec", "table": t, "kind": "one_way_anova"}),
        );
        v.as_object_mut().unwrap().remove("type");
        v
    };
    let a = call(
        &mut ses,
        json!({"type": "create_analysis", "table": t, "spec": spec}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let g = call(
        &mut ses,
        json!({"type": "create_graph", "source": a, "family": "", "title": "IL-6 by dose"}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &mut ses,
        json!({"type": "create_layout", "title": "Figure 1", "graphs": [g], "columns": 1}),
    );
    let path = p(dir, "study.dioptra");
    call(
        &mut ses,
        json!({"type": "save_project_path", "path": s(&path)}),
    );
    path
}

#[test]
fn info_lists_the_tree() {
    let d = tmp();
    let f = project(d.path());
    let out = ok(dioptra(&["info", s(&f)]));
    assert!(
        out.contains("1 tables, 1 analyses, 1 graphs, 1 layouts"),
        "{out}"
    );
    assert!(
        out.contains("table     Plasma IL-6  [column, 3 data sets, 5 rows]"),
        "{out}"
    );
    assert!(
        out.contains("  analysis  One-way ANOVA of Plasma IL-6  [one_way_anova]"),
        "{out}"
    );
    assert!(
        out.contains("    graph     IL-6 by dose  [column]"),
        "{out}"
    );
    assert!(
        out.contains("layout    Figure 1  [1 panels, 1 columns: IL-6 by dose]"),
        "{out}"
    );
}

#[test]
fn run_prints_every_sheet_in_three_formats() {
    let d = tmp();
    let f = project(d.path());
    let tsv = ok(dioptra(&["run", s(&f)]));
    assert!(tsv.contains("One-way ANOVA of Plasma IL-6"), "{tsv}");
    assert!(tsv.contains("Control vs. Low dose"), "{tsv}");
    assert!(tsv.contains('\t'));
    let md = ok(dioptra(&[
        "run",
        s(&f),
        "--analysis",
        "one-way anova of plasma il-6",
        "--format",
        "markdown",
    ]));
    assert!(
        md.contains("| Control vs. High dose |") || md.contains("Control vs. High dose"),
        "{md}"
    );
    assert!(md.contains("|---"), "{md}");
    let js: Value =
        serde_json::from_str(&ok(dioptra(&["run", s(&f), "--format", "json"]))).unwrap();
    assert_eq!(js.as_array().unwrap().len(), 1);
    assert_eq!(js[0]["analysis_kind"], "one_way_anova");
    assert!(js[0]["summary"][0].as_str().unwrap().contains("F(2, 12)"));
    let e = fails(dioptra(&["run", s(&f), "--analysis", "Nope"]));
    assert!(
        e.contains("no analysis called \"Nope\"") && e.contains("One-way ANOVA of Plasma IL-6"),
        "{e}"
    );
}

#[test]
fn analyze_one_shot_with_options() {
    let d = tmp();
    let csv = p(d.path(), "il6.csv");
    std::fs::write(&csv, GROUPS_CSV).unwrap();
    let out = ok(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "one_way_anova",
    ]));
    assert!(out.contains("Tukey"), "{out}");
    assert!(out.contains("Control vs. High dose"), "{out}");
    // Options merge into the defaults.
    let saved = p(d.path(), "il6.dioptra");
    let out = ok(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "one_way_anova",
        "--options",
        r#"{"post_hoc": {"method": {"name": "bonferroni"}}}"#,
        "--format",
        "json",
        "--out",
        s(&saved),
    ]));
    let v: Value = serde_json::from_str(&out).unwrap();
    let text = v.to_string();
    assert!(text.contains("Bonferroni"), "{text}");
    assert!(saved.is_file());
    let e = fails(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "t_tset",
    ]));
    assert!(
        e.contains("unknown analysis \"t_tset\"") && e.contains("t_test"),
        "{e}"
    );
    let e = fails(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "survival",
    ]));
    assert!(e.contains("does not fit"), "{e}");
    let e = fails(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "one_way_anova",
        "--options",
        r#"{"posthoc": {}}"#,
    ]));
    assert!(
        e.contains("unknown option \"posthoc\"") && e.contains("post_hoc"),
        "{e}"
    );
    let e = fails(dioptra(&[
        "analyze",
        s(&csv),
        "--kind",
        "column",
        "--test",
        "t_test",
        "--options",
        "{oops",
    ]));
    assert!(e.contains("--options is not valid JSON"), "{e}");
}

#[test]
fn import_writes_a_project() {
    let d = tmp();
    let csv = p(d.path(), "groups.csv");
    std::fs::write(&csv, GROUPS_CSV).unwrap();
    let out = ok(dioptra(&["import", s(&csv), "--kind", "column"]));
    let proj = p(d.path(), "groups.dioptra");
    assert!(out.contains("Wrote") && proj.is_file(), "{out}");
    let info = ok(dioptra(&["info", s(&proj)]));
    assert!(
        info.contains("groups  [column, 3 data sets, 5 rows]"),
        "{info}"
    );
    let other = p(d.path(), "other.dioptra");
    ok(dioptra(&[
        "import",
        s(&csv),
        "--kind",
        "xy",
        "--out",
        s(&other),
    ]));
    assert!(ok(dioptra(&["info", s(&other)])).contains("[xy,"));
    let e = fails(dioptra(&["import", s(&p(d.path(), "missing.csv"))]));
    assert!(e.contains("no such data file"), "{e}");
    let e = fails(dioptra(&[
        "import",
        s(&csv),
        "--out",
        s(&p(d.path(), "x.zip")),
    ]));
    assert!(e.contains(".dioptra"), "{e}");
}

#[test]
fn render_graphs_and_layouts_in_three_formats() {
    let d = tmp();
    let f = project(d.path());
    let svg = p(d.path(), "fig.svg");
    ok(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "IL-6 by dose",
        "--out",
        s(&svg),
    ]));
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(text.starts_with("<svg") && text.contains("width=\"85mm\""));
    let pdf = p(d.path(), "fig.pdf");
    ok(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "il-6 by dose",
        "--out",
        s(&pdf),
        "--width-mm",
        "120",
    ]));
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-1."));
    let png = p(d.path(), "fig.png");
    ok(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "IL-6 by dose",
        "--out",
        s(&png),
        "--dpi",
        "100",
        "--width-mm",
        "50.8",
        "--height-mm",
        "25.4",
    ]));
    let b = std::fs::read(&png).unwrap();
    assert_eq!(&b[1..4], b"PNG");
    let w = u32::from_be_bytes(b[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(b[20..24].try_into().unwrap());
    assert_eq!((w, h), (200, 100));
    let lay = p(d.path(), "layout.svg");
    ok(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "Figure 1",
        "--out",
        s(&lay),
    ]));
    assert!(std::fs::read_to_string(&lay).unwrap().contains(">A</text>"));
    let e = fails(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "Nope",
        "--out",
        s(&svg),
    ]));
    assert!(
        e.contains("no graph or layout called \"Nope\"") && e.contains("\"IL-6 by dose\""),
        "{e}"
    );
    let e = fails(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "IL-6 by dose",
        "--out",
        s(&p(d.path(), "fig.jpg")),
    ]));
    assert!(e.contains("use .svg, .pdf or .png"), "{e}");
    let e = fails(dioptra(&[
        "render",
        s(&f),
        "--graph",
        "IL-6 by dose",
        "--out",
        s(&png),
        "--dpi",
        "0",
    ]));
    assert!(e.contains("--dpi"), "{e}");
}

#[test]
fn export_tables() {
    let d = tmp();
    let f = project(d.path());
    let csv = p(d.path(), "out.csv");
    ok(dioptra(&[
        "export",
        s(&f),
        "--table",
        "Plasma IL-6",
        "--out",
        s(&csv),
    ]));
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.starts_with("Control,Low dose,High dose"), "{text}");
    assert!(text.contains("4.1,5.2,7.9"), "{text}");
    let xlsx = p(d.path(), "out.xlsx");
    ok(dioptra(&[
        "export",
        s(&f),
        "--table",
        "Plasma IL-6",
        "--out",
        s(&xlsx),
    ]));
    assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"));
    let e = fails(dioptra(&[
        "export",
        s(&f),
        "--table",
        "Plasma IL-6",
        "--out",
        s(&p(d.path(), "out.ods")),
    ]));
    assert!(e.contains("use .csv, .tsv or .xlsx"), "{e}");
}

#[test]
fn equations_list_the_library() {
    let out = ok(dioptra(&["equations"]));
    assert!(out.contains("four_parameter_logistic"));
    assert!(out.contains("Y = Bottom + (Top-Bottom)/(1+10^((LogEC50-X)*HillSlope))"));
    assert!(out.contains("derived: EC50, Span"));
    let js: Value = serde_json::from_str(&ok(dioptra(&["equations", "--json"]))).unwrap();
    let list = js.as_array().unwrap();
    assert!(list.len() > 40, "{}", list.len());
    let mm = list
        .iter()
        .find(|e| e["key"] == "michaelis_menten")
        .unwrap();
    assert_eq!(mm["parameters"], json!(["Vmax", "Km"]));
}

#[test]
fn missing_and_broken_project_files_fail_clearly() {
    let d = tmp();
    let e = fails(dioptra(&["info", s(&p(d.path(), "none.dioptra"))]));
    assert!(e.starts_with("dioptra: error: no such project file"), "{e}");
    let bad = p(d.path(), "bad.dioptra");
    std::fs::write(&bad, b"not a zip").unwrap();
    let e = fails(dioptra(&["run", s(&bad)]));
    assert!(e.contains("cannot open"), "{e}");
    // Usage errors come from clap with its own exit code.
    let o = dioptra(&["render"]);
    assert_eq!(o.status.code(), Some(2));
}
