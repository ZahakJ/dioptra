//! Helpers shared by the families: summaries and error bars, category slots, jitter,
//! legend entries.

use crate::data::{GraphData, Series, TableKind};
use crate::plot::{Ctx, Layer, LegendEntry, Mark, SeriesStyle, Swatch};
use crate::scene::{MarkerShape, Paint, Stroke};
use crate::spec::{ErrorBars, Summary};
use crate::stats::{self, quantile_sorted, sorted};

/// Mean or median of the finite values.
pub fn centre(v: &[f64], s: Summary) -> f64 {
    match s {
        Summary::Mean => stats::mean(v),
        Summary::Median => stats::median(v),
    }
}

/// Error bounds (low, high) for values around `c` (the mean or median).
pub fn error_bounds(v: &[f64], c: f64, e: ErrorBars) -> Option<(f64, f64)> {
    let s = sorted(v);
    let n = s.len();
    if n == 0 || !c.is_finite() {
        return None;
    }
    let r = match e {
        ErrorBars::None => return None,
        ErrorBars::Sd | ErrorBars::Sem | ErrorBars::Ci95 => {
            if n < 2 {
                return None;
            }
            let sd = stats::sd(&s);
            let half = match e {
                ErrorBars::Sd => sd,
                ErrorBars::Sem => sd / (n as f64).sqrt(),
                _ => stats::t_quantile(0.975, (n - 1) as f64) * sd / (n as f64).sqrt(),
            };
            (c - half, c + half)
        }
        ErrorBars::Range => (s[0], s[n - 1]),
        ErrorBars::Iqr => (quantile_sorted(&s, 0.25), quantile_sorted(&s, 0.75)),
    };
    (r.0.is_finite() && r.1.is_finite()).then_some(r)
}

/// Error bounds of a series at X index `i`: precomputed ones win.
pub fn series_error(s: &Series, i: usize, c: f64, e: ErrorBars) -> Option<(f64, f64)> {
    if e == ErrorBars::None {
        return None;
    }
    if let Some(b) = s.error.get(i) {
        return (b.0.is_finite() && b.1.is_finite()).then_some(*b);
    }
    error_bounds(&s.values_at(i), c, e)
}

/// Categories along X for summary families.
#[derive(Debug, Clone)]
pub struct Cats {
    /// Distinct X values in order (position i + 1 holds `xs[i]`).
    pub xs: Vec<f64>,
    /// Labels.
    pub labels: Vec<String>,
    /// Several series share each category (grouped tables).
    pub clustered: bool,
    /// Number of series.
    pub k: usize,
}

impl Cats {
    /// Work out the categories of the data.
    pub fn of(data: &GraphData) -> Cats {
        let mut xs: Vec<f64> = data
            .series
            .iter()
            .flat_map(|s| s.x.iter().copied())
            .filter(|v| v.is_finite())
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let shared = data.series.len() > 1
            && data.series.iter().any(|s| {
                s.x.iter().any(|x| {
                    data.series
                        .iter()
                        .filter(|o| o.x.iter().any(|ox| ox == x))
                        .count()
                        > 1
                })
            });
        let clustered = matches!(data.kind, TableKind::Grouped | TableKind::Contingency) || shared;
        let labels = xs
            .iter()
            .map(|x| {
                let i = x.round() as i64;
                if (x - i as f64).abs() < 1e-9 && i >= 1 && (i as usize) <= data.categories.len() {
                    data.categories[i as usize - 1].clone()
                } else if !clustered {
                    data.series
                        .iter()
                        .find(|s| s.x.contains(x))
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| {
                            crate::ticks::decimal(*x, crate::ticks::decimals_for(&[*x], 1.0))
                        })
                } else {
                    let d = crate::ticks::decimals_for(&[*x], 1e-9).min(6);
                    crate::ticks::decimal(*x, d)
                }
            })
            .collect();
        Cats {
            xs,
            labels,
            clustered,
            k: data.series.len().max(1),
        }
    }

    /// Category position (1-based) of an X value.
    pub fn index(&self, x: f64) -> Option<f64> {
        self.xs.iter().position(|v| *v == x).map(|i| i as f64 + 1.0)
    }

    /// Centre and width of the slot of series `j` at X value `x`.
    pub fn slot(&self, j: usize, x: f64) -> Option<(f64, f64)> {
        let c = self.index(x)?;
        if self.clustered && self.k > 1 {
            let total = 0.84;
            let w = total / self.k as f64;
            Some((c - total / 2.0 + w * (j as f64 + 0.5), w))
        } else {
            Some((c, 0.84))
        }
    }
}

/// Deterministic, density-scaled jitter offsets in [−half, half]: points in dense regions
/// spread wider (a sina-like layout), sparse ones stay near the centre line.
pub fn jitter(values: &[f64], half: f64, seed: u64) -> Vec<f64> {
    let n = values.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![0.0];
    }
    let s = sorted(values);
    let (lo, hi) = (s[0], s[s.len() - 1]);
    let bins = 24usize;
    let width = (hi - lo).max(1e-12);
    let mut hist = vec![0.0f64; bins];
    for v in &s {
        let b = (((v - lo) / width) * bins as f64)
            .floor()
            .clamp(0.0, (bins - 1) as f64) as usize;
        hist[b] += 1.0;
    }
    // Light smoothing.
    let sm: Vec<f64> = (0..bins)
        .map(|i| {
            let a = if i > 0 { hist[i - 1] } else { hist[i] };
            let c = if i + 1 < bins { hist[i + 1] } else { hist[i] };
            0.25 * a + 0.5 * hist[i] + 0.25 * c
        })
        .collect();
    let max = sm.iter().fold(0.0f64, |a, b| a.max(*b)).max(1e-12);
    // Rank of each value for the low-discrepancy sequence.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|a, b| values[*a].total_cmp(&values[*b]).then(a.cmp(b)));
    let mut out = vec![0.0; n];
    let phase = (seed % 1000) as f64 / 1000.0;
    let spread = (n as f64 / 6.0).clamp(0.35, 1.0);
    for (rank, &i) in idx.iter().enumerate() {
        let v = values[i];
        if !v.is_finite() {
            continue;
        }
        let b = (((v - lo) / width) * bins as f64)
            .floor()
            .clamp(0.0, (bins - 1) as f64) as usize;
        let dens = (sm[b] / max).sqrt();
        let u = (phase + rank as f64 * 0.618_033_988_749_895).fract();
        out[i] = (u - 0.5) * 2.0 * half * dens * spread;
    }
    out
}

/// A layer for series `i` (editor target: its dataset style).
pub fn series_layer(i: usize, dataset: usize) -> Layer {
    Layer::new(format!("series-{i}"), format!("/style/datasets/{dataset}"))
}

/// Points paint for raw data drawn over a summary.
pub fn points_paint(ctx: &Ctx<'_>, st: &SeriesStyle) -> Paint {
    st.marker_paint(ctx.theme.background)
}

/// A legend entry with the series' marker.
pub fn marker_entry(ctx: &Ctx<'_>, st: &SeriesStyle) -> LegendEntry {
    LegendEntry {
        label: st.label.clone(),
        swatch: Swatch::Marker(
            st.marker.unwrap_or(MarkerShape::Circle),
            st.marker_paint(ctx.theme.background),
            st.size,
        ),
        dataset: Some(st.dataset),
    }
}

/// A legend entry with a filled square.
pub fn fill_entry(ctx: &Ctx<'_>, st: &SeriesStyle, fill: crate::scene::Color) -> LegendEntry {
    LegendEntry {
        label: st.label.clone(),
        swatch: Swatch::Fill(Paint::both(
            fill,
            Stroke::solid(st.colour, ctx.theme.axis_pt),
        )),
        dataset: Some(st.dataset),
    }
}

/// Error-bar stroke in the foreground colour.
pub fn bar_stroke(ctx: &Ctx<'_>) -> Stroke {
    Stroke::solid(ctx.fg(), ctx.theme.axis_pt)
}

/// Push an error bar.
pub fn push_error(l: &mut Layer, at: f64, lo: f64, hi: f64, cap: f64, stroke: Stroke) {
    l.marks.push(Mark::ErrorBar {
        at,
        lo,
        hi,
        horizontal: false,
        cap,
        stroke,
    });
}

/// Default Y title for summary graphs.
pub fn y_title(data: &GraphData) -> String {
    data.y_label.clone()
}
