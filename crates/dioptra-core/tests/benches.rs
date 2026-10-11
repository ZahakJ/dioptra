//! Timings, run with
//! `cargo test -p dioptra-core --release -- --ignored --nocapture bench_`.
//! In release builds the targets from the brief are asserted, too.

mod common;

use std::time::Instant;

use common::*;
use dioptra_core::api::Session;
use serde_json::json;

fn grid_text(rows: usize, cols: usize) -> String {
    let mut s = String::with_capacity(rows * cols * 9);
    for r in 0..rows {
        for c in 0..cols {
            if c > 0 {
                s.push('\t');
            }
            // Mixed magnitudes and decimals, like instrument output.
            let v = (r * cols + c) as f64 * 0.731 + 0.5;
            s.push_str(&format!("{v:.3}"));
        }
        s.push('\n');
    }
    s
}

fn paste(rows: usize, cols: usize) -> f64 {
    let mut s = Session::new();
    let t = create(&mut s, "column", 1, json!({"kind": "values", "n": 1}), 0);
    let text = grid_text(rows, cols);
    let cmd =
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {}})
            .to_string();
    let t0 = Instant::now();
    let out = s.dispatch_json(&cmd);
    let secs = t0.elapsed().as_secs_f64();
    assert!(
        out.contains("\"paste_report\""),
        "{}",
        &out[..out.len().min(300)]
    );
    assert!(out.contains(&format!("\"numeric_cells\":{}", rows * cols)));
    // Viewport read right after (no recompute pending).
    let v0 = Instant::now();
    let c = cells(&mut s, &t, rows / 2, rows / 2 + 50, 0, cols.min(30));
    let view = v0.elapsed().as_secs_f64();
    assert_eq!(c.len(), 50);
    println!("bench paste {cols} x {rows} ({} cells, {:.1} MB text): {:.3} s; 50-row viewport after: {:.2} ms", rows * cols, text.len() as f64 / 1e6, secs, view * 1e3);
    secs
}

#[test]
#[ignore]
fn bench_paste_1000_columns_by_1000_rows() {
    let secs = paste(1000, 1000);
    if !cfg!(debug_assertions) {
        assert!(secs < 1.5, "paste took {secs:.3} s");
    }
}

#[test]
#[ignore]
fn bench_paste_10_columns_by_100000_rows() {
    let secs = paste(100_000, 10);
    if !cfg!(debug_assertions) {
        assert!(secs < 1.5, "paste took {secs:.3} s");
    }
}

#[test]
#[ignore]
fn bench_formula_over_1m_rows() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 2, json!({"kind": "values", "n": 1}), 0);
    let text: String = (0..1_000_000)
        .map(|i| format!("{}\n", (i % 977) as f64 + 0.25))
        .collect();
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {"header": false}}),
    );
    for (expr, label) in [
        ("A * 2 + 1", "arithmetic"),
        ("log10(A) * 100 / (A + 1)", "element-wise functions"),
        ("(A - mean(A)) / sd(A)", "with aggregates"),
        ("if(A > 500, A, NA)", "conditional"),
        ("lag(A, 1) - A", "series"),
    ] {
        ok(
            &mut s,
            json!({"type": "set_column_formula", "table": t, "col": 1, "expr": expr}),
        );
        // Pull: the first read recomputes the dirty column.
        let t0 = Instant::now();
        let c = cells(&mut s, &t, 0, 3, 0, 2);
        let secs = t0.elapsed().as_secs_f64();
        assert_eq!(c.len(), 3);
        println!(
            "bench formula ={expr} ({label}) over 1,000,000 rows: {:.1} ms",
            secs * 1e3
        );
        if !cfg!(debug_assertions) && label == "arithmetic" {
            assert!(secs < 0.150, "recompute took {:.1} ms", secs * 1e3);
        }
    }
}
