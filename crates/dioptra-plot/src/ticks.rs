//! Tick placement and tick-label formatting.
//!
//! Linear axes use the extended Wilkinson algorithm (Talbot, Lin & Hanrahan, "An Extension
//! of Wilkinson's Algorithm for Positioning Tick Labels on Axes", InfoVis 2010), which
//! scores candidate step sets by simplicity, coverage and density. Log axes put majors on
//! powers of the base and minors on 2…9 × 10ⁿ.

use crate::detmath::DetMath;
use crate::spec::{Scale, TickFormat};
use crate::text::{Run, Shift};

const Q: [f64; 4] = [1.0, 5.0, 2.0, 2.5];
const W: [f64; 4] = [0.25, 0.2, 0.5, 0.05];
const EPS: f64 = 1e-10;

fn simplicity(i: usize, n: usize, j: f64, lmin: f64, lmax: f64, lstep: f64) -> f64 {
    let r = lmin.rem_euclid(lstep);
    let v = if (r < EPS * lstep || lstep - r < EPS * lstep) && lmin <= 0.0 && lmax >= 0.0 {
        1.0
    } else {
        0.0
    };
    1.0 - (i as f64 - 1.0) / (n as f64 - 1.0) - j + v
}

fn simplicity_max(i: usize, n: usize, j: f64) -> f64 {
    1.0 - (i as f64 - 1.0) / (n as f64 - 1.0) - j + 1.0
}

fn coverage(dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = dmax - dmin;
    1.0 - 0.5 * ((dmax - lmax).powi(2) + (dmin - lmin).powi(2)) / (0.1 * r).powi(2)
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let r = dmax - dmin;
    if span > r {
        let half = (span - r) / 2.0;
        1.0 - 0.5 * (2.0 * half * half) / (0.1 * r).powi(2)
    } else {
        1.0
    }
}

fn density(k: f64, m: f64, dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = (k - 1.0) / (lmax - lmin);
    let rt = (m - 1.0) / (lmax.max(dmax) - dmin.min(lmin));
    2.0 - (r / rt).max(rt / r)
}

fn density_max(k: f64, m: f64) -> f64 {
    if k >= m {
        2.0 - (k - 1.0) / (m - 1.0)
    } else {
        1.0
    }
}

/// Extended-Wilkinson labels for [dmin, dmax] with about `m` ticks. With `loose`, the
/// labels cover the data. Returns (first, last, step).
pub fn extended(dmin: f64, dmax: f64, m: usize, loose: bool) -> (f64, f64, f64) {
    let m = (m.max(2)) as f64;
    if dmax.is_nan() || dmin.is_nan() || dmax <= dmin || !dmin.is_finite() || !dmax.is_finite() {
        let v = if dmin.is_finite() { dmin } else { 0.0 };
        let s = if v == 0.0 {
            1.0
        } else {
            10f64.powf_d(v.abs().log10_d().floor())
        };
        return (v - s, v + s, s);
    }
    let n = Q.len();
    let mut best = (-2.0f64, dmin, dmax, (dmax - dmin) / (m - 1.0));
    let mut j = 1.0;
    // Only j = 1 (no skipped multiples): labels are always 1, 2, 2.5 or 5 × 10ⁿ apart.
    'outer: while j < 1.5 {
        for (qi, q) in Q.iter().enumerate() {
            let i = qi + 1;
            let sm = simplicity_max(i, n, j);
            if W[0] * sm + W[1] + W[2] + W[3] < best.0 {
                break 'outer;
            }
            let mut k = 2.0;
            while k < 4.0 * m + 10.0 {
                let dm = density_max(k, m);
                if W[0] * sm + W[1] + W[2] * dm + W[3] < best.0 {
                    break;
                }
                let delta = (dmax - dmin) / (k + 1.0) / j / q;
                let mut z = delta.log10_d().ceil();
                let zmax = z + 6.0;
                while z < zmax {
                    let step = j * q * 10f64.powf_d(z);
                    let cm = coverage_max(dmin, dmax, step * (k - 1.0));
                    if W[0] * sm + W[1] * cm + W[2] * dm + W[3] < best.0 {
                        break;
                    }
                    let min_start = (dmax / step).floor() * j - (k - 1.0) * j;
                    let max_start = (dmin / step).ceil() * j;
                    if min_start > max_start {
                        z += 1.0;
                        continue;
                    }
                    let mut start = min_start;
                    while start <= max_start {
                        // Labels sit on multiples of the step (0, 2, 4 rather than 1, 3, 5).
                        if start.rem_euclid(j) != 0.0 {
                            start += 1.0;
                            continue;
                        }
                        let lmin = start * (step / j);
                        let lmax = lmin + step * (k - 1.0);
                        let s = simplicity(i, n, j, lmin, lmax, step);
                        let c = coverage(dmin, dmax, lmin, lmax);
                        let g = density(k, m, dmin, dmax, lmin, lmax);
                        let score = W[0] * s + W[1] * c + W[2] * g + W[3];
                        let ok = !loose || (lmin <= dmin + EPS * step && lmax >= dmax - EPS * step);
                        if score > best.0 && ok {
                            best = (score, lmin, lmax, step);
                        }
                        start += 1.0;
                    }
                    z += 1.0;
                }
                k += 1.0;
            }
        }
        j += 1.0;
    }
    (best.1, best.2, best.3)
}

/// Clean floating noise from a tick value (0.30000000000000004 → 0.3).
pub fn clean(v: f64, step: f64) -> f64 {
    if v.abs() < step.abs() * 1e-9 {
        return 0.0;
    }
    // Round to six digits below the step, dividing by an exact power of ten.
    let e = (step.abs().log10_d().floor() as i32) - 6;
    if e < 0 {
        let k = 10f64.powi(-e);
        (v * k).round() / k
    } else {
        let k = 10f64.powi(e);
        (v / k).round() * k
    }
}

/// Ticks from `first` to `last` by `step`, inclusive, cleaned.
pub fn seq(first: f64, last: f64, step: f64) -> Vec<f64> {
    let mut v = Vec::new();
    if step.is_nan() || step <= 0.0 {
        return v;
    }
    let n = ((last - first) / step + 1e-9).floor() as i64;
    for i in 0..=n.clamp(0, 10_000) {
        v.push(clean(first + i as f64 * step, step));
    }
    v
}

/// Decimals needed to show every value exactly at its step.
pub fn decimals_for(values: &[f64], step: f64) -> usize {
    for d in 0..=12usize {
        let f = 10f64.powi(d as i32);
        if values
            .iter()
            .all(|v| ((v * f).round() / f - v).abs() <= step.abs() * 1e-6 + 1e-300)
        {
            return d;
        }
    }
    12
}

fn minus(s: String) -> String {
    if let Some(r) = s.strip_prefix('-') {
        format!("−{r}")
    } else {
        s
    }
}

/// A plain decimal string with `d` decimals and a typographic minus.
pub fn decimal(v: f64, d: usize) -> String {
    let s = format!("{:.*}", d, v);
    // "-0.0" → "0.0"
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        return s.trim_start_matches('-').to_string();
    }
    minus(s)
}

/// Scientific runs `m×10ⁿ` (or `10ⁿ` when the mantissa is 1).
pub fn scientific(v: f64, sig: usize) -> Vec<Run> {
    if v == 0.0 {
        return vec![Run {
            text: "0".into(),
            shift: Shift::None,
        }];
    }
    let e = v.abs().log10_d().floor() as i32;
    let mut m = v / 10f64.powi(e);
    let (mut e2, f) = (e, 10f64.powi(sig as i32));
    m = (m * f).round() / f;
    if m.abs() >= 10.0 {
        m /= 10.0;
        e2 += 1;
    }
    let ms = {
        let s = format!("{:.*}", sig, m);
        let s = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        };
        minus(s)
    };
    let exp = minus(e2.to_string());
    let mut runs = Vec::new();
    if ms == "1" || ms == "−1" {
        let lead = if ms == "−1" { "−10" } else { "10" };
        runs.push(Run {
            text: lead.into(),
            shift: Shift::None,
        });
    } else {
        runs.push(Run {
            text: format!("{ms}×10"),
            shift: Shift::None,
        });
    }
    runs.push(Run {
        text: exp,
        shift: Shift::Super,
    });
    runs
}

/// Labels for linear ticks.
pub fn linear_labels(
    ticks: &[f64],
    step: f64,
    fmt: TickFormat,
    decimals: Option<u8>,
) -> Vec<Vec<Run>> {
    let maxabs = ticks.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let sci = match fmt {
        TickFormat::Scientific => true,
        TickFormat::Decimal => false,
        _ => maxabs >= 1e6 || (maxabs > 0.0 && maxabs <= 1e-4),
    };
    if sci {
        // Enough mantissa digits to tell neighbouring ticks apart.
        let e = maxabs.log10_d().floor();
        let rel = (step / 10f64.powf_d(e)).abs();
        let sig = decimals_for(&[rel], rel.max(1e-12)).clamp(0, 6);
        let sig = sig.max(if rel < 1.0 { 1 } else { 0 });
        return ticks.iter().map(|v| scientific(*v, sig)).collect();
    }
    let d = decimals.map_or_else(|| decimals_for(ticks, step), |d| d as usize);
    ticks
        .iter()
        .map(|v| {
            vec![Run {
                text: decimal(*v, d),
                shift: Shift::None,
            }]
        })
        .collect()
}

/// Label for a power of the base on a log axis.
pub fn log_label(exp: f64, base: f64, fmt: TickFormat) -> Vec<Run> {
    let v = base.powf_d(exp);
    let power = match fmt {
        TickFormat::Power | TickFormat::Scientific => true,
        TickFormat::Decimal => false,
        // Auto: plain numbers inside 0.001 … 10 000, powers outside.
        TickFormat::Auto => !(1e-3..=1e4).contains(&v) || base == 2.0 && exp.abs() > 10.0,
    };
    if power {
        let e = exp.round() as i64;
        vec![
            Run {
                text: if base == 2.0 { "2".into() } else { "10".into() },
                shift: Shift::None,
            },
            Run {
                text: minus(e.to_string()),
                shift: Shift::Super,
            },
        ]
    } else {
        let d = if v >= 1.0 {
            0
        } else {
            (-v.log10_d()).ceil() as usize
        };
        vec![Run {
            text: decimal(v, d),
            shift: Shift::None,
        }]
    }
}

/// Major and minor ticks of an axis in *transformed* units (log units on log axes),
/// with labels for the majors.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TickSet {
    /// Major tick positions (transformed units).
    pub major: Vec<f64>,
    /// Labels for the majors.
    pub labels: Vec<Vec<Run>>,
    /// Minor tick positions.
    pub minor: Vec<f64>,
    /// The axis domain implied by the ticks (loose ranges end on a tick).
    pub domain: (f64, f64),
}

/// Compute ticks for a transformed domain. `fixed` is true when the user set both limits
/// (then the domain is kept and ticks fall inside it).
#[allow(clippy::too_many_arguments)]
pub fn ticks(
    scale: Scale,
    lo: f64,
    hi: f64,
    target: usize,
    fixed: bool,
    major_step: Option<f64>,
    minors: u32,
    fmt: TickFormat,
    decimals: Option<u8>,
) -> TickSet {
    match scale {
        Scale::Log10 | Scale::Log2 => {
            let base: f64 = if scale == Scale::Log2 { 2.0 } else { 10.0 };
            let (mut a, mut b) = (lo, hi);
            if !fixed {
                a = (a + 1e-9).floor();
                b = (b - 1e-9).ceil();
                if b <= a {
                    b = a + 1.0;
                }
            }
            if base == 10.0 && !fixed && hi - lo < 1.3 && major_step.is_none() {
                // Under about a decade: 1–2–5 steps (0.2, 0.5, 1, 2) instead of bare decades.
                let mut cands: Vec<f64> = Vec::new();
                let mut k = lo.floor() - 1.0;
                while k <= hi.ceil() + 1.0 {
                    for m in [1.0, 2.0, 5.0] {
                        cands.push(m * 10f64.powf_d(k));
                    }
                    k += 1.0;
                }
                let (rl, rh) = (10f64.powf_d(lo), 10f64.powf_d(hi));
                let first = cands
                    .iter()
                    .copied()
                    .filter(|c| *c <= rl * (1.0 + 1e-9))
                    .fold(f64::NAN, f64::max);
                let last = cands
                    .iter()
                    .copied()
                    .filter(|c| *c >= rh * (1.0 - 1e-9))
                    .fold(f64::NAN, f64::min);
                if first.is_finite() && last.is_finite() {
                    let raw: Vec<f64> = cands
                        .iter()
                        .copied()
                        .filter(|c| *c >= first * (1.0 - 1e-9) && *c <= last * (1.0 + 1e-9))
                        .collect();
                    let mut minor = Vec::new();
                    if minors > 0 {
                        let mut k = first.log10_d().floor();
                        while 10f64.powf_d(k) <= last {
                            for m in 1..10 {
                                let v = m as f64 * 10f64.powf_d(k);
                                if v > first
                                    && v < last
                                    && !raw.iter().any(|r| (r / v - 1.0).abs() < 1e-9)
                                {
                                    minor.push(v.log10_d());
                                }
                            }
                            k += 1.0;
                        }
                    }
                    let labels = raw
                        .iter()
                        .map(|v| {
                            let d = if *v >= 1.0 {
                                0
                            } else {
                                (-v.log10_d()).ceil() as usize
                            };
                            match fmt {
                                TickFormat::Power | TickFormat::Scientific => scientific(*v, 0),
                                _ => vec![Run {
                                    text: decimal(*v, d),
                                    shift: Shift::None,
                                }],
                            }
                        })
                        .collect();
                    return TickSet {
                        major: raw.iter().map(|v| v.log10_d()).collect(),
                        labels,
                        minor,
                        domain: (first.log10_d(), last.log10_d()),
                    };
                }
            }
            let span = b - a;
            if span < 1.0 && fixed {
                // Less than one decade: linear ticks on the raw values.
                let (rl, rh) = (base.powf_d(a), base.powf_d(b));
                let (f, l, s) = extended(rl, rh, target, false);
                let raw: Vec<f64> = seq(f, l, s)
                    .into_iter()
                    .filter(|v| *v >= rl * (1.0 - 1e-9) && *v <= rh * (1.0 + 1e-9) && *v > 0.0)
                    .collect();
                let labels = linear_labels(&raw, s, fmt, decimals);
                return TickSet {
                    major: raw.iter().map(|v| v.log_d(base)).collect(),
                    labels,
                    minor: vec![],
                    domain: (a, b),
                };
            }
            let step = major_step
                .filter(|s| *s >= 1.0)
                .unwrap_or_else(|| (span / target.max(2) as f64).ceil().max(1.0));
            let first = a.ceil();
            let major: Vec<f64> = seq(first, b + 1e-9, step);
            let labels = major.iter().map(|e| log_label(*e, base, fmt)).collect();
            let mut minor = Vec::new();
            if minors > 0 && base == 10.0 {
                let mut d = a.floor();
                while d < b {
                    for k in 2..10 {
                        let v = d + (k as f64).log10_d();
                        if v > a + 1e-9 && v < b - 1e-9 {
                            minor.push(v);
                        }
                    }
                    d += 1.0;
                }
                if step > 1.0 {
                    // Decades that are not labelled become minors too.
                    let mut d = a.ceil();
                    while d <= b {
                        if !major.iter().any(|m| (m - d).abs() < 1e-9) {
                            minor.push(d);
                        }
                        d += 1.0;
                    }
                    minor.sort_by(f64::total_cmp);
                }
            } else if minors > 0 {
                minor = seq(a.ceil(), b, 1.0)
                    .into_iter()
                    .filter(|v| !major.iter().any(|m| (m - v).abs() < 1e-9))
                    .collect();
            }
            TickSet {
                major,
                labels,
                minor,
                domain: (a, b),
            }
        }
        _ => {
            let (first, last, step) = match major_step.filter(|s| *s > 0.0 && (hi - lo) / s < 500.0)
            {
                Some(s) => {
                    if fixed {
                        ((lo / s - 1e-9).ceil() * s, hi, s)
                    } else {
                        ((lo / s + 1e-9).floor() * s, (hi / s - 1e-9).ceil() * s, s)
                    }
                }
                None => {
                    let (f, l, s) = extended(lo, hi, target, !fixed);
                    if fixed {
                        ((lo / s - 1e-9).ceil() * s, hi, s)
                    } else {
                        (f, l, s)
                    }
                }
            };
            let major: Vec<f64> = seq(first, last + step * 1e-9, step);
            let domain = if fixed {
                (lo, hi)
            } else {
                (first.min(lo), last.max(hi))
            };
            let major: Vec<f64> = major
                .into_iter()
                .filter(|v| *v >= domain.0 - step * 1e-9 && *v <= domain.1 + step * 1e-9)
                .collect();
            let labels = linear_labels(&major, step, fmt, decimals);
            let mut minor = Vec::new();
            if minors > 0 {
                let ms = step / (minors as f64 + 1.0);
                let mut v = first - step;
                while v <= last + step {
                    for k in 1..=minors {
                        let t = clean(v + k as f64 * ms, ms);
                        if t > domain.0 && t < domain.1 {
                            minor.push(t);
                        }
                    }
                    v += step;
                }
            }
            TickSet {
                major,
                labels,
                minor,
                domain,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::plain;

    fn labels(t: &TickSet) -> Vec<String> {
        t.labels.iter().map(|r| plain(r)).collect()
    }

    #[test]
    fn nice_linear() {
        let t = ticks(
            Scale::Linear,
            0.0,
            0.9,
            5,
            false,
            None,
            0,
            TickFormat::Auto,
            None,
        );
        let l = labels(&t);
        assert!(
            l.first().unwrap().starts_with('0') && l.last().unwrap().starts_with('1'),
            "{l:?}"
        );
        let t = ticks(
            Scale::Linear,
            3.2,
            97.0,
            6,
            false,
            None,
            0,
            TickFormat::Auto,
            None,
        );
        assert_eq!(t.major.first(), Some(&0.0));
        assert_eq!(t.major.last(), Some(&100.0));
        let t = ticks(
            Scale::Linear,
            -1.3,
            2.7,
            5,
            false,
            None,
            0,
            TickFormat::Auto,
            None,
        );
        assert!(labels(&t).contains(&"−1".to_string()) || labels(&t).contains(&"−2".to_string()));
        assert!(t.major.contains(&0.0));
    }

    #[test]
    fn no_floating_noise() {
        let t = ticks(
            Scale::Linear,
            0.1,
            0.7,
            7,
            false,
            Some(0.1),
            0,
            TickFormat::Auto,
            None,
        );
        assert!(labels(&t).iter().all(|l| l.len() <= 4), "{:?}", labels(&t));
        assert!(t.major.contains(&0.3));
    }

    #[test]
    fn scientific_for_large_and_small() {
        let t = ticks(
            Scale::Linear,
            0.0,
            3.0e6,
            4,
            false,
            None,
            0,
            TickFormat::Auto,
            None,
        );
        assert!(labels(&t).iter().any(|l| l.contains("×10") || l == "0"));
        let r = scientific(2.5e-5, 1);
        assert_eq!(plain(&r), "2.5×10−5");
        assert_eq!(plain(&scientific(1e6, 0)), "106");
        assert_eq!(scientific(1e6, 0)[1].shift, Shift::Super);
    }

    #[test]
    fn log_ticks_and_minors() {
        let t = ticks(
            Scale::Log10,
            0.3,
            2.7,
            5,
            false,
            None,
            1,
            TickFormat::Auto,
            None,
        );
        assert_eq!(t.major, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(labels(&t), ["1", "10", "100", "1000"]);
        assert_eq!(t.minor.len(), 8 * 3);
        assert!((t.minor[0] - 2f64.log10_d()).abs() < 1e-12);
        let t = ticks(
            Scale::Log10,
            -9.0,
            -3.0,
            4,
            false,
            None,
            0,
            TickFormat::Auto,
            None,
        );
        assert_eq!(plain(&t.labels[0]), "10−9");
        let t = ticks(
            Scale::Log10,
            0.0,
            3.0,
            4,
            false,
            None,
            0,
            TickFormat::Power,
            None,
        );
        assert_eq!(plain(&t.labels[1]), "101");
    }

    #[test]
    fn decimals() {
        assert_eq!(decimals_for(&[0.0, 2.5, 5.0], 2.5), 1);
        assert_eq!(decimals_for(&[0.0, 0.25, 0.5], 0.25), 2);
        assert_eq!(decimal(-0.0, 1), "0.0");
        assert_eq!(decimal(-2.0, 0), "−2");
    }
}
