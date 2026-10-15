//! Plate-reader exports (fixtures/plate, from Benchling's allotropy; see its README) read with
//! the values in the files, and `ReplaceTableData` swaps a table's data in place.

use dioptra_core::api::{Command, Response, Session, SubcolumnFormat, TableKind};
use dioptra_core::import::PasteReport;
use dioptra_core::plate::{self, PlateExport, PlateRead, Vendor};
use dioptra_core::table::Table;
use dioptra_core::util::base64_encode;
use serde_json::json;

fn bytes(name: &str) -> Vec<u8> {
    let path = format!("{}/../../fixtures/plate/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn read(name: &str) -> PlateExport {
    plate::read(name, &bytes(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .unwrap_or_else(|| panic!("{name} was not recognised"))
}

/// Value of a well in frame `f`.
fn at(r: &PlateRead, f: usize, well: &str) -> f64 {
    let (row, col) = plate::parse_well(well).unwrap();
    r.frames[f][row * r.cols + col]
}

fn names(e: &PlateExport) -> Vec<&str> {
    e.reads.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn softmax_plate_format_three_wavelengths_utf16() {
    let e = read("softmax_abs_endpoint_partial_plate.txt");
    assert_eq!(e.vendor, Vendor::SoftMaxPro);
    assert_eq!(names(&e), ["230 nm", "260 nm", "340 nm"]);
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols, r.frames.len()), (8, 12, 1));
    assert!(!r.kinetic());
    assert_eq!(at(r, 0, "C3"), 3.147);
    assert_eq!(at(r, 0, "D3"), 0.6);
    assert_eq!(at(r, 0, "C10"), 3.925);
    assert!(at(r, 0, "A1").is_nan() && at(r, 0, "C11").is_nan());
    assert_eq!(at(&e.reads[1], 0, "C3"), 0.611);
    assert_eq!(at(&e.reads[2], 0, "C3"), 0.97);
    assert!(e.notes.iter().any(|n| n.contains("group")));
}

#[test]
fn softmax_kinetic_plate_format() {
    let e = read("softmax_fl_kinetic.txt");
    assert_eq!(names(&e), ["525 nm (1)", "525 nm (2)"]);
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols), (2, 3));
    assert_eq!(r.times, [0.0, 90.0, 180.0, 270.0]);
    assert_eq!(at(r, 1, "A1"), 39.50988);
    assert_eq!(at(r, 1, "B3"), 10.85013);
    assert_eq!(at(r, 3, "B1"), 112.43051);
    assert_eq!(at(&e.reads[1], 1, "A1"), 19.75494);
    assert_eq!(at(&e.reads[1], 3, "B3"), 15.83771);
}

#[test]
fn softmax_time_format_two_plates() {
    let e = read("softmax_lum_timeformat.txt");
    assert_eq!(
        names(&e),
        ["c12345_2 · Luminescence", "c123455_3 · Luminescence"]
    );
    assert_eq!(at(&e.reads[0], 0, "A1"), 1011329.0);
    assert_eq!(at(&e.reads[0], 0, "A2"), 929426.0);
    assert_eq!(at(&e.reads[1], 0, "A1"), 1497823.0);
    assert_eq!(e.reads[0].frames.len(), 1);
}

#[test]
fn bmg_mars_grids() {
    let e = read("bmg_abs_96.csv");
    assert_eq!(e.vendor, Vendor::BmgMars);
    assert_eq!(names(&e), ["Raw Data (450 1)"]);
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols), (8, 12));
    assert_eq!(at(r, 0, "A1"), 0.073);
    assert_eq!(at(r, 0, "H12"), 0.207);
    let e = read("bmg_abs_384.csv");
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols), (16, 24));
    assert_eq!(at(r, 0, "A1"), 0.031);
    assert_eq!(at(r, 0, "C6"), 2.412);
    assert!(at(r, 0, "C7").is_nan());
    assert_eq!(at(r, 0, "P24"), 0.032);
}

#[test]
fn gen5_endpoint_labels() {
    let e = read("gen5_abs_endpoint.txt");
    assert_eq!(e.vendor, Vendor::Gen5);
    assert_eq!(
        names(&e),
        [
            "630nmAbsRead:630",
            "Blank 630nmAbsRead:630",
            "[Concentration]"
        ]
    );
    assert_eq!(at(&e.reads[0], 0, "A1"), 3.337);
    assert_eq!(at(&e.reads[0], 0, "H12"), 0.194);
    assert_eq!(at(&e.reads[1], 0, "A1"), 3.336);
    assert_eq!(at(&e.reads[2], 0, "A1"), 1980.423);
    let e = read("gen5_two_reads.txt");
    assert_eq!(names(&e), ["od:600", "260:280"]);
    assert_eq!(at(&e.reads[0], 0, "B1"), 0.124);
    assert_eq!(at(&e.reads[1], 0, "H10"), 852.0);
}

#[test]
fn gen5_kinetic_long_format() {
    let e = read("gen5_abs_kinetic.txt");
    assert_eq!(names(&e), ["600"]);
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols), (4, 6));
    assert_eq!(r.times.len(), 20, "padding rows without values are dropped");
    assert_eq!(r.times[0], 22.0);
    assert_eq!(r.times[19], 3600.0 + 16.0 * 60.0 + 22.0);
    assert_eq!(at(r, 0, "A1"), -0.066);
    assert_eq!(at(r, 0, "A2"), -0.068);
}

#[test]
fn magellan_list_layout() {
    let e = read("magellan_od.xlsx");
    assert_eq!(e.vendor, Vendor::Magellan);
    assert_eq!(
        names(&e),
        ["Raw data"],
        "the plate-name column is not a read"
    );
    let r = &e.reads[0];
    assert_eq!((r.rows, r.cols), (8, 12));
    assert_eq!(at(r, 0, "A1"), 0.1462);
    assert_eq!(at(r, 0, "B1"), 0.2118);
    assert_eq!(at(r, 0, "E6"), 0.1478);
    let e = read("magellan_elisa.xlsx");
    assert_eq!(names(&e), ["Label1", "Label2"]);
    assert_eq!((e.reads[0].rows, e.reads[0].cols), (16, 24));
    assert_eq!(at(&e.reads[0], 0, "A1"), 0.0557);
    assert_eq!(at(&e.reads[1], 0, "A1"), 0.0509);
    assert_eq!(at(&e.reads[0], 0, "B13"), 0.6458);
    assert_eq!(at(&e.reads[1], 0, "P24"), 0.05);
}

#[test]
fn ordinary_files_are_not_plates() {
    assert!(plate::read("a.csv", b"x,y\n1,2\n").unwrap().is_none());
    assert!(plate::read("a.txt", b"hello\nworld\n").unwrap().is_none());
    assert!(plate::read("a.pzfx", &bytes("bmg_abs_96.csv"))
        .unwrap()
        .is_none());
}

fn table(e: &PlateExport, map: Option<&str>) -> (Table, PasteReport) {
    let mut report = PasteReport::default();
    let map = map.map(|m| plate::parse_plate_map(m).unwrap());
    let t = plate::build_table("t".into(), "T".into(), "f", e, map.as_ref(), &mut report).unwrap();
    (t, report)
}

#[test]
fn plate_tables() {
    // Endpoint: grouped, rows A–H, a data set per read with columns 1–12.
    let e = read("softmax_abs_endpoint_partial_plate.txt");
    let (t, r) = table(&e, None);
    assert_eq!(
        (t.kind, t.rows, t.datasets.len()),
        (TableKind::Grouped, 8, 3)
    );
    assert_eq!(t.row_titles[2], "C");
    assert_eq!(t.datasets[0].format, SubcolumnFormat::Values(12));
    assert_eq!(t.datasets[0].subcolumns[2].title, "3");
    assert_eq!(t.datasets[0].subcolumns[2].values[2], 3.147);
    assert!(r.notes[0].contains("SoftMax Pro"));

    // Kinetic: XY, time in X, a data set per well and read.
    let e = read("softmax_fl_kinetic.txt");
    let (t, _) = table(&e, None);
    assert_eq!((t.kind, t.rows, t.datasets.len()), (TableKind::Xy, 4, 12));
    assert_eq!(t.x.as_ref().unwrap().values, [0.0, 90.0, 180.0, 270.0]);
    assert_eq!(t.datasets[0].name, "A1 · 525 nm (1)");
    assert_eq!(t.datasets[0].subcolumns[0].values[1], 39.50988);
    // Kinetic with a map: groups of wells as replicates.
    let (t, _) = table(&e, Some("A1,Ctrl\nB1,Ctrl\nA2,Drug\n"));
    assert_eq!(t.datasets.len(), 4);
    assert_eq!(t.datasets[0].name, "Ctrl · 525 nm (1)");
    assert_eq!(t.datasets[0].format, SubcolumnFormat::Values(2));
    assert_eq!(t.datasets[0].subcolumns[1].values[3], 112.43051);

    // Endpoint with doses: XY, X = dose, replicates as subcolumns.
    let e = read("bmg_abs_96.csv");
    let map = "\t1\t2\t3\nA\tS @ 1\tS @ 10\tS @ 100\nB\tS @ 1\tS @ 10\tS @ 100\nC\tBlank\t\t\n";
    let (t, r) = table(&e, Some(map));
    assert_eq!((t.kind, t.rows, t.datasets.len()), (TableKind::Xy, 3, 1));
    assert_eq!(t.x.as_ref().unwrap().values, [1.0, 10.0, 100.0]);
    assert_eq!(t.datasets[0].name, "S");
    assert_eq!(t.datasets[0].subcolumns[0].values, [0.073, 0.082, 0.08]);
    assert_eq!(t.datasets[0].subcolumns[1].values, [0.045, 0.067, 0.012]);
    assert!(r.notes.iter().any(|n| n.contains("no dose")));

    // Endpoint with groups only: a column table.
    let (t, _) = table(&e, Some("A1\tCtrl\nA2\tCtrl\nH12\tDrug\n"));
    assert_eq!((t.kind, t.rows), (TableKind::Column, 2));
    assert_eq!(t.datasets[0].subcolumns[0].values, [0.073, 0.082]);
    assert_eq!(t.datasets[1].subcolumns[0].values[0], 0.207);
    assert!(t.datasets[1].subcolumns[0].values[1].is_nan());
}

fn call(s: &mut Session, v: serde_json::Value) -> Response {
    let cmd: Command = serde_json::from_value(v).unwrap();
    match s.dispatch(cmd) {
        Response::Error { message } => panic!("{message}"),
        r => r,
    }
}

fn created(r: &Response) -> String {
    match r {
        Response::PasteReport(p) => p.changed.created.clone().unwrap(),
        Response::Ok { changed } => changed.created.clone().unwrap(),
        r => panic!("unexpected {r:?}"),
    }
}

#[test]
fn import_and_inspect_through_the_session() {
    let mut s = Session::new();
    let b64 = base64_encode(&bytes("gen5_two_reads.txt"));
    match call(
        &mut s,
        json!({"type": "inspect_plate", "name": "g.txt", "base64": b64}),
    ) {
        Response::Plate { plate: Some(p) } => {
            assert_eq!(p.reads, ["od:600", "260:280"]);
            assert_eq!((p.rows, p.cols, p.kinetic), (8, 12, false));
        }
        r => panic!("unexpected {r:?}"),
    }
    let csv = base64_encode(b"a,b\n1,2\n");
    match call(
        &mut s,
        json!({"type": "inspect_plate", "name": "x.csv", "base64": csv}),
    ) {
        Response::Plate { plate: None } => {}
        r => panic!("unexpected {r:?}"),
    }
    let r = call(
        &mut s,
        json!({"type": "import_bytes", "name": "g.txt", "base64": b64, "kind": null,
               "plate_map": "A1,Blank\nB1,Blank\nA2,S\n"}),
    );
    let id = created(&r);
    let t = &s.project().tables[&id];
    assert_eq!(t.kind, TableKind::Column);
    assert_eq!(t.datasets[0].name, "Blank · od:600");
    assert_eq!(t.datasets[0].subcolumns[0].values, [0.052, 0.124]);
    // One undo step removes it.
    call(&mut s, json!({"type": "undo"}));
    assert!(!s.project().tables.contains_key(&id));
}

#[test]
fn replace_table_data_keeps_formulas_analyses_and_graphs() {
    let mut s = Session::new();
    let id = created(&call(
        &mut s,
        json!({"type": "import_text", "text": "x\ty\n1\t2\n2\t4\n3\t6\n", "kind": "xy",
               "title": "Doses", "options": {}}),
    ));
    call(
        &mut s,
        json!({"type": "set_dataset_name", "table": id, "dataset": 0, "name": "Response"}),
    );
    // A formula data set after the data: 10 × Y.
    call(
        &mut s,
        json!({"type": "add_dataset", "table": id, "name": "Scaled"}),
    );
    call(
        &mut s,
        json!({"type": "set_column_formula", "table": id, "col": 2, "expr": "10 * A"}),
    );
    let spec = match call(
        &mut s,
        json!({"type": "default_analysis_spec", "table": id, "kind": "linear_regression"}),
    ) {
        Response::AnalysisSpec(sp) => sp,
        r => panic!("unexpected {r:?}"),
    };
    let a = created(&call(
        &mut s,
        json!({"type": "create_analysis", "table": id, "spec": spec}),
    ));
    let g = created(&call(
        &mut s,
        json!({"type": "create_graph", "source": id, "family": "scatter", "title": null}),
    ));
    let csv = base64_encode(b"dose,resp,extra\n1,5\n2,7\n3,9\n4,11\n");
    let r = call(
        &mut s,
        json!({"type": "replace_table_data", "table": id, "name": "new.csv", "base64": csv}),
    );
    let Response::PasteReport(rep) = r else {
        panic!()
    };
    assert!(rep.changed.ids.contains(&a) && rep.changed.ids.contains(&g));
    let t = &s.project().tables[&id];
    assert_eq!((t.rows, t.title.as_str()), (4, "Doses"));
    assert_eq!(t.x.as_ref().unwrap().values, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(t.datasets[0].name, "Response");
    assert_eq!(t.datasets[0].subcolumns[0].values, [5.0, 7.0, 9.0, 11.0]);
    // The third column of the file is empty: the extra data set holds nothing.
    let cells = call(
        &mut s,
        json!({"type": "get_cells", "table": id, "range": {"row0": 0, "row1": 4, "col0": 2, "col1": 3}}),
    );
    let Response::Cells { rows, .. } = cells else {
        panic!()
    };
    let scaled: Vec<f64> = rows.iter().map(|r| r[0].v.unwrap()).collect();
    assert_eq!(
        scaled,
        [50.0, 70.0, 90.0, 110.0],
        "the formula column is kept and recomputed"
    );
    match call(&mut s, json!({"type": "get_results", "id": a})) {
        Response::Results(sheet) => {
            use dioptra_core::results::ResultCell;
            let slope = sheet
                .sections
                .iter()
                .flat_map(|sec| &sec.table.rows)
                .find_map(|r| match r.as_slice() {
                    [_, ResultCell::Text(k), ResultCell::Number { v, .. }, ..] if k == "Slope" => {
                        Some(*v)
                    }
                    _ => None,
                });
            assert_eq!(slope, Some(2.0));
        }
        r => panic!("unexpected {r:?}"),
    }
    assert!(s.project().graphs.contains_key(&g));
    // One undo step brings the old data back.
    call(&mut s, json!({"type": "undo"}));
    let t = &s.project().tables[&id];
    assert_eq!(t.rows, 3);
    assert_eq!(t.datasets[0].subcolumns[0].values, [2.0, 4.0, 6.0]);
    // A different kind is refused.
    let plate = base64_encode(&bytes("bmg_abs_96.csv"));
    let cmd: Command = serde_json::from_value(
        json!({"type": "replace_table_data", "table": id, "name": "p.csv", "base64": plate}),
    )
    .unwrap();
    match s.dispatch(cmd) {
        Response::Error { message } => assert!(message.contains("grouped table"), "{message}"),
        r => panic!("unexpected {r:?}"),
    }
}
