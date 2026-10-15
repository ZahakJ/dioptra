//! Real Prism files (fixtures/prism, see its README for sources and licences) import with
//! the right table kinds, shapes and values.

use dioptra_core::api::{SubcolumnFormat, TableKind, VariableType};
use dioptra_core::import::read_prism_file;
use dioptra_core::table::Table;

fn import(name: &str) -> (Vec<Table>, Vec<String>) {
    let path = format!("{}/../../fixtures/prism/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut n = 0;
    read_prism_file(&bytes, || {
        n += 1;
        format!("t{n}")
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn has_note(notes: &[String], s: &str) -> bool {
    notes.iter().any(|n| n.contains(s))
}

fn values(t: &Table, ds: usize, sub: usize) -> &[f64] {
    &t.datasets[ds].subcolumns[sub].values
}

#[test]
fn low_high_becomes_mean_and_limits() {
    let (ts, notes) = import("Yue-Jiang__column_low-high.pzfx");
    let t = &ts[0];
    assert_eq!(
        (t.kind, t.rows, t.datasets.len()),
        (TableKind::Grouped, 3, 4)
    );
    assert_eq!(t.datasets[0].name, "A (mean)");
    assert_eq!(values(t, 0, 0), [100.0, 110.0, 120.0]);
    assert_eq!(t.datasets[1].name, "A");
    assert_eq!(t.datasets[1].format, SubcolumnFormat::LowerUpper);
    // Mean, +error, -error = 100, 10, 30 -> lower 70, upper 110.
    assert_eq!(values(t, 1, 0), [70.0, 90.0, 110.0]);
    assert_eq!(values(t, 1, 1), [110.0, 130.0, 150.0]);
    assert!(has_note(&notes, "+error and −error"), "{notes:?}");
}

#[test]
fn upper_lower_limits_keep_their_values() {
    let (ts, notes) = import("Yue-Jiang__column_upper-lower-limits.pzfx");
    let t = &ts[0];
    assert_eq!(t.datasets.len(), 4);
    assert_eq!(values(t, 0, 0), [100.0, 110.0, 120.0]);
    assert_eq!(values(t, 1, 0), [70.0, 90.0, 110.0]);
    assert_eq!(values(t, 1, 1), [110.0, 120.0, 130.0]);
    assert_eq!(t.datasets[3].name, "B");
    assert_eq!(values(t, 3, 0), [7.0, 18.0, 29.0]);
    assert!(has_note(&notes, "two limits"), "{notes:?}");
}

#[test]
fn cv_becomes_sd() {
    let (ts, notes) = import("cheminfo__table.pzfx");
    let t = &ts[0];
    assert_eq!(t.datasets.len(), 2);
    assert_eq!(t.datasets[0].format, SubcolumnFormat::MeanSdN);
    // Mean 110, CV 20% -> SD 22.
    assert_eq!(values(t, 0, 0), [100.0, 110.0, 120.0]);
    assert_eq!(values(t, 0, 1), [10.0, 22.0, 36.0]);
    assert_eq!(values(t, 0, 2), [2.0, 3.0, 4.0]);
    assert!(has_note(&notes, "%CV"), "{notes:?}");
}

#[test]
fn date_x_keeps_the_dates() {
    let (ts, notes) = import("Yue-Jiang__x_date.pzfx");
    let t = &ts[0];
    assert_eq!((t.kind, t.rows), (TableKind::Xy, 50));
    let x = t.x.as_ref().unwrap();
    assert_eq!(x.title, "Date X");
    assert_eq!(x.values[..2], [0.0, 5.22929500342231]);
    assert_eq!(
        t.row_titles[..3],
        ["16-Jan-2004", "9-Apr-2009", "1-Mar-2012"]
    );
    assert!(
        has_note(&notes, "dates were kept as row titles"),
        "{notes:?}"
    );
}

#[test]
fn x_error_is_noted() {
    let (ts, notes) = import("Yue-Jiang__x_error_y_sdn.pzfx");
    let t = &ts[0];
    assert_eq!(t.x.as_ref().unwrap().values, [1.0, 2.0, 3.0]);
    assert_eq!(t.row_titles, ["A", "B", "C"]);
    assert_eq!(t.datasets[0].name, "Ya");
    assert_eq!(t.datasets[0].format, SubcolumnFormat::MeanSdN);
    assert_eq!(values(t, 0, 1), [50.0, 60.0, 70.0]);
    assert!(
        has_note(&notes, "X error subcolumn (3 values)"),
        "{notes:?}"
    );
}

#[test]
fn excluded_values_stay_excluded() {
    let (ts, _) = import("Yue-Jiang__x_y_with_strike.pzfx");
    let t = &ts[0];
    assert_eq!((t.rows, t.datasets.len()), (3, 3));
    assert_eq!(t.datasets[0].format, SubcolumnFormat::Values(2));
    assert!(t.datasets[0].subcolumns[0].excluded.contains(&1));
    assert!(t.datasets[0].subcolumns[1].excluded.contains(&1));
    assert!(t.datasets[1].subcolumns[1].excluded.is_empty());
    assert_eq!(values(t, 1, 1), [10.0, 9.0, 8.0]);
}

#[test]
fn survival_and_contingency() {
    let (ts, _) = import("Yue-Jiang__survival.pzfx");
    let t = &ts[0];
    assert_eq!((t.kind, t.rows), (TableKind::Survival, 27));
    assert_eq!(t.x.as_ref().unwrap().title, "Days");
    assert_eq!(t.x.as_ref().unwrap().values[..3], [78.0, 34.0, 123.0]);
    assert_eq!(t.datasets[1].name, "Treatment A");
    let (ts, _) = import("Yue-Jiang__contingency.pzfx");
    let t = &ts[0];
    assert_eq!(t.kind, TableKind::Contingency);
    assert_eq!(t.row_titles, ["a", "b"]);
    assert_eq!(values(t, 1, 0), [30.0, 40.0]);
}

#[test]
fn nested_tables_name_their_subgroups() {
    let (ts, notes) = import("hammerlab__Fig2.pzfx");
    assert_eq!(ts.len(), 2);
    let t = &ts[0];
    assert_eq!(
        (t.kind, t.title.as_str()),
        (TableKind::Nested, "unstimulated")
    );
    assert_eq!(t.datasets.len(), 3);
    let d = &t.datasets[0];
    assert_eq!(
        (d.name.as_str(), d.format),
        ("Donor 21", SubcolumnFormat::Values(2))
    );
    assert_eq!(d.subcolumns[0].title, "15 min");
    assert_eq!(d.subcolumns[1].title, "1 day");
    assert_eq!(values(t, 0, 1), [49.93]);
    assert_eq!(t.col_header(1), "Donor 21 (1 day)");
    assert_eq!(values(&ts[1], 2, 0), [92.5]);
    assert!(!has_note(&notes, "SubColumnTitles"), "{notes:?}");
}

#[test]
fn multiple_variables_and_own_subcolumn_titles() {
    let (ts, _) = import("smestern__prism_template2.pzfx");
    let kinds: Vec<_> = ts.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [
            TableKind::Variables,
            TableKind::Grouped,
            TableKind::Column,
            TableKind::Xy
        ]
    );
    let mv = &ts[0];
    assert_eq!((mv.rows, mv.datasets.len()), (100, 2));
    assert_eq!(mv.datasets[0].var_type, Some(VariableType::Number));
    assert_eq!(values(mv, 0, 0)[..3], [1.0, 2.0, 3.0]);
    // OwnSet="1": each data set names its own subcolumns.
    let g = &ts[1];
    let titles = |d: usize| -> Vec<&str> {
        g.datasets[d]
            .subcolumns
            .iter()
            .map(|c| c.title.as_str())
            .collect()
    };
    assert_eq!(titles(0), ["sub1", "sub2", ""]);
    assert_eq!(titles(1), ["sub4", "sub5", "sub6"]);

    let (ts, _) = import("taehoonh__cats_multiple.pzfx");
    let t = &ts[0];
    assert_eq!((t.kind, t.rows), (TableKind::Variables, 144));
    let names: Vec<_> = t.datasets.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Sex", "Bwt", "Hwt"]);
    assert_eq!(t.datasets[0].var_type, Some(VariableType::Category));
    assert_eq!(t.datasets[0].subcolumns[0].text[&0], "F");
    assert_eq!(t.datasets[0].subcolumns[0].text[&143], "M");
}

#[test]
fn prism_1_2_column_table() {
    let (ts, notes) = import("figshare34328751__ACTC1.prism");
    assert_eq!(ts.len(), 1);
    let t = &ts[0];
    assert_eq!(
        (t.kind, t.title.as_str(), t.rows),
        (TableKind::Column, "数据1", 6)
    );
    let names: Vec<_> = t.datasets.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["pcDNA3.1", "pcDNA3.1-ACTC1"]);
    assert_eq!(values(t, 0, 0)[..2], [1.278650699, 1.193023287]);
    assert_eq!(values(t, 1, 0)[5], 20518.28036);
    assert!(has_note(&notes, "1 graph was not"), "{notes:?}");
}

#[test]
fn prism_1_4_grouped_and_column() {
    let (ts, notes) = import("figshare34469319__RTqPCR_MEC_2D_vs_3D__4_genes.prism");
    let shape: Vec<_> = ts
        .iter()
        .map(|t| (t.title.as_str(), t.kind, t.rows, t.datasets.len()))
        .collect();
    assert_eq!(
        shape,
        [
            ("Gene expression Z-score", TableKind::Grouped, 4, 2),
            ("Gene expression fold change", TableKind::Grouped, 5, 2),
            ("Gene expression (2D)", TableKind::Column, 5, 4),
        ]
    );
    let z = &ts[0];
    assert_eq!(z.datasets[1].name, "3D");
    assert_eq!(z.datasets[0].format, SubcolumnFormat::Values(5));
    assert_eq!(values(z, 0, 0)[..3], [0.638, -0.951, -0.37]);
    assert_eq!(values(z, 1, 4)[0], -0.531);
    let c = &ts[2];
    assert_eq!(c.datasets[3].name, "ACTA2");
    assert_eq!(values(c, 0, 0), [0.956, 1.069, 0.994, 1.065, 0.924]);
    assert!(has_note(&notes, "3 graphs were not"), "{notes:?}");
}

#[test]
fn prism_1_6_rich_titles_and_ragged_columns() {
    let (ts, _) = import("figshare33793582__X_inactivation_graphs.prism");
    assert_eq!(ts.len(), 2);
    let t = &ts[0];
    assert_eq!(
        (t.title.as_str(), t.kind, t.rows),
        ("%GFP+", TableKind::Column, 9)
    );
    let names: Vec<_> = t.datasets.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Hprt1WT", "Hprt1Hemi", "Hprt1Het"]);
    assert_eq!(values(t, 0, 0)[..4], [0.8, 1.98, 9.46, 3.33]);
    assert!(values(t, 1, 0)[2].is_nan());
    assert_eq!(values(t, 2, 0)[7], 42.8);
    assert_eq!(values(&ts[1], 1, 0)[..3], [0.0, 0.0, 0.05]);
}

#[test]
fn not_a_prism_zip() {
    let mut n = 0;
    let r = read_prism_file(b"PK\x03\x04garbage", || {
        n += 1;
        format!("t{n}")
    });
    assert!(r.is_err());
}

#[test]
fn every_fixture_imports() {
    let base = format!("{}/../../fixtures/prism", env!("CARGO_MANIFEST_DIR"));
    let mut n = 0;
    for dir in [base.clone(), format!("{base}/corpus")] {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "pzfx" && ext != "prism" {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let mut k = 0;
            let (tables, _) = read_prism_file(&bytes, || {
                k += 1;
                format!("t{k}")
            })
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(!tables.is_empty(), "{}", path.display());
            n += 1;
        }
    }
    assert!(n >= 30, "only {n} fixtures");
}
