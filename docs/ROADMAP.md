# Roadmap

What comes next, in order, and what waits, with the reason for each. The
[changelog](https://github.com/ZahakJ/dioptra/blob/main/CHANGELOG.md) lists what each
release contains.

## Next: statistics for real designs

Designs that need more than one-factor repeated measures and unit means.

- **Mixed-effects models** (REML), so repeated measures with missing values and nested
  data with several levels are analysed as what they are, with a model-formula layer under
  the guided dialogs.
- **Nested t test and nested one-way ANOVA** as mixed models, replacing the analysis of
  unit means.
- **Three-way ANOVA** and **two-way ANOVA with repeated measures on both factors**.
- **Multiple linear, logistic and Poisson regression**, and **Cox proportional hazards
  regression**, on a shared iteratively reweighted least squares and Newton
  infrastructure.
- **Interpolation from standard curves**, area under the curve, and row statistics.

## Then: extension and scale

- **A scripting surface** (Python or Lua).
- **A plugin system** for analyses and graph families.
- **Faster rendering for very large scatter plots** (more than a million points) with a
  canvas or WebGL renderer, if real use shows the need.
- **A system font picker** for figures. Arbitrary fonts can already be embedded in PDF; the
  missing part is the interface.

## Deferred, with reasons

| Item | Why it waits | What would unblock it |
|---|---|---|
| WebGL or canvas rendering for more than 10⁶ points | The engine's SVG with level-of-detail thinning covers typical data sizes; a second renderer doubles what must be kept in sync | A profiling case from real use |
| Hosted web app | The WebAssembly build exists and runs the end-to-end tests; hosting adds accounts, storage and support | A decision to run a service |
| Mixed-effects models, nested ANOVA inference | Need REML and a model-formula layer; nested tables ship with descriptives, unit-mean analyses and SuperPlots | Statistics for real designs |
| Multiple logistic regression, Cox regression | Need IRLS and Newton infrastructure | Statistics for real designs |
| Three-way ANOVA, two-way repeated measures on both factors | Two-way ANOVA has one repeated factor | Statistics for real designs |
| Scripting (Python or Lua) | The formula language and the file format cover reproducibility; a scripting surface needs a stable API | Extension and scale |
| Collaboration and cloud sync | The file diffs cleanly under git; real-time sync is out of scope | Not planned |
| Plugin system | Premature before the analysis API settles | Extension and scale |
| System font picker in figures | Bundled Inter keeps output identical on every system | The interface work |
| Localised interface | English only | Translators |
| Arrowheads on connecting lines | Prism offers them; needs a marker primitive in the scene and in the PDF path | A request |
| "Leave gap at symbols" for connecting lines | Points are drawn over the lines, which reads the same at journal sizes | A case where it does not |
| Geometric mean as a summary and for summary lines | Needs a log-scale summary through extraction, error bars and the inspector together | A request |
| Different styles per data set on one graph (bars for A, dots for B) | Families draw every data set alike; mixing needs per-data-set family overrides | Real figures that need it |
| Fill patterns on boxes and violins | Boxes and notched boxes are polygons; patterns are clipped to rectangles today | Clipping to polygons |
| Space before the first and after the last category, space for blank cells | `group_width` covers the gap between categories | A request |
| Labels of points by row title on before–after graphs | Callouts exist for scatter and volcano; categorical families do not use them | A request |
| Error-bar cap styles beyond a width (Prism's short/long) and error-bar line style | `cap_pt` covers the width; a dashed error bar is rare | A request |
| Axis origin options beyond "at the edges" and "at zero" | Prism also offers "custom" crossing points | A request |

Suggestions are welcome as GitHub issues. A request that comes with a data set and the
answer another program gives is the easiest kind to act on.
