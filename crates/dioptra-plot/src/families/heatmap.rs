//! `heatmap` (cells coloured on a sequential or diverging scale, colour bar, optional
//! average-linkage clustering of rows and columns with dendrograms) and
//! `correlation_matrix` (lower triangle of Pearson coefficients, printed in the cells).

use super::common::{centre, Cats};
use crate::plot::{AxisPlan, ColourBar, Ctx, Dendrogram, Layer, Mark, Plot};
use crate::scene::{Anchor, Baseline, Color, Paint, Stroke};
use crate::spec::FrameStyle;
use crate::stats::{cluster, pearson};
use crate::text::{self, Weight};
use crate::theme::Ramp;

fn contrast(c: Color) -> Color {
    if c.luminance() > 0.36 {
        Color::rgb(0x1a, 0x1a, 0x1a)
    } else {
        Color::rgb(0xff, 0xff, 0xff)
    }
}

fn fmt_cell(v: f64) -> String {
    let a = v.abs();
    let d = if a >= 100.0 {
        0
    } else if a >= 10.0 {
        1
    } else {
        2
    };
    crate::ticks::decimal(v, d)
}

/// The ramp and range for values: diverging around 0 when the data cross it (or the
/// palette is diverging), sequential otherwise.
fn scale_for(ctx: &Ctx<'_>, lo: f64, hi: f64) -> (Ramp, f64, f64) {
    let diverging = ctx.palette == "tol_sunset" || (lo < 0.0 && hi > 0.0);
    if diverging {
        let m = lo.abs().max(hi.abs()).max(1e-12);
        (Ramp::named("tol_sunset"), -m, m)
    } else {
        let name = match ctx.palette.as_str() {
            "cividis" | "grey" => ctx.palette.as_str(),
            _ if ctx.theme.mono => "grey",
            _ => "viridis",
        };
        (Ramp::named(name), lo, if hi > lo { hi } else { lo + 1.0 })
    }
}

/// Build a heatmap: rows are categories (X values), columns are series.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let o = &ctx.spec.options;
    let t = &ctx.theme;
    let cats = Cats::of(d);
    let nr = cats.xs.len();
    let nc = d.series.len();
    let mut m = vec![vec![f64::NAN; nc]; nr];
    for (j, s) in d.series.iter().enumerate() {
        for (k, x) in s.x.iter().enumerate() {
            if let Some(r) = cats.index(*x) {
                let v = if s.replicates.is_empty() {
                    s.y.get(k).copied().unwrap_or(f64::NAN)
                } else {
                    centre(&s.values_at(k), o.summary)
                };
                m[r as usize - 1][j] = v;
            }
        }
    }
    let (row_merges, row_order) = if o.cluster_rows && nr > 2 {
        cluster(&m)
    } else {
        (vec![], (0..nr).collect())
    };
    let mt: Vec<Vec<f64>> = (0..nc).map(|j| m.iter().map(|r| r[j]).collect()).collect();
    let (col_merges, col_order) = if o.cluster_columns && nc > 2 {
        cluster(&mt)
    } else {
        (vec![], (0..nc).collect())
    };
    let (lo, hi) = m
        .iter()
        .flatten()
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |a, v| {
            (a.0.min(*v), a.1.max(*v))
        });
    let (ramp, vlo, vhi) = scale_for(ctx, lo, hi);
    let mut p = Plot {
        x: AxisPlan {
            bare: true,
            ..AxisPlan::categorical(
                col_order
                    .iter()
                    .map(|j| d.series[*j].name.clone())
                    .collect(),
                "",
            )
        },
        y: AxisPlan {
            bare: true,
            reverse: true,
            ..AxisPlan::categorical(
                row_order.iter().map(|r| cats.labels[*r].clone()).collect(),
                "",
            )
        },
        frame: Some(FrameStyle::None),
        colour_bar: Some(ColourBar {
            ramp: ramp.clone(),
            lo: vlo,
            hi: vhi,
            title: d.y_label.clone(),
        }),
        ..Default::default()
    };
    if !row_merges.is_empty() {
        p.dendro_y = Some(Dendrogram {
            merges: row_merges,
            order: row_order.clone(),
        });
    }
    if !col_merges.is_empty() {
        p.dendro_x = Some(Dendrogram {
            merges: col_merges,
            order: col_order.clone(),
        });
    }
    let mut l = Layer::new("cells", "/style/palette");
    let sep = Stroke::solid(t.background, 0.5);
    for (ri, r) in row_order.iter().enumerate() {
        for (ci, c) in col_order.iter().enumerate() {
            let v = m[*r][*c];
            let (x, y) = (ci as f64 + 1.0, ri as f64 + 1.0);
            let fill = if v.is_finite() {
                ramp.at((v - vlo) / (vhi - vlo))
            } else {
                t.grid
            };
            l.marks.push(Mark::Rect {
                x0: x - 0.5,
                x1: x + 0.5,
                y0: y - 0.5,
                y1: y + 0.5,
                paint: Paint::both(fill, sep),
            });
            if o.cell_values && v.is_finite() {
                l.marks.push(Mark::Label {
                    x,
                    y,
                    dx: 0.0,
                    dy: 0.0,
                    runs: text::parse(&fmt_cell(v)),
                    size: t.tick_pt * 0.85,
                    color: contrast(fill),
                    anchor: Anchor::Middle,
                    baseline: Baseline::Middle,
                    weight: Weight::Regular,
                });
            }
        }
    }
    p.layers.push(l);
    p
}

fn aligned(a: &crate::data::Series, b: &crate::data::Series) -> (Vec<f64>, Vec<f64>) {
    let mut va = Vec::new();
    let mut vb = Vec::new();
    for (i, x) in a.x.iter().enumerate() {
        if let Some(j) = b.x.iter().position(|bx| bx == x) {
            va.push(a.y.get(i).copied().unwrap_or(f64::NAN));
            vb.push(b.y.get(j).copied().unwrap_or(f64::NAN));
        }
    }
    (va, vb)
}

/// Build a correlation matrix (lower triangle).
pub fn correlation(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let t = &ctx.theme;
    let n = d.series.len();
    let names: Vec<String> = d.series.iter().map(|s| s.name.clone()).collect();
    let k = n.saturating_sub(1).max(1);
    let ramp = Ramp::named("tol_sunset");
    let mut p = Plot {
        x: AxisPlan {
            bare: true,
            ..AxisPlan::categorical(names.iter().take(k).cloned().collect(), "")
        },
        y: AxisPlan {
            bare: true,
            reverse: true,
            ..AxisPlan::categorical(names.iter().skip(1).cloned().collect(), "")
        },
        frame: Some(FrameStyle::None),
        colour_bar: Some(ColourBar {
            ramp: ramp.clone(),
            lo: -1.0,
            hi: 1.0,
            title: "Pearson r".into(),
        }),
        aspect: Some(1.0),
        ..Default::default()
    };
    let mut l = Layer::new("cells", "/style/palette");
    let sep = Stroke::solid(t.background, 0.75);
    // Series means the row-aligned values; summarise replicates first.
    let flat: Vec<crate::data::Series> = d
        .series
        .iter()
        .map(|s| {
            let mut c = s.clone();
            if !s.replicates.is_empty() {
                c.y = (0..s.x.len())
                    .map(|i| crate::stats::mean(&s.values_at(i)))
                    .collect();
            }
            c
        })
        .collect();
    for i in 1..n {
        for j in 0..i {
            let (a, b) = aligned(&flat[i], &flat[j]);
            let r = pearson(&a, &b);
            let (x, y) = (j as f64 + 1.0, i as f64);
            let fill = if r.is_finite() {
                ramp.at((r + 1.0) / 2.0)
            } else {
                t.grid
            };
            l.marks.push(Mark::Rect {
                x0: x - 0.5,
                x1: x + 0.5,
                y0: y - 0.5,
                y1: y + 0.5,
                paint: Paint::both(fill, sep),
            });
            if r.is_finite() {
                l.marks.push(Mark::Label {
                    x,
                    y,
                    dx: 0.0,
                    dy: 0.0,
                    runs: text::parse(&crate::ticks::decimal(r, 2)),
                    size: t.tick_pt * 0.85,
                    color: contrast(fill),
                    anchor: Anchor::Middle,
                    baseline: Baseline::Middle,
                    weight: Weight::Regular,
                });
            }
        }
    }
    p.layers.push(l);
    p
}
