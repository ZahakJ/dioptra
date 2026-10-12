# Roadmap

Version 0.1.0 covers the everyday work of a bench scientist: eight table kinds, formulas
and transforms, twenty analyses with assumption checks, twenty-three graph families, and
export to SVG, PDF and PNG. This page says what comes next and what waits, with the reason
for each. Dates are not promised; the order is.

## Version 0.2: statistics for real designs

The theme is designs that 0.1 cannot analyse properly.

- **Mixed-effects models** (REML), so repeated measures with missing values and nested
  data with several levels are analysed as what they are, with a model-formula layer under
  the guided dialogs.
- **Nested t test and nested one-way ANOVA** as mixed models, replacing the analysis of
  unit means.
- **Three-way ANOVA** and **two-way ANOVA with repeated measures on both factors**.
- **Multiple linear, logistic and Poisson regression**, and **Cox proportional hazards
  regression**, on a shared iteratively reweighted least squares and Newton
  infrastructure. (Cox is already implemented and tested in the engine; it needs the
  regression interface.)
- **Interpolation from standard curves**, area under the curve, and row statistics.

## Shipped in 0.3.0: connecting lines and the bar-graph pass

Prism's before–after lines, "connect each replicate" and summary connecting lines, as an
option of every categorical graph rather than a family of its own (research:
`research/06-connecting-lines.md`).

- **Connecting lines** on column, strip (dot plot, mean ± error), box and violin graphs:
  subjects (rows), means, medians or both; style, width, opacity; one colour, by data set,
  per subject from a palette, or by a grouping column with a legend; gap or join across a
  missing value; within groups or across groups on grouped tables; lines attached to the
  jittered points; cut at axis breaks; each subject line selectable in the editor.
- **Paired analyses graph with connecting lines by default** (paired and ratio-paired t,
  Wilcoxon, repeated-measures ANOVA, Friedman).
- Closed while comparing with Prism's column and grouped Format Graph:
  **horizontal orientation** (column, strip, box, violin), **error-bar direction** (both,
  above, below) and **cap width**, **bar width** and **slot width** (the gap between bars
  and between groups), **bar fill** (tint, solid, outline), **fill patterns** per data set,
  **open markers** on column graphs (they were drawn filled), the **strip graph without points**
  (mean ± SD/SEM/CI only) and **without its mean line**, and a **Bars and points** inspector
  section for options that had no control (summary, jitter).
- File format 1.1 with a migration for the retired `before_after` family.

Deferred from this pass, with reasons, in the table below.

## Version 0.4: extension and scale

- **A scripting surface** (Python or Lua) once the analysis API has been stable for a
  release.
- **A plugin system** for analyses and graph families, for the same reason.
- **Faster rendering for very large scatter plots** (more than a million points) with a
  canvas or WebGL renderer, if real use shows the need.
- **A system font picker** for figures. Arbitrary fonts can already be embedded in PDF; the
  missing part is the interface.

## Deferred, with reasons

| Item | Why it waits | What would unblock it |
|---|---|---|
| WebGL or canvas rendering for more than 10⁶ points | The engine's SVG with level-of-detail thinning covers 0.1 data sizes; a second renderer doubles what must be kept in sync | A profiling case from real use |
| Hosted web app | The WebAssembly build exists and runs the end-to-end tests; hosting adds accounts, storage and support | A decision to run a service |
| Mixed-effects models, nested ANOVA inference | Need REML and a model-formula layer; nested tables ship with descriptives, unit-mean analyses and SuperPlots | Version 0.2 |
| Multiple logistic regression, Cox regression | Need the IRLS and Newton infrastructure planned for 0.2 | Version 0.2 |
| Three-way ANOVA, two-way repeated measures on both factors | Two-way with one repeated factor is the 0.1 ceiling | Version 0.2 |
| Writing Prism `.pzfx` | Reading is enough to move data out; writing a proprietary format is not a goal | A strong request |
| Scripting (Python or Lua) | The formula language and the file format cover reproducibility; a scripting surface needs a stable API | Version 0.3 |
| Collaboration and cloud sync | The file diffs cleanly under git; real-time sync is out of scope | Not planned |
| Plugin system | Premature before the analysis API settles | Version 0.3 |
| System font picker in figures | Bundled Inter keeps output identical on every system | The interface work |
| Localised interface | English only so far | Translators |
| Arrowheads on connecting lines | Prism offers them; needs a marker primitive in the scene and in the PDF path | A request |
| "Leave gap at symbols" for connecting lines | Points are drawn over the lines, which reads the same at journal sizes | A case where it does not |
| Geometric mean as a summary and for summary lines | Needs a log-scale summary through extraction, error bars and the inspector together | A request |
| Different styles per data set on one graph (bars for A, dots for B) | Families draw every data set alike; mixing needs per-data-set family overrides | Real figures that need it |
| Fill patterns on boxes and violins | Boxes and notched boxes are polygons; patterns are clipped to rectangles today | Clipping to polygons |
| Space before the first and after the last category, space for blank cells | `group_width` covers the gap between categories | A request |
| Labels of points by row title on before–after graphs | Callouts exist for scatter and volcano; categorical families do not use them yet | A request |
| Error-bar cap styles beyond a width (Prism's short/long) and error-bar line style | `cap_pt` covers the width; a dashed error bar is rare | A request |
| Axis origin options beyond "at the edges" and "at zero" | Prism also offers "custom" crossing points | A request |

Suggestions are welcome as GitHub issues. A request that comes with a data set and the
answer another program gives is the easiest kind to act on.
