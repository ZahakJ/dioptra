//! Every analysis kind end to end through `dispatch_json`: the sheet has its sections, the
//! key numbers equal the stats crate's direct call (or the oracle fixture), the assumption
//! checks are there, and the summary sentence carries the formatted p and the CI. Then
//! staleness after an edit, graphs from analyses, and a project round trip.

// 3.14 in the Hollander & Wolfe data is a measurement, not pi.
#![allow(clippy::approx_constant)]

mod common;

use common::*;
use dioptra_core::api::Session;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn table(s: &mut Session, kind: &str, datasets: usize, format: Value, text: &str) -> String {
    let t = create(s, kind, datasets, format, 0);
    ok(
        s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {}}),
    );
    t
}

fn analyze(s: &mut Session, t: &str, spec: Value) -> (String, Value) {
    let a = ok(
        s,
        json!({"type": "create_analysis", "table": t, "spec": spec}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let r = ok(s, json!({"type": "get_results", "id": a}));
    assert!(
        r.get("message").is_none(),
        "analysis failed: {}",
        r["message"]
    );
    (a, r)
}

fn sec<'a>(r: &'a Value, title: &str) -> &'a Value {
    r["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["title"].as_str().unwrap().starts_with(title))
        .unwrap_or_else(|| {
            panic!(
                "no section \"{title}\" in {:?}",
                r["sections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["title"].clone())
                    .collect::<Vec<_>>()
            )
        })
}

fn has_sections(r: &Value, titles: &[&str]) {
    for t in titles {
        sec(r, t);
    }
}

/// Number in a cell (number or p value).
fn n(c: &Value) -> f64 {
    if let Some(v) = c.get("number") {
        return v["v"].as_f64().unwrap_or(f64::NAN);
    }
    if let Some(v) = c.get("p_value") {
        return v["p"].as_f64().unwrap_or(f64::NAN);
    }
    panic!("not a number cell: {c}")
}

/// Row of a section whose first cell is text starting with `key`.
fn row<'a>(r: &'a Value, title: &str, key: &str) -> &'a Vec<Value> {
    sec(r, title)["table"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_array().unwrap())
        .find(|r| r[0]["text"].as_str().is_some_and(|t| t.starts_with(key)))
        .unwrap_or_else(|| panic!("no row \"{key}\" in section \"{title}\": {}", sec(r, title)))
}

fn kv(r: &Value, title: &str, key: &str) -> f64 {
    n(&row(r, title, key)[1])
}

fn close(a: f64, b: f64, rel: f64) {
    assert!(
        (a - b).abs() <= rel * b.abs().max(1e-300) + 1e-12,
        "{a} != {b} (rel {rel})"
    );
}

fn summary(r: &Value) -> String {
    r["summary"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn check<'a>(r: &'a Value, name: &str) -> &'a Value {
    r["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"].as_str().unwrap().starts_with(name))
        .unwrap_or_else(|| panic!("no check \"{name}\": {}", r["assumptions"]))
}

fn status(r: &Value, name: &str) -> String {
    check(r, name)["status"].as_str().unwrap().to_string()
}

fn common_parts(r: &Value) {
    assert!(
        r["methods"].as_str().unwrap().contains("Dioptra 0.1.0."),
        "{}",
        r["methods"]
    );
    assert!(r["checklist"].as_array().unwrap().len() >= 3);
    assert!(!r["assumptions"].as_array().unwrap().is_empty());
    assert!(!r["summary"].as_array().unwrap().is_empty());
}

fn tsv(cols: &[&[f64]]) -> String {
    let rows = cols.iter().map(|c| c.len()).max().unwrap();
    let mut out = String::new();
    for i in 0..rows {
        let line: Vec<String> = cols
            .iter()
            .map(|c| c.get(i).map_or(String::new(), |v| format!("{v}")))
            .collect();
        out.push_str(&line.join("\t"));
        out.push('\n');
    }
    out
}

const SLEEP_X: [f64; 10] = [0.7, -1.6, -0.2, -1.2, -0.1, 3.4, 3.7, 0.8, 0.0, 2.0];
const SLEEP_Y: [f64; 10] = [1.9, 0.8, 1.1, 0.1, -0.1, 4.4, 5.5, 1.6, 4.6, 3.4];
const PG: [[f64; 10]; 3] = [
    [4.17, 5.58, 5.18, 6.11, 4.50, 4.61, 5.17, 4.53, 5.33, 5.14],
    [4.81, 4.17, 4.41, 3.59, 5.87, 3.83, 6.03, 4.89, 4.32, 4.69],
    [6.31, 5.12, 5.54, 5.50, 5.37, 5.29, 4.92, 6.15, 5.80, 5.26],
];

fn plant(s: &mut Session) -> String {
    table(
        s,
        "column",
        3,
        json!({"kind": "values", "n": 1}),
        &format!("ctrl\ttrt1\ttrt2\n{}", tsv(&[&PG[0], &PG[1], &PG[2]])),
    )
}

fn sleep(s: &mut Session) -> String {
    table(
        s,
        "column",
        2,
        json!({"kind": "values", "n": 1}),
        &format!("drug1\tdrug2\n{}", tsv(&[&SLEEP_Y, &SLEEP_X])),
    )
}

// ---------------------------------------------------------------------------
// t tests
// ---------------------------------------------------------------------------

#[test]
fn paired_t_sleep_matches_fixture() {
    let mut s = Session::new();
    let t = sleep(&mut s);
    let (_, r) = analyze(&mut s, &t, json!({"kind": "t_test", "variant": "paired"}));
    common_parts(&r);
    has_sections(
        &r,
        &[
            "Descriptive",
            "Test",
            "Difference",
            "Effect size",
            "Normality",
            "Pairing",
        ],
    );
    // fixtures/oracle/t_tests.json paired/sleep_classic (x = drug2... here drug1 − drug2 = −(x − y))
    close(kv(&r, "Test", "t").abs(), 4.062127683382037, 1e-8);
    close(kv(&r, "Test", "df"), 9.0, 1e-12);
    close(
        kv(&r, "Test", "P value (two-sided)"),
        0.002832890197384273,
        1e-8,
    );
    let d = row(&r, "Difference", "Mean of");
    close(n(&d[1]), 1.58, 1e-10);
    close(n(&d[3]), 0.7001142367230175, 1e-8);
    close(n(&d[4]), 2.459885763276983, 1e-8);
    close(kv(&r, "Effect size", "Hedges"), 1.173924903514205, 1e-6);
    let sm = summary(&r);
    assert!(sm.contains("p = .003"), "{sm}");
    assert!(sm.contains("was 1.6 (95% CI 0.7 to 2.5)"), "{sm}");
    assert!(sm.contains("t(9) = 4.06"), "{sm}");
    assert!(["pass", "caution", "fail"]
        .contains(&status(&r, "Gaussian distribution of the paired differences").as_str()));
    assert_eq!(status(&r, "Independent"), "not_checked");
    assert!(r["methods"]
        .as_str()
        .unwrap()
        .starts_with("Paired t test, two-sided, 95% CI"));
}

#[test]
fn welch_and_student_match_stats() {
    let mut s = Session::new();
    let t = sleep(&mut s);
    let (_, r) = analyze(&mut s, &t, json!({"kind": "t_test"}));
    let direct = dioptra_stats::ttest::welch(&SLEEP_Y, &SLEEP_X, &Default::default()).unwrap();
    close(kv(&r, "Test", "t"), direct.t, 1e-12);
    close(kv(&r, "Test", "df"), direct.df, 1e-12);
    let sm = summary(&r);
    assert!(sm.contains(&format!("t({:.1})", direct.df)), "{sm}");
    assert!(
        sm.contains("The mean difference (drug1 − drug2) was 1.6 (95% CI"),
        "{sm}"
    );
    assert!(r["methods"]
        .as_str()
        .unwrap()
        .starts_with("Welch's unpaired t test, two-sided, 95% CI"));
    assert_eq!(
        status(&r, "Equal variances"),
        "pass",
        "informational under Welch"
    );
    // Student's test with a one-sided alternative states the direction.
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "t_test", "variant": "unpaired", "alternative": "greater"}),
    );
    let st = dioptra_stats::ttest::unpaired(&SLEEP_Y, &SLEEP_X, &Default::default()).unwrap();
    close(kv(&r, "Test", "t"), st.t, 1e-12);
    close(
        kv(&r, "Test", "P value (one-sided, H1: drug1 > drug2)"),
        st.p_greater,
        1e-12,
    );
    assert!(r["methods"]
        .as_str()
        .unwrap()
        .contains("one-sided (H1: drug1 > drug2)"));
    assert!(summary(&r).contains("one-sided p ="));
    // Alternatives are offered when a check is in doubt.
    for a in r["assumptions"].as_array().unwrap() {
        if a["status"] == "fail" || a["status"] == "caution" {
            assert!(
                a.get("alternative").is_some() || a["name"] == "Values used",
                "{a}"
            );
        }
    }
}

#[test]
fn t_tests_from_summary_data() {
    let mut s = Session::new();
    // Mean, SD, N of drug1 and drug2.
    let (m1, s1) = (2.33f64, 2.002f64);
    let (m2, s2) = (0.75f64, 1.789f64);
    let t = create(&mut s, "column", 2, json!({"kind": "mean_sd_n"}), 1);
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": format!("{m1}\t{s1}\t10\t{m2}\t{s2}\t10"), "options": {"header": false}}),
    );
    let (_, r) = analyze(&mut s, &t, json!({"kind": "t_test"}));
    // Welch from summary statistics.
    let (v1, v2) = (s1 * s1 / 10.0, s2 * s2 / 10.0);
    let tt = (m1 - m2) / (v1 + v2).sqrt();
    let df = (v1 + v2).powi(2) / (v1 * v1 / 9.0 + v2 * v2 / 9.0);
    close(kv(&r, "Test", "t"), tt, 1e-10);
    close(kv(&r, "Test", "df"), df, 1e-10);
    assert!(r["methods"]
        .as_str()
        .unwrap()
        .contains("computed from the entered mean, SD and N"));
    assert_eq!(
        status(&r, "Gaussian distribution of each group"),
        "not_checked"
    );
    // Not possible from summary data: Mann–Whitney says so.
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": {"kind": "mann_whitney"}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let r = ok(&mut s, json!({"type": "get_results", "id": a}));
    assert!(r["message"]
        .as_str()
        .unwrap()
        .contains("not possible from summary data"));
}

#[test]
fn one_sample_and_ratio_paired() {
    let mut s = Session::new();
    let t = sleep(&mut s);
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "t_test", "variant": "one_sample", "columns": [0], "mu": 1.0}),
    );
    let d = dioptra_stats::ttest::one_sample(&SLEEP_Y, 1.0, &Default::default()).unwrap();
    close(kv(&r, "Test", "t"), d.t, 1e-12);
    assert!(summary(&r).contains("differs from 1 by"));
    // Ratio paired on positive data (Hollander & Wolfe).
    let x = [1.83, 0.5, 1.62, 2.48, 1.68, 1.88, 1.55, 3.06, 1.3];
    let y = [0.878, 0.647, 0.598, 2.05, 1.06, 1.29, 1.06, 3.14, 1.29];
    let t2 = table(
        &mut s,
        "column",
        2,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&x, &y]),
    );
    let (_, r) = analyze(
        &mut s,
        &t2,
        json!({"kind": "t_test", "variant": "ratio_paired"}),
    );
    close(kv(&r, "Test", "t"), 2.439871844314116, 1e-8);
    let ratio = row(&r, "Ratio", "Geometric mean");
    close(n(&ratio[1]), 1.3744908777059444, 1e-8);
    close(n(&ratio[2]), 1.0176053642545027, 1e-8);
    assert!(summary(&r).contains("geometric mean ratio"));
}

// ---------------------------------------------------------------------------
// Rank tests
// ---------------------------------------------------------------------------

#[test]
fn mann_whitney_and_wilcoxon_match_stats() {
    let mut s = Session::new();
    let t = sleep(&mut s);
    let (_, r) = analyze(&mut s, &t, json!({"kind": "mann_whitney"}));
    common_parts(&r);
    has_sections(&r, &["Descriptive", "Test", "Difference", "Effect size"]);
    let d =
        dioptra_stats::mannwhitney::mann_whitney(&SLEEP_Y, &SLEEP_X, &Default::default()).unwrap();
    close(kv(&r, "Test", "U (Prism"), d.u, 1e-12);
    close(kv(&r, "Test", "P value (two-sided)"), d.p_two_sided, 1e-12);
    close(
        n(&row(&r, "Difference", "drug1")[1]),
        d.hodges_lehmann,
        1e-12,
    );
    let sm = summary(&r);
    assert!(sm.contains("Hodges–Lehmann shift (drug1 − drug2)"), "{sm}");
    assert!(sm.contains(&format!("U = {}", d.u)), "{sm}");
    // Wilcoxon, paired by row.
    let (_, r) = analyze(&mut s, &t, json!({"kind": "wilcoxon"}));
    let w = dioptra_stats::wilcoxon::paired(&SLEEP_Y, &SLEEP_X, &Default::default()).unwrap();
    close(kv(&r, "Test", "W+"), w.w_plus, 1e-12);
    close(kv(&r, "Test", "P value (two-sided)"), w.p_two_sided, 1e-12);
    close(
        n(&row(&r, "Pseudo-median", "drug1")[1]),
        w.hodges_lehmann,
        1e-12,
    );
    assert!(summary(&r).contains("W+ ="));
    check(&r, "Symmetric differences");
}

#[test]
fn kruskal_wallis_and_dunn_on_plantgrowth() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let (_, r) = analyze(&mut s, &t, json!({"kind": "kruskal_wallis"}));
    common_parts(&r);
    let refs: Vec<&[f64]> = PG.iter().map(|g| g.as_slice()).collect();
    let d = dioptra_stats::anova::kruskal_wallis(&refs).unwrap();
    close(kv(&r, "Test", "H"), d.h, 1e-12);
    close(kv(&r, "Test", "P value"), d.p, 1e-12);
    let dunn = sec(&r, "Dunn's multiple comparisons");
    assert_eq!(dunn["table"]["rows"].as_array().unwrap().len(), 3);
    assert!(summary(&r).contains("H(2) = "));
}

#[test]
fn friedman_and_rm_anova_match_stats() {
    let mut s = Session::new();
    let m = [
        [10.0, 12.0, 14.5],
        [8.0, 9.5, 12.0],
        [11.0, 13.0, 13.5],
        [9.0, 10.0, 12.5],
        [12.0, 15.0, 15.5],
        [7.5, 9.0, 11.0],
    ];
    let cols: Vec<Vec<f64>> = (0..3).map(|j| m.iter().map(|r| r[j]).collect()).collect();
    let t = table(
        &mut s,
        "column",
        3,
        json!({"kind": "values", "n": 1}),
        &format!("Pre\tMid\tPost\n{}", tsv(&[&cols[0], &cols[1], &cols[2]])),
    );
    let mm: Vec<Vec<f64>> = m.iter().map(|r| r.to_vec()).collect();
    let (_, r) = analyze(&mut s, &t, json!({"kind": "friedman"}));
    let d = dioptra_stats::anova::friedman(&mm).unwrap();
    close(kv(&r, "Test", "Friedman"), d.chi2, 1e-12);
    let (_, r) = analyze(&mut s, &t, json!({"kind": "repeated_measures_anova"}));
    common_parts(&r);
    has_sections(
        &r,
        &[
            "ANOVA table",
            "Corrected and uncorrected",
            "Sphericity",
            "Effect size",
            "Multiple comparisons (paired t tests",
        ],
    );
    let d = dioptra_stats::anova::rm_anova(&mm).unwrap();
    close(n(&row(&r, "ANOVA table", "Treatment")[4]), d.f, 1e-12);
    close(
        n(&row(&r, "Corrected and uncorrected", "Greenhouse")[5]),
        d.p_gg,
        1e-12,
    );
    close(
        n(&row(&r, "Corrected and uncorrected", "Greenhouse")[1]),
        d.gg_epsilon,
        1e-12,
    );
    check(&r, "Sphericity");
    assert!(summary(&r).contains("Greenhouse–Geisser corrected"));
}

// ---------------------------------------------------------------------------
// ANOVA
// ---------------------------------------------------------------------------

#[test]
fn one_way_anova_plantgrowth_with_tukey() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let (a, r) = analyze(&mut s, &t, json!({"kind": "one_way_anova"}));
    common_parts(&r);
    has_sections(
        &r,
        &[
            "Descriptive",
            "ANOVA table",
            "Effect size",
            "Tukey",
            "Normality",
            "Equal variances",
        ],
    );
    // fixtures/oracle/anova_oneway.json plantgrowth_classic
    close(
        n(&row(&r, "ANOVA table", "Treatment")[4]),
        4.846087862380138,
        1e-10,
    );
    close(
        n(&row(&r, "ANOVA table", "Treatment")[5]),
        0.01590995832562288,
        1e-8,
    );
    close(
        n(&row(&r, "ANOVA table", "Residual")[1]),
        10.492090000000001,
        1e-10,
    );
    close(
        n(&row(&r, "Effect size", "η²")[1]),
        0.2641482968321198,
        1e-10,
    );
    close(
        n(&row(&r, "Effect size", "ω²")[1]),
        0.20407884598997103,
        1e-8,
    );
    // fixtures/oracle/multiple_comparisons.json tukey_kramer/plantgrowth_classic
    let tk = sec(&r, "Tukey");
    let rows = tk["table"]["rows"].as_array().unwrap();
    let want = [0.390871144202107, 0.19799599129957068, 0.012006423979493142];
    for (row, p) in rows.iter().zip(want) {
        close(n(&row[6]), p, 1e-6);
    }
    close(n(&rows[2][2]), -1.556216051402862, 1e-6);
    let sm = summary(&r);
    assert!(sm.contains("F(2, 27) = 4.85, p = .016, η² = .26"), "{sm}");
    assert!(
        sm.contains("Tukey's multiple comparisons: trt1 vs. trt2"),
        "{sm}"
    );
    assert!(sm.contains("p = .012"), "{sm}");
    assert_eq!(status(&r, "Equal variances"), "pass");
    // A graph from the ANOVA carries one bracket per comparison, in the project style.
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g}));
    assert_eq!(spec["family"], "column");
    let ann = spec["annotations"].as_array().unwrap();
    assert_eq!(ann.len(), 3);
    assert!(ann.iter().all(|b| b["kind"] == "significance_bracket"));
    assert!(ann
        .iter()
        .any(|b| b["text"] == "p = .012" && b["from"] == 1 && b["to"] == 2));
    let svg = ok(&mut s, json!({"type": "render_graph", "id": g}));
    assert!(svg["svg"].as_str().unwrap().contains("p = .012"));
    // GP style gives asterisks.
    ok(&mut s, json!({"type": "set_p_value_style", "style": "gp"}));
    let g2 = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "box", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let spec = ok(&mut s, json!({"type": "get_graph_spec", "id": g2}));
    assert!(spec["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["text"] == "*"));
}

#[test]
fn one_way_dunnett_and_welch() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "one_way_anova", "welch": true, "post_hoc": {"method": {"name": "dunnett", "control": 0}}}),
    );
    // fixtures: dunnett/plantgrowth_classic
    let dn = sec(&r, "Dunnett");
    let rows = dn["table"]["rows"].as_array().unwrap();
    close(n(&rows[0][4]), -1.3307908011646807, 1e-7);
    close(n(&rows[1][6]), 0.15348586151509802, 1e-6);
    // Welch's ANOVA from the fixture.
    close(
        n(&row(&r, "Tests not assuming", "Welch")[1]),
        5.180972408113194,
        1e-8,
    );
    assert!(summary(&r).starts_with("Welch's ANOVA: F(2, 17.1) = 5.18, p = .017"));
}

#[test]
fn one_way_from_summary_matches_raw() {
    let mut s = Session::new();
    let t = create(&mut s, "column", 3, json!({"kind": "mean_sd_n"}), 1);
    let mut line = Vec::new();
    for g in PG {
        let m = g.iter().sum::<f64>() / 10.0;
        let sd = (g.iter().map(|x| (x - m).powi(2)).sum::<f64>() / 9.0).sqrt();
        line.push(format!("{m}\t{sd}\t10"));
    }
    ok(
        &mut s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": line.join("\t"), "options": {"header": false}}),
    );
    let (_, r) = analyze(&mut s, &t, json!({"kind": "one_way_anova"}));
    close(
        n(&row(&r, "ANOVA table", "Treatment")[4]),
        4.846087862380138,
        1e-9,
    );
    let tk = sec(&r, "Tukey");
    close(n(&tk["table"]["rows"][2][6]), 0.012006423979493142, 1e-6);
    assert_eq!(
        status(&r, "Gaussian distribution of the residuals"),
        "not_checked"
    );
}

#[test]
fn two_way_anova_matches_stats_with_simple_effects() {
    let mut s = Session::new();
    // Rows = factor A (2 levels), data sets = factor B (2), 3 replicates.
    let cells = [
        [[4.0, 5.0, 6.0], [7.0, 8.5, 9.0]],
        [[5.0, 5.5, 7.0], [12.0, 13.0, 11.5]],
    ];
    let mut text = String::new();
    for row in &cells {
        let v: Vec<String> = row.iter().flatten().map(|x| x.to_string()).collect();
        text.push_str(&v.join("\t"));
        text.push('\n');
    }
    let t = table(
        &mut s,
        "grouped",
        2,
        json!({"kind": "values", "n": 3}),
        &text,
    );
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "two_way_anova", "post_hoc": {"method": {"name": "sidak"}}}),
    );
    common_parts(&r);
    has_sections(
        &r,
        &[
            "ANOVA table (type III",
            "Cell means",
            "Multiple comparisons within each row",
            "Normality",
        ],
    );
    let c: Vec<Vec<Vec<f64>>> = cells
        .iter()
        .map(|r| r.iter().map(|c| c.to_vec()).collect())
        .collect();
    let d = dioptra_stats::anova::two_way_anova(&c, Default::default()).unwrap();
    for tm in &d.terms {
        let name = match tm.name.as_str() {
            "A" => "Row factor",
            "B" => "Column factor",
            _ => "Interaction",
        };
        close(n(&row(&r, "ANOVA table", name)[4]), tm.f, 1e-10);
        close(n(&row(&r, "ANOVA table", name)[5]), tm.p, 1e-10);
    }
    assert!(summary(&r).starts_with("Interaction: F(1, 8) = "));
    let se = sec(&r, "Multiple comparisons within each row");
    assert_eq!(se["table"]["rows"].as_array().unwrap().len(), 2);
    // Type I on request.
    let (_, r1) = analyze(&mut s, &t, json!({"kind": "two_way_anova", "ss_type": "i"}));
    sec(&r1, "ANOVA table (type I");
}

#[test]
fn two_way_repeated_matches_one_way_rm() {
    let mut s = Session::new();
    // One data set, four subjects (subcolumns), three repeated rows.
    let m = [
        [10.0, 12.0, 14.5],
        [8.0, 9.5, 12.0],
        [11.0, 13.0, 13.5],
        [9.0, 10.0, 12.5],
    ];
    let mut text = String::new();
    for w in 0..3 {
        let v: Vec<String> = m.iter().map(|sbj| sbj[w].to_string()).collect();
        text.push_str(&v.join("\t"));
        text.push('\n');
    }
    let t = table(
        &mut s,
        "grouped",
        1,
        json!({"kind": "values", "n": 4}),
        &text,
    );
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "two_way_anova", "repeated": "columns"}),
    );
    let mm: Vec<Vec<f64>> = m.iter().map(|r| r.to_vec()).collect();
    let d = dioptra_stats::anova::rm_anova(&mm).unwrap();
    close(
        n(&row(&r, "ANOVA table (split-plot)", "Row factor (repeated)")[4]),
        d.f,
        1e-10,
    );
    check(&r, "Sphericity");
}

// ---------------------------------------------------------------------------
// Contingency
// ---------------------------------------------------------------------------

#[test]
fn fisher_and_chi_square() {
    let mut s = Session::new();
    let t = table(
        &mut s,
        "contingency",
        2,
        json!({"kind": "single"}),
        "3\t1\n1\t3\n",
    );
    let (_, r) = analyze(&mut s, &t, json!({"kind": "fisher_exact"}));
    common_parts(&r);
    // fixtures/oracle/contingency.json fisher_2x2/tea_tasting_classic
    close(
        kv(&r, "Test", "P value (two-sided)"),
        0.48571428571428565,
        1e-8,
    );
    close(
        kv(&r, "Test", "Odds ratio (conditional MLE)"),
        6.408319658199662,
        1e-6,
    );
    close(n(&row(&r, "Effect size", "Relative risk")[1]), 3.0, 1e-12);
    close(
        n(&row(&r, "Effect size", "Relative risk")[3]),
        17.953883318244678,
        1e-8,
    );
    assert!(summary(&r).contains("p = .486"));
    // Chi-square on fisher_2x2/moderate: small expected counts are not an issue there.
    let t2 = table(
        &mut s,
        "contingency",
        2,
        json!({"kind": "single"}),
        "12\t5\n7\t15\n",
    );
    let (_, r) = analyze(&mut s, &t2, json!({"kind": "chi_square", "yates": true}));
    has_sections(
        &r,
        &[
            "Observed counts",
            "Expected counts",
            "Test",
            "Association",
            "Effect size",
            "Adjusted residuals",
        ],
    );
    let d =
        dioptra_stats::contingency::chi_square_independence(&[vec![12, 5], vec![7, 15]]).unwrap();
    close(
        n(&row(&r, "Test", "Yates")[1]),
        d.yates.as_ref().unwrap().statistic,
        1e-12,
    );
    close(n(&row(&r, "Test", "Fisher")[3]), 0.024841720377823295, 1e-8);
    assert_eq!(status(&r, "Expected counts"), "pass");
    let sm = summary(&r);
    assert!(sm.contains("relative risk 2.2 (95% CI 1.1 to 4.4)"), "{sm}");
    assert!(sm.contains("(Yates-corrected)"));
    // Small expected counts suggest Fisher.
    let (_, r) = analyze(&mut s, &t, json!({"kind": "chi_square"}));
    assert_ne!(status(&r, "Expected counts"), "pass");
    assert!(check(&r, "Expected counts")["alternative"]
        .as_str()
        .unwrap()
        .contains("Fisher"));
    // r x c Fisher.
    let t3 = table(
        &mut s,
        "contingency",
        3,
        json!({"kind": "single"}),
        "2\t0\t3\n1\t4\t0\n0\t2\t5\n",
    );
    let (_, r) = analyze(&mut s, &t3, json!({"kind": "fisher_exact"}));
    close(
        kv(&r, "Test", "P value (two-sided, exact)"),
        0.016659810777457717,
        1e-7,
    );
}

#[test]
fn contingency_counts_are_validated() {
    let mut s = Session::new();
    let t = table(
        &mut s,
        "contingency",
        2,
        json!({"kind": "single"}),
        "3\t1\n1.23\t3\n",
    );
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": {"kind": "chi_square"}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let r = ok(&mut s, json!({"type": "get_results", "id": a}));
    let m = r["message"].as_str().unwrap();
    assert!(
        m.contains("A2") && m.contains("1.23") && m.contains("whole numbers"),
        "{m}"
    );
    // The table was not rewritten.
    assert_eq!(cells(&mut s, &t, 1, 2, 0, 1), vec![vec!["1.23"]]);
}

#[test]
fn chi_square_goodness_of_fit_on_parts() {
    let mut s = Session::new();
    let t = table(
        &mut s,
        "parts",
        1,
        json!({"kind": "single"}),
        "30\n20\n10\n",
    );
    let (_, r) = analyze(&mut s, &t, json!({"kind": "chi_square"}));
    let d = dioptra_stats::contingency::chi_square_gof(&[30, 20, 10], None, 0).unwrap();
    close(kv(&r, "Test", "χ²"), d.chi2, 1e-12);
}

// ---------------------------------------------------------------------------
// Correlation and regression
// ---------------------------------------------------------------------------

const RX: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
const RY: [f64; 10] = [2.1, 3.9, 6.2, 7.8, 10.1, 12.2, 13.8, 16.1, 18.3, 19.7];

#[test]
fn correlation_matches_stats() {
    let mut s = Session::new();
    let t = table(
        &mut s,
        "xy",
        1,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&RX, &RY]),
    );
    let (a, r) = analyze(&mut s, &t, json!({"kind": "correlation"}));
    common_parts(&r);
    let d = dioptra_stats::correlation::pearson(&RX, &RY, 0.95).unwrap();
    let rr = &sec(&r, "Correlation")["table"]["rows"][0];
    close(n(&rr[2]), d.estimate, 1e-12);
    close(n(&rr[3]), d.ci.unwrap().0, 1e-12);
    assert!(summary(&r).contains("r(8) = "));
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "correlation", "method": "spearman"}),
    );
    let sp = dioptra_stats::correlation::spearman(&RX, &RY, 0.95).unwrap();
    close(
        n(&sec(&r, "Correlation")["table"]["rows"][0][2]),
        sp.rho,
        1e-12,
    );
    // Graph of a two-variable correlation: a scatter of the pairs.
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "auto", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "scatter"
    );
    assert_eq!(
        ok(&mut s, json!({"type": "render_graph", "id": g}))["svg"]
            .as_str()
            .unwrap()
            .matches("<circle")
            .count(),
        10
    );
}

#[test]
fn linear_regression_with_band_curve() {
    let mut s = Session::new();
    let t = table(
        &mut s,
        "xy",
        1,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&RX, &RY]),
    );
    let (a, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "linear_regression", "bands": "confidence", "residuals": true}),
    );
    common_parts(&r);
    has_sections(
        &r,
        &[
            "Best-fit values",
            "Goodness of fit",
            "Runs test",
            "Normality",
            "Residuals",
        ],
    );
    let d =
        dioptra_stats::regression::simple_linear_regression(&RX, &RY, Default::default()).unwrap();
    close(
        n(&row(&r, "Best-fit values", "Data set A")[2]),
        d.slope,
        1e-12,
    );
    close(
        n(&sec(&r, "Goodness of fit")["table"]["rows"][0][2]),
        d.r_squared,
        1e-12,
    );
    check(&r, "Linearity (runs test)");
    let sm = summary(&r);
    assert!(sm.contains("slope 1.99 (95% CI 1.94 to 2.05)"), "{sm}");
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let svg = ok(&mut s, json!({"type": "render_graph", "id": g}))["svg"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(svg.contains("id=\"curve-0\""), "the fit line is drawn");
}

#[test]
fn nonlinear_4pl_matches_fixture_and_graphs_a_curve() {
    let mut s = Session::new();
    // fixtures/oracle/nonlinear_regression.json fit/4pl_dose_response: duplicates per X.
    let x = [
        -9.0, -8.5, -8.0, -7.5, -7.0, -6.5, -6.0, -5.5, -5.0, -4.5, -4.0,
    ];
    let y = [
        [4.532, 9.042],
        [7.551, 7.627],
        [8.299, 14.287],
        [17.455, 11.673],
        [31.319, 29.776],
        [51.44, 49.821],
        [63.554, 71.927],
        [79.565, 78.704],
        [87.146, 91.23],
        [93.896, 95.299],
        [94.997, 96.18],
    ];
    let mut text = String::new();
    for (xi, yi) in x.iter().zip(&y) {
        text.push_str(&format!("{xi}\t{}\t{}\n", yi[0], yi[1]));
    }
    let t = table(&mut s, "xy", 1, json!({"kind": "values", "n": 2}), &text);
    let (a, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "nonlinear_regression", "equation": {"name": "four_parameter_logistic"}, "compare_with": {"name": "log_agonist_vs_response"}}),
    );
    common_parts(&r);
    has_sections(
        &r,
        &[
            "Best-fit values",
            "Derived values",
            "Goodness of fit",
            "Convergence",
            "Runs test",
            "Replicates test",
            "Model comparison",
            "Normality",
        ],
    );
    let want = [
        4.784013496601509,
        96.55801952611922,
        -6.446128957436861,
        0.748323715910514,
    ];
    let se = [
        1.633311572400767,
        1.7226034965714268,
        0.04516209752742231,
        0.0575949273749959,
    ];
    for (i, name) in ["Bottom", "Top", "LogEC50", "HillSlope"].iter().enumerate() {
        let rr = sec(&r, "Best-fit values")["table"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row[1]["text"] == *name)
            .unwrap()
            .clone();
        close(n(&rr[2]), want[i], 1e-6);
        close(n(&rr[3]), se[i], 1e-5);
    }
    let ec50 = sec(&r, "Derived values")["table"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row[1]["text"] == "EC50")
        .unwrap()
        .clone();
    close(n(&ec50[2]), 10f64.powf(want[2]), 1e-6);
    close(
        n(&sec(&r, "Goodness of fit")["table"]["rows"][0][3]),
        0.9949507329037858,
        1e-8,
    );
    let sm = summary(&r);
    assert!(
        sm.contains("LogEC50 = -6.45 (95% CI -6.54 to -6.35)"),
        "{sm}"
    );
    assert!(sm.contains("EC50 = "), "{sm}");
    assert!(sm.contains("Model comparison"), "{sm}");
    assert_eq!(status(&r, "Convergence"), "pass");
    // A graph from the fit carries the curve (and its band).
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "scatter"
    );
    let svg = ok(&mut s, json!({"type": "render_graph", "id": g}))["svg"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(svg.contains("id=\"curve-0\""));
    // ROUT flags a planted outlier.
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 5, "col": 1, "input": "90"}),
    );
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "nonlinear_regression", "equation": {"name": "4pl"}, "rout_q": 1.0}),
    );
    let out = sec(&r, "Outliers (ROUT");
    assert_eq!(out["table"]["rows"].as_array().unwrap().len(), 1);
    close(n(&out["table"]["rows"][0][2]), 90.0, 1e-12);
}

#[test]
fn nonlinear_constraints_and_custom_formula() {
    let mut s = Session::new();
    let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|t| {
            100.0 * (-0.3 * t).exp()
                + 5.0
                + if (*t as usize).is_multiple_of(2) {
                    0.4
                } else {
                    -0.4
                }
        })
        .collect();
    let t = table(
        &mut s,
        "xy",
        1,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&x, &y]),
    );
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "nonlinear_regression", "equation": {"name": "one_phase_decay"},
               "constraints": [{"kind": "fixed", "param": "Plateau", "value": 5.0}]}),
    );
    let pl = sec(&r, "Best-fit values")["table"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row[1]["text"] == "Plateau")
        .unwrap()
        .clone();
    assert_eq!(pl[6]["text"], "Fixed");
    assert!(summary(&r).contains("Plateau fixed at 5"));
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "nonlinear_regression", "equation": {"name": "decay", "formula": "Y = A*exp(-K*X) + C"},
               "initial": {"A": 90.0, "K": 0.2, "C": 1.0}}),
    );
    let k = sec(&r, "Best-fit values")["table"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row[1]["text"].as_str().unwrap().eq_ignore_ascii_case("K"))
        .unwrap()
        .clone();
    close(n(&k[2]), 0.3, 0.02);
    // Unknown parameters are named.
    let a = ok(
        &mut s,
        json!({"type": "create_analysis", "table": t, "spec": {"kind": "nonlinear_regression", "equation": {"name": "one_phase_decay"}, "constraints": [{"kind": "fixed", "param": "Nope", "value": 1.0}]}}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let m = ok(&mut s, json!({"type": "get_results", "id": a}))["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(m.contains("Nope") && m.contains("Plateau"), "{m}");
}

// ---------------------------------------------------------------------------
// Normality and outliers
// ---------------------------------------------------------------------------

#[test]
fn normality_and_outliers() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let (a, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "normality", "tests": ["shapiro_wilk", "dagostino_pearson", "anderson_darling", "kolmogorov_smirnov"]}),
    );
    common_parts(&r);
    let sw = dioptra_stats::normality::shapiro_wilk(&PG[0]).unwrap();
    let rows = sec(&r, "Normality tests")["table"]["rows"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(rows.len(), 12);
    close(n(&rows[0][4]), sw.w, 1e-12);
    close(n(&rows[0][5]), sw.p, 1e-12);
    assert!(summary(&r).contains("Shapiro–Wilk W = "));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "qq"
    );
    ok(&mut s, json!({"type": "render_graph", "id": g}));
    // Outliers: a planted extreme value is flagged with its row, not removed.
    let v = [10.1, 9.8, 10.3, 10.0, 9.9, 10.2, 25.0, 10.1];
    let t2 = table(
        &mut s,
        "column",
        1,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&v]),
    );
    for method in [
        json!({"method": "grubbs", "alpha": 0.05}),
        json!({"method": "rout", "q": 1.0}),
    ] {
        let (_, r) = analyze(&mut s, &t2, json!({"kind": "outliers", "method": method}));
        let fl = sec(&r, "Flagged values")["table"]["rows"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(fl.len(), 1);
        close(n(&fl[0][1]), 7.0, 1e-12);
        close(n(&fl[0][3]), 25.0, 1e-12);
        assert!(summary(&r).contains("(row 7)"));
        assert_eq!(status(&r, "Decision to exclude"), "caution");
    }
    assert_eq!(cells(&mut s, &t2, 6, 7, 0, 1), vec![vec!["25"]]);
}

// ---------------------------------------------------------------------------
// Survival, ROC, Bland–Altman, PCA
// ---------------------------------------------------------------------------

const G1T: [f64; 21] = [
    6., 6., 6., 6., 7., 9., 10., 10., 11., 13., 16., 17., 19., 20., 22., 23., 25., 32., 32., 34.,
    35.,
];
const G1E: [u8; 21] = [
    1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0,
];
const G2T: [f64; 21] = [
    1., 1., 2., 2., 3., 4., 4., 5., 5., 8., 8., 8., 8., 11., 11., 12., 12., 15., 17., 22., 23.,
];

fn gehan(s: &mut Session) -> String {
    let mut text = String::new();
    for (t, e) in G1T.iter().zip(G1E) {
        text.push_str(&format!("{t}\t{e}\t\n"));
    }
    for t in G2T {
        text.push_str(&format!("{t}\t\t1\n"));
    }
    let t = create(s, "survival", 2, json!({"kind": "single"}), 0);
    ok(
        s,
        json!({"type": "paste", "table": t, "row0": 0, "col0": 0, "text": text, "options": {"header": false}}),
    );
    ok(
        s,
        json!({"type": "set_dataset_name", "table": t, "dataset": 0, "name": "6-MP"}),
    );
    ok(
        s,
        json!({"type": "set_dataset_name", "table": t, "dataset": 1, "name": "Placebo"}),
    );
    t
}

#[test]
fn survival_gehan_leukaemia() {
    let mut s = Session::new();
    let t = gehan(&mut s);
    let (a, r) = analyze(&mut s, &t, json!({"kind": "survival"}));
    common_parts(&r);
    has_sections(
        &r,
        &[
            "Kaplan–Meier: ",
            "Median survival",
            "Curve comparison",
            "Observed and expected",
            "Hazard ratio",
        ],
    );
    let e1: Vec<bool> = G1E.iter().map(|e| *e == 1).collect();
    let e2 = vec![true; 21];
    let lr = dioptra_stats::survival::logrank(
        &[(&G1T, &e1), (&G2T, &e2)],
        dioptra_stats::survival::LogRankWeight::LogRank,
        None,
    )
    .unwrap();
    close(
        n(&row(&r, "Curve comparison", "Log-rank (Mantel")[1]),
        lr.chi2,
        1e-12,
    );
    let km = dioptra_stats::survival::kaplan_meier(&G1T, &e1, Default::default()).unwrap();
    close(n(&row(&r, "Median survival", "6-MP")[3]), km.median, 1e-12);
    let first = &sec(&r, "Kaplan–Meier: 6-MP")["table"]["rows"][0];
    close(n(&first[4]), 100.0 * 0.8571428571428572, 1e-10);
    close(n(&first[6]), 100.0 * 0.6197179552659837, 1e-8);
    let sm = summary(&r);
    assert!(sm.contains("Median survival was 23 in 6-MP"), "{sm}");
    assert!(sm.contains("Log-rank (Mantel–Cox) χ²(1) = "), "{sm}");
    assert!(
        sm.contains("hazard ratio (6-MP / Placebo, Mantel–Haenszel)"),
        "{sm}"
    );
    assert_eq!(status(&r, "Proportional hazards"), "not_checked");
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "kaplan_meier"
    );
    ok(&mut s, json!({"type": "render_graph", "id": g}));
    // A bad event code is named.
    ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 1, "input": "2"}),
    );
    let m = ok(&mut s, json!({"type": "get_results", "id": a}))["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        m.contains("A1") && m.contains("1 (event) or 0 (censored)"),
        "{m}"
    );
}

#[test]
fn roc_bland_altman_pca() {
    let mut s = Session::new();
    let ctrl = [1.2, 2.3, 1.8, 2.9, 3.1, 2.2, 1.5, 2.8, 3.5, 2.0];
    let pat = [3.4, 4.1, 2.7, 5.0, 4.4, 3.9, 2.5, 4.8, 3.3, 5.2];
    let t = table(
        &mut s,
        "column",
        2,
        json!({"kind": "values", "n": 1}),
        &format!("Controls\tPatients\n{}", tsv(&[&ctrl, &pat])),
    );
    let (a, r) = analyze(&mut s, &t, json!({"kind": "roc"}));
    common_parts(&r);
    let d = dioptra_stats::roc::roc(&pat, &ctrl, Default::default()).unwrap();
    close(
        n(&row(&r, "Area under the curve", "AUC (DeLong)")[1]),
        d.auc,
        1e-12,
    );
    close(
        n(&row(&r, "Area under the curve", "AUC (DeLong)")[3]),
        d.ci_delong.0,
        1e-12,
    );
    assert!(summary(&r).contains("Youden cut-off"));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "roc"
    );
    ok(&mut s, json!({"type": "render_graph", "id": g}));
    // Bland–Altman of the same two columns as methods.
    let (b, r) = analyze(&mut s, &t, json!({"kind": "bland_altman"}));
    let ba = dioptra_stats::agreement::bland_altman(
        &ctrl,
        &pat,
        dioptra_stats::agreement::BaKind::Difference,
        0.95,
    )
    .unwrap();
    close(n(&row(&r, "Bias and limits", "Bias")[1]), ba.bias, 1e-12);
    close(
        n(&row(&r, "Bias and limits", "Upper limit")[1]),
        ba.loa_upper,
        1e-12,
    );
    assert!(summary(&r).contains("limits of agreement from"));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": b, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(&mut s, json!({"type": "get_graph_spec", "id": g}))["family"],
        "bland_altman"
    );
    ok(&mut s, json!({"type": "render_graph", "id": g}));
    let (_, r) = analyze(
        &mut s,
        &t,
        json!({"kind": "bland_altman", "method": "ratio"}),
    );
    let ba = dioptra_stats::agreement::bland_altman(
        &ctrl,
        &pat,
        dioptra_stats::agreement::BaKind::Ratio,
        0.95,
    )
    .unwrap();
    close(n(&row(&r, "Bias and limits", "Bias")[1]), ba.bias, 1e-12);
    // PCA on a variables table.
    let v1 = [2.5, 0.5, 2.2, 1.9, 3.1, 2.3, 2.0, 1.0, 1.5, 1.1];
    let v2 = [2.4, 0.7, 2.9, 2.2, 3.0, 2.7, 1.6, 1.1, 1.6, 0.9];
    let v3 = [1.0, 3.2, 0.8, 1.5, 0.2, 0.9, 1.4, 2.8, 2.0, 2.9];
    let tv = table(
        &mut s,
        "variables",
        3,
        json!({"kind": "single"}),
        &format!("a\tb\tc\n{}", tsv(&[&v1, &v2, &v3])),
    );
    let (p, r) = analyze(&mut s, &tv, json!({"kind": "pca"}));
    common_parts(&r);
    has_sections(&r, &["Eigenvalues", "Loadings", "Eigenvectors", "Scores"]);
    let rows: Vec<Vec<f64>> = (0..10).map(|i| vec![v1[i], v2[i], v3[i]]).collect();
    let d = dioptra_stats::multivariate::pca(&rows, Default::default()).unwrap();
    close(
        n(&row(&r, "Eigenvalues", "PC1")[1]),
        d.eigenvalues[0],
        1e-10,
    );
    assert!(summary(&r).contains("Kaiser's rule"));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": p, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    ok(&mut s, json!({"type": "render_graph", "id": g}));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": p, "family": "column", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    ok(&mut s, json!({"type": "render_graph", "id": g}));
}

// ---------------------------------------------------------------------------
// Staleness, listing, round trip
// ---------------------------------------------------------------------------

#[test]
fn every_kind_is_implemented() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let l = ok(
        &mut s,
        json!({"type": "list_analyses", "table": t, "range": null}),
    );
    for o in l["analyses"].as_array().unwrap() {
        assert_eq!(o["implemented"], true, "{o}");
    }
}

#[test]
fn edits_make_analyses_and_graphs_stale() {
    let mut s = Session::new();
    let t = plant(&mut s);
    let (a, r) = analyze(&mut s, &t, json!({"kind": "one_way_anova"}));
    let g = ok(
        &mut s,
        json!({"type": "create_graph", "source": a, "family": "", "title": null}),
    )["changed"]["created"]
        .as_str()
        .unwrap()
        .to_string();
    let f0 = n(&row(&r, "ANOVA table", "Treatment")[4]);
    let ch = ok(
        &mut s,
        json!({"type": "set_cell", "table": t, "row": 0, "col": 2, "input": "9.5"}),
    );
    let ids: Vec<String> = ch["changed"]["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&a) && ids.contains(&g), "{ids:?}");
    let p = ok(&mut s, json!({"type": "get_project"}));
    let item = |id: &str| {
        p["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(item(&a)["stale"], true);
    let r2 = ok(&mut s, json!({"type": "get_results", "id": a}));
    let f1 = n(&row(&r2, "ANOVA table", "Treatment")[4]);
    assert!((f1 - f0).abs() > 1e-6, "results follow the edit");
    let mut g2 = PG;
    g2[2][0] = 9.5;
    let refs: Vec<&[f64]> = g2.iter().map(|g| g.as_slice()).collect();
    close(
        f1,
        dioptra_stats::anova::one_way_anova(&refs).unwrap().f,
        1e-12,
    );
    ok(&mut s, json!({"type": "render_graph", "id": g}));
}

#[test]
fn project_round_trip_with_every_kind() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    let col = plant(&mut s);
    let pair = sleep(&mut s);
    let xy = table(
        &mut s,
        "xy",
        1,
        json!({"kind": "values", "n": 1}),
        &tsv(&[&RX, &RY]),
    );
    let grp = table(
        &mut s,
        "grouped",
        2,
        json!({"kind": "values", "n": 2}),
        "4\t5\t7\t8\n5\t6\t12\t13\n6\t5\t9\t10\n",
    );
    let ct = table(
        &mut s,
        "contingency",
        2,
        json!({"kind": "single"}),
        "12\t5\n7\t15\n",
    );
    let sv = gehan(&mut s);
    let vars = table(
        &mut s,
        "variables",
        3,
        json!({"kind": "single"}),
        &format!(
            "a\tb\tc\n{}",
            tsv(&[
                &RX,
                &RY,
                &[3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0]
            ])
        ),
    );
    let specs = [
        (&col, json!({"kind": "descriptive"})),
        (&pair, json!({"kind": "t_test"})),
        (&pair, json!({"kind": "mann_whitney"})),
        (&pair, json!({"kind": "wilcoxon"})),
        (&col, json!({"kind": "one_way_anova"})),
        (&col, json!({"kind": "repeated_measures_anova"})),
        (&grp, json!({"kind": "two_way_anova"})),
        (&col, json!({"kind": "kruskal_wallis"})),
        (&col, json!({"kind": "friedman"})),
        (&ct, json!({"kind": "chi_square"})),
        (&ct, json!({"kind": "fisher_exact"})),
        (&xy, json!({"kind": "correlation"})),
        (&xy, json!({"kind": "linear_regression"})),
        (
            &xy,
            json!({"kind": "nonlinear_regression", "equation": {"name": "straight_line"}}),
        ),
        (&col, json!({"kind": "normality"})),
        (&col, json!({"kind": "outliers"})),
        (&sv, json!({"kind": "survival"})),
        (&pair, json!({"kind": "roc"})),
        (&pair, json!({"kind": "bland_altman"})),
        (&vars, json!({"kind": "pca"})),
    ];
    let mut done = Vec::new();
    for (t, spec) in specs {
        let (a, r) = analyze(&mut s, t, spec.clone());
        assert_eq!(r["analysis_kind"], spec["kind"]);
        done.push((a, r));
    }
    assert_eq!(done.len(), dioptra_core::analysis::KINDS.len());
    let b64 = ok(&mut s, json!({"type": "save_project_bytes"}))["base64"]
        .as_str()
        .unwrap()
        .to_string();
    let mut s2 = Session::new();
    ok(
        &mut s2,
        json!({"type": "open_project_bytes", "base64": b64, "name": "all.dioptra"}),
    );
    for (a, r) in &done {
        let r2 = ok(&mut s2, json!({"type": "get_results", "id": a}));
        assert_eq!(
            r, &r2,
            "results of {} differ after reopening",
            r["analysis_kind"]
        );
    }
    // Recompute reproduces every number.
    ok(&mut s2, json!({"type": "recompute_all"}));
    for (a, r) in &done {
        let r2 = ok(&mut s2, json!({"type": "get_results", "id": a}));
        assert_eq!(r["sections"], r2["sections"], "{}", r["analysis_kind"]);
    }
}
