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

## Version 0.3: extension and scale

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

Suggestions are welcome as GitHub issues. A request that comes with a data set and the
answer another program gives is the easiest kind to act on.
