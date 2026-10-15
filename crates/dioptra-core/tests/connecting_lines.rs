//! Connecting lines in the document model: the spec round-trips through patches and the
//! project file, format 1.0 `before_after` graphs migrate to the strip spec that draws the
//! same picture, paired analyses default to bars with subject lines, and a grouping column
//! colours the lines without being plotted.

mod common;

use common::*;
use dioptra_core::api::Session;
use dioptra_core::file;
use dioptra_core::graph::{Connect, GraphSpec};
use serde_json::{json, Value};

fn paired_table(s: &mut Session) -> String {
    let t = create(s, "column", 2, json!({"kind": "values", "n": 1}), 0);
    ok(
        s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "Before\tAfter\n120\t111\n131\t122\n118\t\n142\t130\n109\t104", "options": {"header": true}}),
    );
    t
}

fn created(r: Value) -> String {
    r["changed"]["created"].as_str().unwrap().to_string()
}

#[test]
fn spec_round_trips_through_patch_and_file() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    let t = paired_table(&mut s);
    let g = created(ok(
        &mut s,
        json!({"type": "create_graph", "source": t, "family": "column"}),
    ));
    let patch = json!({"options": {
        "connecting": {"connect": "both", "style": "dashed", "width_pt": 0.5, "colour_by": "subject",
                       "palette": "tol_muted", "opacity": 0.8, "missing": "skip",
                       "summary_style": "dotted", "summary_width_pt": 2.5, "summary_colour": "#123456"},
        "orientation": "horizontal", "error_direction": "above", "cap_pt": 3.0,
        "bar_width": 0.5, "group_width": 0.7, "bar_fill": "solid", "summary_line": false},
        "style": {"datasets": [{"dataset": 1, "pattern": "crosshatch", "marker": "diamond_open"}]}});
    ok(
        &mut s,
        json!({"type": "patch_graph", "id": g, "patch": patch}),
    );
    let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
    assert_eq!(spec["options"]["connecting"]["connect"], "both");
    assert_eq!(spec["options"]["connecting"]["missing"], "skip");
    assert_eq!(spec["options"]["orientation"], "horizontal");
    assert_eq!(spec["style"]["datasets"][0]["pattern"], "crosshatch");
    let typed: GraphSpec = serde_json::from_value(strip_type(spec.clone())).unwrap();
    // Serialise → deserialise is the identity.
    let again: GraphSpec = serde_json::from_str(&serde_json::to_string(&typed).unwrap()).unwrap();
    assert_eq!(typed, again);
    // Undo removes the options; redo restores them.
    ok(&mut s, json!({"type": "undo"}));
    let undone = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
    assert!(undone["options"].get("connecting").is_none());
    ok(&mut s, json!({"type": "redo"}));
    // Save, open: identical spec and identical SVG.
    let svg1 = ok(&mut s, json!({"type": "render_graph", "id": g}))["svg"].clone();
    let saved = ok(&mut s, json!({"type": "save_project_bytes"}));
    let bytes = dioptra_core::util::base64_decode(saved["base64"].as_str().unwrap()).unwrap();
    let (p2, _) = file::open_bytes(&bytes).unwrap();
    assert_eq!(p2.graphs[&g].spec, typed);
    let mut s2 = Session::new();
    ok(
        &mut s2,
        json!({"type": "open_project_bytes", "base64": saved["base64"], "name": null}),
    );
    let svg2 = ok(&mut s2, json!({"type": "render_graph", "id": g}))["svg"].clone();
    assert_eq!(svg1, svg2);
    // The manifest names the current format.
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut m = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.json").unwrap(), &mut m).unwrap();
    assert!(m.contains(dioptra_core::file::FORMAT), "{m}");
}

fn strip_type(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("type");
    v
}

#[test]
fn default_options_stay_out_of_the_file() {
    let spec = GraphSpec::new("t".into(), "column");
    let j = serde_json::to_value(&spec).unwrap();
    for k in [
        "connecting",
        "orientation",
        "error_direction",
        "bar_fill",
        "summary_line",
    ] {
        assert!(j["options"].get(k).is_none(), "{k} written by default");
    }
    // And a 0.2 spec without them reads with the defaults.
    let old: GraphSpec = serde_json::from_str(r#"{"source":"t","family":"column"}"#).unwrap();
    assert_eq!(old.options.connecting.connect, Connect::None);
    assert!(old.options.summary_line);
}

/// A format 1.0 file (Dioptra 0.2) with a `before_after` graph.
fn v1_file(mean_line: bool) -> Vec<u8> {
    use std::io::Write;
    let mut s = Session::new();
    fixed_clock(&mut s);
    let t = paired_table(&mut s);
    let g = created(ok(
        &mut s,
        json!({"type": "create_graph", "source": t, "family": "column"}),
    ));
    let saved = ok(&mut s, json!({"type": "save_project_bytes"}));
    let bytes = dioptra_core::util::base64_decode(saved["base64"].as_str().unwrap()).unwrap();
    // Rewrite: manifest 1.0, graph spec as 0.2 wrote a before–after graph.
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut f, &mut text).unwrap();
        if name == "manifest.json" {
            text = text.replace(dioptra_core::file::FORMAT, "dioptra/1");
        }
        if name == format!("graphs/{g}.json") {
            let mut v: Value = serde_json::from_str(&text).unwrap();
            v["spec"]["family"] = json!("before_after");
            v["spec"]["options"]["mean_line"] = json!(mean_line);
            text = serde_json::to_string_pretty(&v).unwrap();
        }
        z.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

#[test]
fn format_1_0_before_after_graphs_migrate() {
    for mean_line in [true, false] {
        let (p, _) = file::open_bytes(&v1_file(mean_line)).unwrap();
        let g = p.graphs.values().next().unwrap();
        assert_eq!(g.spec.family, "strip");
        assert_eq!(g.spec.options.jitter, 0.0);
        assert!(!g.spec.options.summary_line);
        assert_eq!(
            g.spec.options.connecting.connect,
            if mean_line {
                Connect::Both
            } else {
                Connect::Individuals
            }
        );
        // Saved again, the file is format 1.1 and reopens unchanged.
        let again = file::save_bytes(&p, &[]).unwrap();
        let (p2, _) = file::open_bytes(&again).unwrap();
        assert!(p.same_content(&p2));
    }
}

#[test]
fn paired_analyses_default_to_bars_with_subject_lines() {
    for spec in [
        json!({"kind": "t_test", "variant": "paired"}),
        json!({"kind": "wilcoxon"}),
    ] {
        let mut s = Session::new();
        let t = paired_table(&mut s);
        let a = created(ok(
            &mut s,
            json!({"type": "create_analysis", "table": t, "spec": spec}),
        ));
        let g = created(ok(
            &mut s,
            json!({"type": "create_graph", "source": a, "family": "auto"}),
        ));
        let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
        assert_eq!(spec["family"], "column");
        assert_eq!(spec["options"]["connecting"]["connect"], "individuals");
        let svg = ok(&mut s, json!({"type": "render_graph", "id": g}));
        let svg = svg["svg"].as_str().unwrap();
        // Four complete pairs; row 3 has no "after" value.
        assert_eq!(svg.matches("id=\"subject-").count(), 4, "{spec}");
        assert!(svg.contains("annotation-0"), "the bracket is kept");
    }
}

#[test]
fn a_grouping_column_colours_lines_and_is_not_plotted() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 3, json!({"kind": "values", "n": 1}), 0);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": "Pre\tPost\tSex\n1\t2\tF\n2\t3\tM\n3\t3.5\tF", "options": {"header": true}}),
    );
    let g = created(ok(
        &mut s,
        json!({"type": "create_graph", "source": t, "family": "paired"}),
    ));
    ok(
        &mut s,
        json!({"type": "patch_graph", "id": g, "patch": {"options": {"connecting": {"colour_by": "group", "group_column": 2}}}}),
    );
    let r = ok(&mut s, json!({"type": "render_graph", "id": g}));
    let svg = r["svg"].as_str().unwrap();
    assert_eq!(svg.matches("id=\"subject-").count(), 3);
    assert!(
        !svg.contains(">Sex<"),
        "the grouping column is not a category"
    );
    assert!(
        svg.contains(">F<") && svg.contains(">M<"),
        "legend lists the groups"
    );
}
