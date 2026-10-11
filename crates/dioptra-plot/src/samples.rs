//! Deterministic sample data for every family: the snapshot tests, the gallery example
//! and the documentation all draw these. Values come from a fixed-seed generator, so
//! they are identical on every platform.

use crate::data::{Curve, GraphData, Series, TableKind};
use crate::detmath::DetMath;
use crate::spec::{Annotation, GraphSpec};
use crate::stats::Rng;

struct Gen(Rng);

impl Gen {
    fn new(seed: u64) -> Self {
        Gen(Rng::new(seed))
    }
    /// Standard normal by Box–Muller.
    fn normal(&mut self) -> f64 {
        let u1 = self.0.uniform().max(1e-12);
        let u2 = self.0.uniform();
        (-2.0 * u1.ln_d()).sqrt() * (std::f64::consts::TAU * u2).cos_d()
    }
    fn norm(&mut self, m: f64, s: f64) -> f64 {
        m + s * self.normal()
    }
    fn round(v: f64, d: i32) -> f64 {
        let f = 10f64.powi(d);
        (v * f).round() / f
    }
    fn sample(&mut self, n: usize, m: f64, s: f64) -> Vec<f64> {
        (0..n).map(|_| Gen::round(self.norm(m, s), 2)).collect()
    }
}

fn column(names: &[&str], groups: Vec<Vec<f64>>, y: &str) -> GraphData {
    GraphData {
        series: names
            .iter()
            .zip(groups)
            .enumerate()
            .map(|(i, (n, v))| Series::category(n, i, i as f64 + 1.0, v))
            .collect(),
        categories: names.iter().map(|s| s.to_string()).collect(),
        y_label: y.into(),
        kind: TableKind::Column,
        ..Default::default()
    }
}

/// Sample spec and data for a family (`None` for an unknown family).
pub fn sample(family: &str) -> Option<(GraphSpec, GraphData)> {
    let mut g = Gen::new(42);
    let mut spec = GraphSpec::new("sample".into(), family);
    let data = match family {
        "scatter" | "line" => {
            let xs: Vec<f64> = (0..9).map(|i| i as f64 * 2.5).collect();
            let mut series = Vec::new();
            for (k, (name, a, b)) in [("Wild type", 1.2, 0.42), ("Knock-out", 0.9, 0.21)]
                .iter()
                .enumerate()
            {
                let reps: Vec<Vec<f64>> = xs
                    .iter()
                    .map(|x| {
                        (0..3)
                            .map(|_| Gen::round(a + b * x + g.norm(0.0, 0.45), 2))
                            .collect()
                    })
                    .collect();
                let y = reps.iter().map(|r| crate::stats::mean(r)).collect();
                series.push(Series {
                    name: name.to_string(),
                    dataset: k,
                    x: xs.clone(),
                    y,
                    replicates: reps,
                    ..Default::default()
                });
            }
            let fx: Vec<f64> = (0..=40).map(|i| i as f64 * 0.5).collect();
            let curves = if family == "scatter" {
                vec![Curve {
                    name: "Linear fit".into(),
                    dataset: Some(0),
                    y: fx.iter().map(|x| 1.2 + 0.42 * x).collect(),
                    lo: fx
                        .iter()
                        .map(|x| 1.2 + 0.42 * x - 0.35 - 0.01 * (x - 10.0).powi(2))
                        .collect(),
                    hi: fx
                        .iter()
                        .map(|x| 1.2 + 0.42 * x + 0.35 + 0.01 * (x - 10.0).powi(2))
                        .collect(),
                    x: fx,
                    step: false,
                    censor: vec![],
                }]
            } else {
                vec![]
            };
            spec.axes.x.title = Some("Time (min)".into());
            spec.axes.y.title = Some("[Ca^{2+}]_i (\\mu M)".into());
            GraphData {
                series,
                curves,
                x_label: "Time (min)".into(),
                y_label: "Response".into(),
                kind: TableKind::Xy,
                ..Default::default()
            }
        }
        "column" | "box" | "violin" | "strip" | "raincloud" => {
            let names = ["Vehicle", "Low dose", "High dose"];
            let groups = vec![g.sample(12, 10.0, 2.0), g.sample(12, 13.5, 2.4), {
                let mut v = g.sample(11, 17.0, 2.0);
                v.push(26.4);
                v
            }];
            if family == "column" {
                spec.annotations = vec![
                    Annotation::SignificanceBracket {
                        from: 0,
                        to: 1,
                        text: "p = 0.003".into(),
                        y: None,
                    },
                    Annotation::SignificanceBracket {
                        from: 0,
                        to: 2,
                        text: "p < 0.001".into(),
                        y: None,
                    },
                ];
            }
            if family == "raincloud" {
                let mut d = column(&names, groups, "Latency (ms)");
                // A bimodal group shows what the cloud is for.
                d.series[1] = Series::category("Low dose", 1, 2.0, {
                    let mut v = g.sample(20, 11.0, 1.2);
                    v.extend(g.sample(20, 16.5, 1.3));
                    v
                });
                return Some((spec, d));
            }
            column(&names, groups, "Latency (ms)")
        }
        "column_grouped" => {
            spec.family = "column".into();
            let rows = ["24 h", "48 h", "72 h"];
            let mut series = Vec::new();
            for (k, (name, base)) in [("Control", 1.0), ("siRNA", 0.6)].iter().enumerate() {
                let reps: Vec<Vec<f64>> = (0..3)
                    .map(|r| g.sample(4, base * (1.0 + r as f64 * 0.8), 0.15 * (1.0 + r as f64)))
                    .collect();
                series.push(Series {
                    name: name.to_string(),
                    dataset: k,
                    x: vec![1.0, 2.0, 3.0],
                    y: reps.iter().map(|r| crate::stats::mean(r)).collect(),
                    replicates: reps,
                    ..Default::default()
                });
            }
            GraphData {
                series,
                categories: rows.iter().map(|s| s.to_string()).collect(),
                x_label: "Time after transfection".into(),
                y_label: "Relative expression".into(),
                kind: TableKind::Grouped,
                ..Default::default()
            }
        }
        "before_after" => {
            let before = g.sample(10, 120.0, 12.0);
            let after: Vec<f64> = before
                .iter()
                .map(|b| Gen::round(b - 9.0 + g.norm(0.0, 6.0), 1))
                .collect();
            spec.axes.y.title = Some("Systolic pressure (mmHg)".into());
            column(
                &["Before", "After"],
                vec![before, after],
                "Systolic pressure (mmHg)",
            )
        }
        "estimation" => {
            let c = g.sample(20, 52.0, 7.0);
            let t = g.sample(20, 59.0, 8.0);
            column(&["Control", "Treated"], vec![c, t], "Grip strength (N)")
        }
        "superplot" => {
            let mut series = Vec::new();
            for (k, (name, m)) in [("Control", 18.0), ("Drug", 22.0)].iter().enumerate() {
                let mut vals = Vec::new();
                let mut groups = Vec::new();
                for rep in 0..3 {
                    let shift = g.norm(0.0, 1.8);
                    for _ in 0..25 {
                        vals.push(Gen::round(m + shift + g.norm(0.0, 3.0), 2));
                        groups.push(rep);
                    }
                }
                let mut s = Series::category(name, k, k as f64 + 1.0, vals);
                s.groups = groups;
                series.push(s);
            }
            GraphData {
                series,
                categories: vec!["Control".into(), "Drug".into()],
                group_names: vec![
                    "Replicate 1".into(),
                    "Replicate 2".into(),
                    "Replicate 3".into(),
                ],
                y_label: "Cell speed (\\mu m/min)".into(),
                kind: TableKind::Nested,
                ..Default::default()
            }
        }
        "histogram" | "density" => {
            let a = g.sample(400, 5.2, 1.0);
            let b = g.sample(300, 6.8, 1.3);
            let mut d = column(
                &["Neurons", "Glia"],
                vec![a, b],
                "Soma area (\\mu m^2 \\times 10^2)",
            );
            if family == "histogram" {
                d.series.truncate(1);
            }
            d
        }
        "heatmap" => {
            let genes = [
                "ACTB", "GAPDH", "MYC", "TP53", "EGFR", "VEGFA", "IL6", "TNF", "CXCL8", "SOX2",
                "NANOG", "POU5F1",
            ];
            let samples = ["S1", "S2", "S3", "S4", "S5", "S6"];
            let mut series = Vec::new();
            for (j, s) in samples.iter().enumerate() {
                let y: Vec<f64> = (0..genes.len())
                    .map(|i| {
                        let block = if (i % 3 == 0) == (j < 3) { 1.6 } else { -1.2 };
                        Gen::round(block + g.norm(0.0, 0.6), 2)
                    })
                    .collect();
                series.push(Series::xy(
                    s,
                    j,
                    (1..=genes.len()).map(|i| i as f64).collect(),
                    y,
                ));
            }
            spec.options.cluster_rows = true;
            spec.options.cluster_columns = true;
            GraphData {
                series,
                categories: genes.iter().map(|s| s.to_string()).collect(),
                y_label: "z-score".into(),
                kind: TableKind::Grouped,
                ..Default::default()
            }
        }
        "kaplan_meier" => {
            let mut series = Vec::new();
            for (k, (name, rate)) in [("Standard", 0.06), ("Experimental", 0.035)]
                .iter()
                .enumerate()
            {
                let mut x = Vec::new();
                let mut y = Vec::new();
                for _ in 0..40 {
                    let tt = -(g.0.uniform().max(1e-9)).ln_d() / rate;
                    let cens = 5.0 + g.0.uniform() * 40.0;
                    let (time, ev) = if tt <= cens { (tt, 1.0) } else { (cens, 0.0) };
                    x.push(Gen::round(time.min(48.0), 1));
                    y.push(if time > 48.0 { 0.0 } else { ev });
                }
                series.push(Series::xy(name, k, x, y));
            }
            GraphData {
                series,
                x_label: "Months".into(),
                kind: TableKind::Survival,
                ..Default::default()
            }
        }
        "roc" => {
            let c = g.sample(60, 0.0, 1.0);
            let p = g.sample(60, 1.3, 1.1);
            column(&["Controls", "Patients"], vec![c, p], "Marker")
        }
        "bland_altman" => {
            let truth: Vec<f64> = (0..40).map(|_| g.norm(100.0, 20.0)).collect();
            let a: Vec<f64> = truth
                .iter()
                .map(|t| Gen::round(t + g.norm(0.0, 4.0), 1))
                .collect();
            let b: Vec<f64> = truth
                .iter()
                .map(|t| Gen::round(t * 0.98 + g.norm(0.0, 4.0), 1))
                .collect();
            column(&["Assay A", "Assay B"], vec![a, b], "Glucose (mg/dL)")
        }
        "volcano" => {
            let n = 1500;
            let mut x = Vec::new();
            let mut y = Vec::new();
            let mut labels = Vec::new();
            for i in 0..n {
                let eff = if i % 25 == 0 {
                    g.norm(0.0, 2.6)
                } else {
                    g.norm(0.0, 0.5)
                };
                let z = eff / 0.55 + g.norm(0.0, 1.0) * 0.3;
                let p = (2.0 * (1.0 - crate::stats::norm_cdf(z.abs()))).max(1e-30);
                x.push(Gen::round(eff, 3));
                y.push(p);
                labels.push(format!("GENE{i}"));
            }
            GraphData {
                series: vec![Series {
                    labels,
                    ..Series::xy("Genes", 0, x, y)
                }],
                kind: TableKind::Variables,
                ..Default::default()
            }
        }
        "forest" => {
            let rows = [
                "Age < 65",
                "Age ≥ 65",
                "Female",
                "Male",
                "Diabetes",
                "No diabetes",
                "Overall",
            ];
            let est = [0.72, 0.88, 0.69, 0.84, 0.95, 0.74, 0.79];
            let mut s = Series::xy(
                "Hazard ratio",
                0,
                (1..=rows.len()).map(|i| i as f64).collect(),
                est.to_vec(),
            );
            s.error = est
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let w = if i == 6 { 0.12 } else { 0.25 + 0.03 * i as f64 };
                    (Gen::round(e * (1.0 - w), 2), Gen::round(e * (1.0 + w), 2))
                })
                .collect();
            spec.axes.x.scale = crate::spec::Scale::Log10;
            spec.axes.x.ticks.minor = 1;
            GraphData {
                series: vec![s],
                categories: rows.iter().map(|s| s.to_string()).collect(),
                y_label: "Hazard ratio (95% CI)".into(),
                kind: TableKind::Grouped,
                ..Default::default()
            }
        }
        "correlation_matrix" => {
            let names = ["Weight", "Height", "BMI", "Glucose", "HbA1c"];
            let n = 60;
            let base: Vec<f64> = (0..n).map(|_| g.normal()).collect();
            let other: Vec<f64> = (0..n).map(|_| g.normal()).collect();
            let mut series = Vec::new();
            for (j, name) in names.iter().enumerate() {
                let y: Vec<f64> = (0..n)
                    .map(|i| {
                        let v = match j {
                            0 => base[i],
                            1 => 0.6 * base[i] + 0.8 * g.normal(),
                            2 => 0.8 * base[i] - 0.3 * other[i] + 0.4 * g.normal(),
                            3 => 0.3 * base[i] + 0.7 * other[i] + 0.5 * g.normal(),
                            _ => -0.2 * base[i] + 0.8 * other[i] + 0.4 * g.normal(),
                        };
                        Gen::round(v, 3)
                    })
                    .collect();
                series.push(Series::xy(name, j, (1..=n).map(|i| i as f64).collect(), y));
            }
            GraphData {
                series,
                kind: TableKind::Variables,
                ..Default::default()
            }
        }
        "pie" | "donut" => {
            let cats = [
                "Neutrophils",
                "Lymphocytes",
                "Monocytes",
                "Eosinophils",
                "Basophils",
            ];
            GraphData {
                series: vec![Series::xy(
                    "Leukocytes",
                    0,
                    (1..=5).map(|i| i as f64).collect(),
                    vec![58.0, 30.0, 7.0, 4.0, 1.0],
                )],
                categories: cats.iter().map(|s| s.to_string()).collect(),
                kind: TableKind::Parts,
                ..Default::default()
            }
        }
        "qq" => {
            let mut v = g.sample(60, 4.0, 1.0);
            v.iter_mut()
                .for_each(|x| *x = Gen::round(x.exp_d() / 10.0, 3));
            column(&["Plasma IL-6"], vec![v], "Sample quantiles (pg/mL)")
        }
        "residuals" => {
            let xs: Vec<f64> = (0..30).map(|i| i as f64).collect();
            let ys: Vec<f64> = xs
                .iter()
                .map(|x| {
                    Gen::round(
                        2.0 + 0.5 * x + 0.012 * (x - 15.0).powi(2) + g.norm(0.0, 0.6),
                        2,
                    )
                })
                .collect();
            GraphData {
                series: vec![Series::xy("Standard curve", 0, xs, ys)],
                x_label: "Concentration (ng/mL)".into(),
                y_label: "Absorbance".into(),
                kind: TableKind::Xy,
                ..Default::default()
            }
        }
        _ => return None,
    };
    Some((spec, data))
}
