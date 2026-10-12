//! Connecting lines for the categorical families (column, strip, box, violin): a line per
//! subject joining the values of a row across the categories (Prism's before–after
//! lines), and a line joining the means or medians.
//!
//! A family records where it drew each category's points in a [`Placed`] (row-aligned, at
//! the jittered positions), draws its bodies (bars, boxes, violins) and its marks (points,
//! error bars, summary ticks) into separate layers, and hands everything to [`assemble`],
//! which stacks them: bodies, subject lines, points and error bars, summary line.

use super::common::Cats;
use crate::plot::{Ctx, Layer, LegendEntry, Mark, Plot, Swatch};
use crate::scene::{Color, Dash, Stroke};
use crate::spec::{
    Connect, ConnectColour, ConnectDirection, ConnectingLines, LineStyle, MissingLines, Summary,
};
use crate::stats;
use crate::theme::categorical;

/// Where a family drew one category of one series.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    /// Series index.
    pub series: usize,
    /// Category (0-based position on the category axis).
    pub group: usize,
    /// Centre of the slot.
    pub at: f64,
    /// Point positions by row (`None`: no value in that row).
    pub rows: Vec<Option<(f64, f64)>>,
    /// The finite values (for means and medians).
    pub values: Vec<f64>,
    /// A precomputed centre for summary-format data (mean/SD/N), used when `values` is
    /// empty.
    pub given: Option<f64>,
}

impl Placed {
    /// Record a category from its row-aligned values and jitter offsets.
    pub fn new(series: usize, at: f64, raw: &[f64], offsets: &[f64]) -> Placed {
        Placed {
            series,
            group: (at.round() as usize).saturating_sub(1),
            at,
            rows: raw
                .iter()
                .enumerate()
                .map(|(r, v)| {
                    v.is_finite()
                        .then(|| (at + offsets.get(r).copied().unwrap_or(0.0), *v))
                })
                .collect(),
            values: raw.iter().copied().filter(|v| v.is_finite()).collect(),
            given: None,
        }
    }

    fn centre(&self, s: Summary) -> Option<f64> {
        let v = if self.values.is_empty() {
            self.given?
        } else {
            match s {
                Summary::Mean => stats::mean(&self.values),
                Summary::Median => stats::median(&self.values),
            }
        };
        v.is_finite().then_some(v)
    }
}

/// The dash of a line style (`None`: no line).
pub fn dash_of(s: LineStyle) -> Option<Dash> {
    match s {
        LineStyle::Solid => Some(Dash::Solid),
        LineStyle::Dashed => Some(Dash::Dashed),
        LineStyle::Dotted => Some(Dash::Dotted),
        LineStyle::DashDot => Some(Dash::DashDot),
        LineStyle::None => None,
    }
}

/// Whether connecting lines will be drawn for this spec.
pub fn active(ctx: &Ctx<'_>) -> bool {
    ctx.spec.options.connecting.connect != Connect::None
}

/// Split a sequence with holes into the runs a line is drawn through: runs of consecutive
/// values (`Gap`), or every value joined (`Skip`). Runs of fewer than two points vanish.
pub fn runs<T: Copy>(seq: &[Option<T>], missing: MissingLines) -> Vec<Vec<T>> {
    let mut out = Vec::new();
    match missing {
        MissingLines::Skip => {
            let v: Vec<T> = seq.iter().flatten().copied().collect();
            if v.len() >= 2 {
                out.push(v);
            }
        }
        MissingLines::Gap => {
            let mut cur = Vec::new();
            for p in seq {
                match p {
                    Some(p) => cur.push(*p),
                    None => {
                        if cur.len() >= 2 {
                            out.push(std::mem::take(&mut cur));
                        }
                        cur.clear();
                    }
                }
            }
            if cur.len() >= 2 {
                out.push(cur);
            }
        }
    }
    out
}

/// One chain of categories a line runs through, in drawing order.
struct Chain<'a> {
    /// Id fragment.
    key: String,
    members: Vec<&'a Placed>,
    /// The series, when every member belongs to one (lines across groups).
    series: Option<usize>,
}

fn chains<'a>(cats: &Cats, placed: &'a [Placed], dir: ConnectDirection) -> Vec<Chain<'a>> {
    let mut out = Vec::new();
    if !cats.clustered {
        let mut m: Vec<&Placed> = placed.iter().collect();
        m.sort_by(|a, b| a.at.total_cmp(&b.at));
        out.push(Chain {
            key: String::new(),
            members: m,
            series: None,
        });
        return out;
    }
    match dir {
        ConnectDirection::WithinGroups => {
            let mut groups: Vec<usize> = placed.iter().map(|p| p.group).collect();
            groups.sort_unstable();
            groups.dedup();
            for g in groups {
                let mut m: Vec<&Placed> = placed.iter().filter(|p| p.group == g).collect();
                m.sort_by(|a, b| a.at.total_cmp(&b.at));
                out.push(Chain {
                    key: format!("g{g}-"),
                    members: m,
                    series: None,
                });
            }
        }
        ConnectDirection::AcrossGroups => {
            let mut series: Vec<usize> = placed.iter().map(|p| p.series).collect();
            series.sort_unstable();
            series.dedup();
            // Every group position, so a series missing from a group leaves a hole.
            let mut groups: Vec<usize> = placed.iter().map(|p| p.group).collect();
            groups.sort_unstable();
            groups.dedup();
            for i in series {
                let m: Vec<&Placed> = placed.iter().filter(|p| p.series == i).collect();
                let mut ordered: Vec<&Placed> = Vec::new();
                for g in &groups {
                    if let Some(p) = m.iter().find(|p| p.group == *g) {
                        ordered.push(p);
                    }
                }
                out.push(Chain {
                    key: format!("s{i}-"),
                    members: ordered,
                    series: Some(i),
                });
            }
        }
    }
    out
}

/// Does `members[j]` directly follow `members[j-1]` along the chain's axis? In chains
/// across groups, a series absent from a group in between is a missing value.
fn holes(c: &Chain<'_>, all_groups: &[usize]) -> Vec<bool> {
    // true at j when a missing category sits before members[j].
    let mut out = vec![false; c.members.len()];
    if c.series.is_none() {
        return out;
    }
    let pos = |p: &Placed| all_groups.iter().position(|g| *g == p.group);
    for (o, w) in out.iter_mut().skip(1).zip(c.members.windows(2)) {
        if let (Some(a), Some(b)) = (pos(w[0]), pos(w[1])) {
            *o = b > a + 1;
        }
    }
    out
}

/// A sequence with the missing categories of an across-groups chain put back as holes.
fn with_holes<T: Copy>(vals: Vec<Option<T>>, gaps: &[bool]) -> Vec<Option<T>> {
    let mut out = Vec::with_capacity(vals.len() * 2);
    for (j, v) in vals.into_iter().enumerate() {
        if gaps.get(j).copied().unwrap_or(false) {
            out.push(None);
        }
        out.push(v);
    }
    out
}

struct Lines {
    subjects: Vec<Layer>,
    summary: Option<Layer>,
    legend: Vec<LegendEntry>,
}

fn build(ctx: &Ctx<'_>, cats: &Cats, placed: &[Placed]) -> Lines {
    let o: &ConnectingLines = &ctx.spec.options.connecting;
    let t = &ctx.theme;
    let mut res = Lines {
        subjects: Vec::new(),
        summary: None,
        legend: Vec::new(),
    };
    if o.connect == Connect::None || placed.is_empty() {
        return res;
    }
    let n = ctx.data.series.len();
    let series_colour = |i: usize| {
        let s = &ctx.data.series[i.min(n.saturating_sub(1))];
        ctx.style(i, n, s.dataset, &s.name).colour
    };
    let mut all_groups: Vec<usize> = placed.iter().map(|p| p.group).collect();
    all_groups.sort_unstable();
    all_groups.dedup();
    let chains = chains(cats, placed, o.direction);
    let opacity = o.opacity.clamp(0.0, 1.0);

    if o.connect.individuals() {
        if let Some(dash) = dash_of(o.style) {
            let width = o.width_pt.filter(|w| *w >= 0.0).unwrap_or(t.axis_pt);
            let rows = placed.iter().map(|p| p.rows.len()).max().unwrap_or(0);
            let single = o
                .colour
                .as_deref()
                .and_then(Color::hex)
                .unwrap_or_else(|| ctx.fg());
            let palette = o.palette.clone().unwrap_or_else(|| ctx.palette.clone());
            let night = t.name == "night";
            // Group colouring: distinct values in order of first appearance.
            let groups: Vec<String> = {
                let mut g: Vec<String> = Vec::new();
                for v in &ctx.data.row_groups {
                    if !v.is_empty() && !g.contains(v) {
                        g.push(v.clone());
                    }
                }
                g
            };
            let by_group = o.colour_by == ConnectColour::Group && !groups.is_empty();
            let subject_cols = if by_group {
                categorical(&palette, groups.len(), night)
            } else {
                categorical(&palette, rows.max(1), night)
            };
            let colour_of = |r: usize, series: usize| -> Color {
                match o.colour_by {
                    ConnectColour::Single => single,
                    ConnectColour::Dataset => series_colour(series),
                    ConnectColour::Subject => subject_cols[r % subject_cols.len()],
                    ConnectColour::Group => {
                        let gi = ctx
                            .data
                            .row_groups
                            .get(r)
                            .and_then(|v| groups.iter().position(|g| g == v));
                        match gi {
                            Some(gi) if by_group => subject_cols[gi % subject_cols.len()],
                            _ if by_group => single,
                            _ => subject_cols[r % subject_cols.len()],
                        }
                    }
                }
            };
            for c in &chains {
                let gaps = holes(c, &all_groups);
                for r in 0..rows {
                    let seq: Vec<Option<((f64, f64), usize)>> = c
                        .members
                        .iter()
                        .map(|p| p.rows.get(r).copied().flatten().map(|pt| (pt, p.series)))
                        .collect();
                    let seq = with_holes(seq, &gaps);
                    let mut layer =
                        Layer::new(format!("subject-{}{}", c.key, r + 1), "/options/connecting");
                    for run in runs(&seq, o.missing) {
                        let per_segment =
                            o.colour_by == ConnectColour::Dataset && c.series.is_none();
                        if per_segment {
                            for w in run.windows(2) {
                                let col = colour_of(r, w[0].1).alpha(opacity);
                                layer.marks.push(Mark::Line {
                                    pts: vec![w[0].0, w[1].0],
                                    stroke: Stroke::solid(col, width).dashed(dash),
                                    step: false,
                                });
                            }
                        } else {
                            let col = colour_of(r, run[0].1).alpha(opacity);
                            layer.marks.push(Mark::Line {
                                pts: run.iter().map(|p| p.0).collect(),
                                stroke: Stroke::solid(col, width).dashed(dash),
                                step: false,
                            });
                        }
                    }
                    if !layer.marks.is_empty() {
                        res.subjects.push(layer);
                    }
                }
            }
            if by_group {
                for (gi, g) in groups.iter().enumerate() {
                    res.legend.push(LegendEntry {
                        label: g.clone(),
                        swatch: Swatch::Line(
                            Stroke::solid(subject_cols[gi % subject_cols.len()], width.max(1.0))
                                .dashed(dash),
                            None,
                        ),
                        dataset: None,
                    });
                }
            }
        }
    }

    let summary = match o.connect {
        Connect::Means => Some(Summary::Mean),
        Connect::Medians => Some(Summary::Median),
        Connect::Both => Some(ctx.spec.options.summary),
        _ => None,
    };
    if let (Some(stat), Some(dash)) = (summary, dash_of(o.summary_style)) {
        let width = o
            .summary_width_pt
            .filter(|w| *w >= 0.0)
            .unwrap_or(t.data_pt * 2.0);
        let fixed = o.summary_colour.as_deref().and_then(Color::hex);
        let mut layer = Layer::new("connect-summary", "/options/connecting");
        for c in &chains {
            let gaps = holes(c, &all_groups);
            let seq: Vec<Option<(f64, f64)>> = c
                .members
                .iter()
                .map(|p| p.centre(stat).map(|v| (p.at, v)))
                .collect();
            let colour = fixed.unwrap_or_else(|| match c.series {
                Some(i) => series_colour(i),
                None => ctx.fg(),
            });
            for run in runs(&with_holes(seq, &gaps), o.missing) {
                layer.marks.push(Mark::Line {
                    pts: run,
                    stroke: Stroke::solid(colour, width).dashed(dash),
                    step: false,
                });
            }
        }
        if !layer.marks.is_empty() {
            res.summary = Some(layer);
        }
    }
    res
}

/// Stack a categorical family's layers. `bodies[i]` and `marks[i]` are series `i`'s
/// bodies (bars, boxes, violins) and marks (points, error bars, summary ticks), both
/// carrying the series' id and target. Without connecting lines each series stays one
/// layer (bodies, then marks); with them the order is every body, the subject lines, every
/// series' marks (`series-i-points`), then the summary line.
pub fn assemble(
    ctx: &Ctx<'_>,
    p: &mut Plot,
    cats: &Cats,
    bodies: Vec<Layer>,
    marks: Vec<Layer>,
    placed: &[Placed],
) {
    let lines = build(ctx, cats, placed);
    if lines.subjects.is_empty() && lines.summary.is_none() {
        for (mut b, m) in bodies.into_iter().zip(marks) {
            b.marks.extend(m.marks);
            p.layers.push(b);
        }
        return;
    }
    p.layers
        .extend(bodies.into_iter().filter(|b| !b.marks.is_empty()));
    p.layers.extend(lines.subjects);
    for mut m in marks {
        if m.marks.is_empty() {
            continue;
        }
        m.id = format!("{}-points", m.id);
        p.layers.push(m);
    }
    p.layers.extend(lines.summary);
    p.legend.extend(lines.legend);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_and_skips() {
        let seq = [Some(1), None, Some(2), Some(3), None, Some(4)];
        assert_eq!(runs(&seq, MissingLines::Gap), vec![vec![2, 3]]);
        assert_eq!(runs(&seq, MissingLines::Skip), vec![vec![1, 2, 3, 4]]);
        assert!(runs(&[Some(1), None], MissingLines::Skip).is_empty());
    }

    #[test]
    fn placed_rows_follow_offsets() {
        let p = Placed::new(0, 2.0, &[1.0, f64::NAN, 3.0], &[0.1, 0.0, -0.1]);
        assert_eq!(p.rows, vec![Some((2.1, 1.0)), None, Some((1.9, 3.0))]);
        assert_eq!(p.group, 1);
        assert_eq!(p.centre(Summary::Mean), Some(2.0));
    }
}
