//! `histogram`: shared bins (Freedman–Diaconis width unless a width or count is given,
//! aligned to multiples of the width), counts or density, one outline per series.

use super::common::{fill_entry, series_layer};
use crate::detmath::DetMath;
use crate::plot::{AxisPlan, Ctx, Mark, Plot};
use crate::scene::{Paint, Stroke};
use crate::stats::{quantile_sorted, sorted};

/// Bin width by Freedman–Diaconis (2·IQR·n^(−1/3)), falling back to Sturges.
pub fn fd_width(v: &[f64]) -> f64 {
    let s = sorted(v);
    let n = s.len();
    if n < 2 {
        return 1.0;
    }
    let iqr = quantile_sorted(&s, 0.75) - quantile_sorted(&s, 0.25);
    let range = s[n - 1] - s[0];
    let w = 2.0 * iqr * (n as f64).powf_d(-1.0 / 3.0);
    if w > 0.0 && w.is_finite() {
        w
    } else if range > 0.0 {
        range / ((n as f64).log2_d().ceil() + 1.0)
    } else {
        1.0
    }
}

/// Round a width to 1, 2 or 5 × 10ⁿ so bin edges are readable.
pub fn nice_width(w: f64) -> f64 {
    let e = w.log10_d().floor();
    let f = w / 10f64.powf_d(e);
    let m = if f < 1.5 {
        1.0
    } else if f < 3.5 {
        2.0
    } else if f < 7.5 {
        5.0
    } else {
        10.0
    };
    m * 10f64.powf_d(e)
}

/// Build a histogram.
pub fn plot(ctx: &Ctx<'_>) -> Plot {
    let d = ctx.data;
    let o = &ctx.spec.options;
    let t = &ctx.theme;
    let mut p = Plot {
        x: AxisPlan::continuous(&d.y_label),
        y: AxisPlan {
            include: vec![0.0],
            ..AxisPlan::continuous(if o.density { "Density" } else { "Count" })
        },
        ..Default::default()
    };
    let all: Vec<f64> = d.series.iter().flat_map(|s| s.all_values()).collect();
    let s = sorted(&all);
    if s.is_empty() {
        return p;
    }
    let (lo, hi) = (s[0], s[s.len() - 1]);
    let mut w = match (o.bin_width, o.bins) {
        (Some(w), _) if w > 0.0 => w,
        (_, Some(b)) if b > 0 => ((hi - lo) / b as f64).max(1e-12),
        _ => nice_width(fd_width(&all)),
    };
    if (hi - lo) / w > 400.0 {
        w = (hi - lo) / 400.0;
    }
    let start = (lo / w).floor() * w;
    let nb = (((hi - start) / w).floor() as usize + 1).max(1);
    let n = d.series.len();
    let multi = n > 1;
    for (i, se) in d.series.iter().enumerate() {
        let st = ctx.style(i, n, se.dataset, &se.name);
        let v = se.all_values();
        let mut counts = vec![0usize; nb];
        for x in &v {
            let b = (((x - start) / w).floor() as isize).clamp(0, nb as isize - 1) as usize;
            counts[b] += 1;
        }
        let scale = if o.density {
            1.0 / (v.len().max(1) as f64 * w)
        } else {
            1.0
        };
        let fill = if t.mono {
            if multi {
                st.colour.alpha(0.25)
            } else {
                t.grid
            }
        } else if multi {
            st.colour.alpha(0.3)
        } else {
            ctx.tint(st.colour)
        };
        let mut l = series_layer(i, se.dataset);
        for (b, c) in counts.iter().enumerate() {
            if *c == 0 {
                continue;
            }
            let x0 = start + b as f64 * w;
            l.marks.push(Mark::Rect {
                x0,
                x1: x0 + w,
                y0: 0.0,
                y1: *c as f64 * scale,
                paint: Paint::both(
                    fill,
                    Stroke::solid(if t.mono { t.axis } else { st.colour }, t.axis_pt * 0.8),
                ),
            });
        }
        p.layers.push(l);
        p.legend.push(fill_entry(ctx, &st, fill));
    }
    p.notes
        .push(format!("histogram: bin width {w}, first edge {start}"));
    p
}
