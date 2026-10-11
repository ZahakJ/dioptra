# Graphs

How Dioptra draws figures: the graph specification the UI edits, the data each family
needs, the options each family reads, the themes and palettes, and the hit-test targets
the graph editor uses. The engine is `crates/dioptra-plot`; `dioptra-core` re-exports
its specification types, so the JSON below is exactly what `create_graph`,
`update_graph`, `patch_graph` and `get_graph_spec` carry.

## The pipeline

1. `dioptra_core::graph::extract` reads the source table into `GraphData`: series with
   X values (or category positions 1, 2, 3, …), Y values, raw replicates per X (missing
   cells kept as NaN so rows line up for paired graphs), precomputed error bounds for
   mean/SD/N-style tables, subgroup ids for nested tables, curves and at-risk tables.
2. The family turns spec + data into a *plot*: axis plans and marks in data units.
3. The layout measures every label with the bundled Inter metrics, sizes the margins so
   tick labels, axis titles, legends, colour bars and tables never collide, maps marks to
   points, clips them geometrically (no clipping masks in exported files) and emits a
   *scene* in points.
4. The scene becomes SVG (own writer), PNG (resvg, any DPI) or PDF (svg2pdf, fonts
   embedded as subsets). The SVG shown in the app is the SVG exported.

Sizes are in millimetres; strokes, ticks and fonts in points (1 pt = 1/72 in). The
default figure is 85 × 70 mm (a single journal column), 8 pt tick labels, 9 pt titles.
PNG pixel size is `round(mm / 25.4 × dpi)`: 85 mm at 300 dpi is 1003.94 → 1004 px. PNGs
carry the DPI in a `pHYs` chunk.

## The specification

Every field has a default, so `{"source": "<table id>", "family": "column"}` is a valid
spec, and merge patches can set one field at a time. Style fields that are absent mean
"the theme decides".

```json
{
  "source": "<table or analysis id>",
  "family": "column",
  "title": "Optional heading",
  "mapping": {
    "datasets": [0, 2],            // which datasets (empty: all)
    "x": 0, "y": [1, 2],           // flat columns, variables tables only
    "label": 3,                    // flat column with point labels (volcano)
    "error_bars": "sem",           // none | sd | sem | ci95 | range | iqr
    "points": true                 // data points over summaries
  },
  "style": {
    "theme": "instrument",         // instrument | night | journal
    "palette": "okabe_ito",        // absent: theme default
    "size_mm": {"width": 85, "height": 70},
    "font_pt": 8,                  // tick labels; titles are +1 pt
    "line_pt": 0.75,               // axis lines; data lines scale with it
    "marker_pt": 4,
    "transparent": false,
    "datasets": [{"dataset": 1, "colour": "#D55E00", "marker": "square_open",
                  "marker_pt": 5, "line_style": "dashed", "line_pt": 1.2,
                  "label": "Knock-out"}]
  },
  "axes": {
    "x": {"title": "Dose (nM)", "scale": "log10", "min": 0.1, "max": 1000,
          "breaks": [{"from": 20, "to": 80}],
          "ticks": {"major": 1, "minor": 1, "direction": "out", "length_pt": 3,
                    "format": "auto", "decimals": 1, "label_angle": 45},
          "hidden": false},
    "y": {"title": "Response (% of max)"},
    "frame": "l",                  // l | box | none
    "origin": "frame",             // frame | zero
    "grid": false,
    "offset_pt": 0
  },
  "legend": {"position": "auto", "title": "Genotype", "at": {"x": 0.05, "y": 0.05}},
  "options": { "...": "family options, below" },
  "annotations": [
    {"kind": "significance_bracket", "from": 0, "to": 2, "text": "p < 0.001"},
    {"kind": "text_label", "text": "R^2 = 0.98", "x": 2, "y": 15, "size_pt": 8},
    {"kind": "line", "x1": 0, "y1": 0, "x2": 10, "y2": 10, "dashed": true},
    {"kind": "reference_line", "axis": "y", "value": 10, "label": "LLOQ", "dashed": true}
  ]
}
```

(Comments are for this document only; JSON has none.)

Notes:

- **Text markup** in titles, labels and category names: `^{…}` and `_{…}` for super- and
  subscripts (`^2`, `^-1` and `_i` take one character or a signed number), `\alpha` …
  `\omega`, `\Delta`, `\mu`, `\degree`, `\pm`, `\times`, `\le`, `\ge`, `\infty`, `\permil`
  for symbols (a following space or `{}` ends the name, as in TeX), `\^`, `\_`, `\\` for
  the literal characters. A newline in a category name forces a line break.
- **Ticks** are placed by the extended Wilkinson algorithm on 1, 2, 2.5 and 5 × 10ⁿ
  steps; the axis ends on a tick unless data sit on the last tick, in which case it runs
  4% past it. Numbers never show floating noise; they switch to `m×10ⁿ` for |x| ≥ 10⁶ or
  ≤ 10⁻⁴, and use a typographic minus. Log axes put majors on powers of ten (1, 10, 100
  inside 0.001…10 000, 10ⁿ outside, or as `format` asks), minors on 2…9 × 10ⁿ, and
  switch to 1–2–5 steps when the data span less than about a decade. Labels that would
  collide are thinned (the dropped ones stay as minor ticks); long category labels wrap
  onto up to three lines before they rotate to 45°.
- **Breaks** cut the axis, bars and gridlines and draw a gap glyph.
- **Legend position** `auto` lets the family decide: right of the plot for most, inside
  top right for survival, inside bottom right for ROC, above a volcano plot. `at` places
  the legend's top-left corner at fractions of the plot area (set by dragging).
- **Significance brackets** run between 0-based categories; with no `y` they sit above
  the highest point, bar or error bar in their span and stack when they would overlap.
- **Markers**: `circle`, `square`, `triangle`, `triangle_down`, `diamond`, `plus`,
  `cross`, `none`, and `_open` variants (`circle_open`). Line styles: `solid`, `dashed`,
  `dotted`, `dash_dot`, `none`.

### Family options (`options`)

| Field | Default | Used by |
|---|---|---|
| `summary` | `mean` | column, strip, superplot, before_after (`mean` or `median`) |
| `arrangement` | `grouped` | column on grouped tables (`grouped`, `stacked`, `percent`) |
| `connect` | `false` | scatter: join points |
| `notch` | `false` | box |
| `violin_inner` | `box` | violin (`box`, `strip`, `quartiles`, `none`) |
| `truncate` | `true` | violin, raincloud: cut the density at the data range |
| `bandwidth` | Silverman | violin, raincloud, density |
| `bin_width`, `bins` | Freedman–Diaconis | histogram |
| `density` | `false` | histogram: density instead of counts |
| `jitter` | `0.5` | column, box, strip, raincloud, estimation, superplot |
| `mean_line` | `true` | before_after |
| `bootstrap`, `seed` | `5000`, `20261011` | estimation (and the jitter phase) |
| `cluster_rows`, `cluster_columns` | `false` | heatmap |
| `cell_values` | `false` | heatmap |
| `ci_band`, `censor_ticks`, `at_risk` | `true` | kaplan_meier |
| `fc_threshold`, `p_threshold`, `label_top` | `1`, `0.05`, `5` | volcano |
| `reference` | 0 (1 on log axes) | forest |
| `donut_hole` | `0.55` | donut |

## Families

Each entry says what data it draws, what it computes, and gives a minimal spec. Every
family honours the theme, palette, per-dataset styles, sizes, axis settings, legends and
annotations.

### scatter
XY points. Data: XY tables (X column, Y datasets with replicates or mean/SD/N formats) or
variables tables with `mapping.x`/`mapping.y`. Replicates are drawn as their mean (or
median) with error bars as `error_bars` asks; with `error_bars: "none"` and `points`
the raw replicates are drawn faintly. X error bars come from the data. Fit curves and
CI bands in `GraphData.curves` are drawn under the points in the series colour.
`{"source": "t1", "family": "scatter", "mapping": {"error_bars": "sem"}}`

### line
As scatter with every series joined by a line in its line style, markers on.
`{"source": "t1", "family": "line", "style": {"datasets": [{"dataset": 0, "line_style": "dashed"}]}}`

### column
Bars of the mean or median with error bars and the points on top (default). Column
tables give one bar per dataset; grouped tables give clusters by row with a legend;
`arrangement` stacks them or scales stacks to 100%. On log Y axes bars start at the
axis. Dark bars get upward error bars only.
`{"source": "t2", "family": "column", "mapping": {"error_bars": "sd", "points": true}}`

### box
Tukey box plots: quartiles (type 7), whiskers to the furthest point within 1.5 IQR,
outliers as open points; `notch` draws median ± 1.58 IQR/√n; `points` shows every value.
`{"source": "t2", "family": "box", "options": {"notch": true}}`

### violin
Gaussian kernel densities mirrored about each category, cut at the data range unless
`truncate` is false, with a slim box (default), the points, or quartile lines inside.
`{"source": "t2", "family": "violin", "options": {"violin_inner": "quartiles"}}`

### strip
All points, jittered deterministically (density-scaled, golden-ratio sequence), with the
mean or median line and error bars.
`{"source": "t2", "family": "strip", "mapping": {"error_bars": "sd"}}`

### raincloud
Half violin (cloud), slim box and jittered points (rain) per category.
`{"source": "t2", "family": "raincloud"}`

### before_after
Paired values: each row is a subject joined across datasets by a grey line; the mean (or
median) is a heavy line. Rows must align (column tables keep empty cells in place).
`{"source": "t3", "family": "before_after", "options": {"mean_line": true}}`

### estimation
Gardner–Altman: raw data with mean ± SD as a gapped line beside each group; on the
right, a floating axis anchored at the control mean (the first dataset) with each
group's mean difference, its bootstrap distribution and its bias-corrected and
accelerated 95% CI (5000 resamples, fixed seed, so the figure is reproducible).
`{"source": "t3", "family": "estimation", "options": {"bootstrap": 5000, "seed": 20261011}}`

### superplot
Nested tables: small points for every observation coloured (journal: shaped) by
subgroup, a large point per subgroup mean, and the mean ± error of the subgroup means,
so the error bar reflects biological replicates, not cells.
`{"source": "nested1", "family": "superplot", "mapping": {"error_bars": "sem"}}`

### histogram
Shared bins (Freedman–Diaconis width rounded to 1, 2 or 5 × 10ⁿ, or `bin_width`/`bins`),
counts or density, translucent when several datasets overlap.
`{"source": "t2", "family": "histogram", "options": {"bin_width": 0.5}}`

### density
Kernel density curves with a light fill, one per dataset.
`{"source": "t2", "family": "density"}`

### heatmap
Rows are table rows (row titles), columns are datasets; cells coloured on viridis
(sequential) or Tol sunset (diverging, symmetric around 0 when the data cross it), with
a colour bar. Rows and columns can be reordered by average-linkage clustering on
Euclidean distance with dendrograms; `cell_values` prints the numbers.
`{"source": "g1", "family": "heatmap", "options": {"cluster_rows": true, "cluster_columns": true}}`

### correlation_matrix
Pearson correlations between datasets (variables, paired by row), lower triangle,
coefficients printed, diverging scale from −1 to 1, square cells.
`{"source": "vars1", "family": "correlation_matrix"}`

### kaplan_meier
Survival tables (X = time, 1 = event, 0 = censored): step curves in percent, censor
ticks, 95% CI bands (exponential Greenwood, log(−log) scale) and the number at risk under
each X tick. Curves supplied by an analysis are drawn instead when present.
`{"source": "surv1", "family": "kaplan_meier", "options": {"ci_band": false}}`

### roc
The first dataset holds controls, each later one a patient group (higher values mean
disease): empirical ROC curves in percent, the chance diagonal, AUC in the legend.
`{"source": "t3", "family": "roc"}`

### bland_altman
The first two datasets paired by row: difference against mean, bias and 95% limits of
agreement (bias ± 1.96 SD) with their 95% CIs shaded and labelled.
`{"source": "t3", "family": "bland_altman"}`

### volcano
Variables table with `mapping.x` = log₂ fold change and `mapping.y` = p values (or
−log₁₀ p): threshold lines, up/down/not significant colouring with counts in the
legend, and labels (from `mapping.label` or row titles) for the `label_top` most
significant points, placed to avoid each other.
`{"source": "rnaseq", "family": "volcano", "mapping": {"x": 0, "y": [1], "label": 2}}`

### forest
One row per estimate, top to bottom, with its CI, a reference line and the numbers in a
column on the right. Data: a dataset in a `lower_upper`, `mean_ci_n` or mean/SD format
(the error bounds), or three datasets read as estimate, lower, upper; row titles label
the rows. Log X axes suit ratios.
`{"source": "or1", "family": "forest", "axes": {"x": {"scale": "log10"}}}`

### pie, donut
Parts of a whole from a parts table (or one value per dataset), clockwise from twelve
o'clock, percentages in slices of 5% or more, shares in the legend. Slice `k` takes its
colour from `style.datasets` entries with `dataset: k`.
`{"source": "parts1", "family": "donut", "options": {"donut_hole": 0.6}}`

### qq
Sample quantiles against normal quantiles (Blom positions) with a line through the
quartiles, per dataset.
`{"source": "t2", "family": "qq"}`

### residuals
Observed minus fitted against X: the fitted curve of the series from the analysis when
supplied, otherwise a least-squares line; a dashed zero line.
`{"source": "t1", "family": "residuals"}`

## Themes

| Theme | Background | Axes | Type | Lines | Notes |
|---|---|---|---|---|---|
| `instrument` (default) | white | #222, 0.75 pt, L frame, ticks out 3 pt | Inter 8 pt ticks, 9 pt titles, 10 pt heading | data 1 pt, markers 4 pt | no gridlines; bars and boxes filled with a 70% tint of the series colour |
| `night` | #16191a | #c9cdca | light text #e8eae8 | as instrument | for slides; the first Okabe–Ito colour becomes light grey |
| `journal` | white | black, 0.5 pt | Inter 7 pt / 8 pt (Inter runs wider than Helvetica, so a journal that substitutes Helvetica still fits) | data 0.75 pt, markers 3.5 pt | black and white: series differ by marker shape and fill (●○■□▲△◆◇) and dash; heatmaps keep a colour-blind-safe scale (set `palette: "grey"` for grey) |

`font_pt`, `line_pt` and `marker_pt` override the theme; titles keep their offset from
`font_pt`, data lines scale with `line_pt`.

## Palettes

| Name | Kind | Colours |
|---|---|---|
| `okabe_ito` (default) | categorical | #000000 #E69F00 #56B4E9 #009E73 #0072B2 #D55E00 #CC79A7 #F0E442 (yellow last for white paper) |
| `tol_bright` | categorical | #4477AA #EE6677 #228833 #CCBB44 #66CCEE #AA3377 #BBBBBB |
| `tol_muted` | categorical | #332288 #88CCEE #44AA99 #117733 #999933 #DDCC77 #CC6677 #882255 #AA4499 |
| `tol_light` | categorical | #77AADD #EE8866 #EEDD88 #FFAABB #99DDFF #44BB99 #BBCC33 #AAAA00 #DDDDDD |
| `viridis` | sequential | 11 stops #440154 … #FDE725; categorical use samples 0–0.85 |
| `cividis` | sequential | 11 stops #00224E … #FEE838 |
| `tol_sunset` | diverging | #364B9A … #EAECCC … #A50026 (11 stops) |
| `grey` | categorical / sequential | #000000 #666666 #999999 #BBBBBB #333333; ramp #F4F4F4 → #111111 |

Colours are tokens: a per-dataset `colour` overrides the palette.

## Hit-test targets (graph editor)

`render_graph` and `render_layout` answer

```json
{"type": "svg", "svg": "<svg …>", "width_mm": 85, "height_mm": 70,
 "targets": [{"id": "axis-x-title", "target": "/axes/x/title",
              "bbox": {"x": 92.4, "y": 182.1, "width": 54.3, "height": 8.7}}]}
```

`bbox` is in points in the SVG's `viewBox` coordinates (the SVG is `width_mm` wide;
multiply by the displayed pixels per point to draw a selection box). Every `<g>` with an
`id` also carries `data-target` with the same pointer, so a click on any SVG element can
walk up to its group. Targets, in drawing order:

| id | target | what |
|---|---|---|
| `plot-area` | `/mapping` | all data marks |
| `series-{i}` | `/style/datasets/{d}` | one plotted series; `d` is its dataset index (find or create the `style.datasets` entry with `dataset: d`) |
| `curve-{i}`, `pairs`, `summary`, `cells`, `reference`, `thresholds`, `labels`, `difference-{j}`, `agreement` | `/mapping` or `/options/…` | family layers |
| `annotation-{i}` | `/annotations/{i}` | a bracket, label or line |
| `grid` | `/axes/grid` | gridlines |
| `axis-x`, `axis-y` | `/axes/x`, `/axes/y` | axis line, ticks and labels |
| `axis-x-labels`, `axis-y-labels` | `/axes/x/ticks`, `/axes/y/ticks` | tick labels |
| `axis-x-title`, `axis-y-title` | `/axes/x/title`, `/axes/y/title` | axis titles |
| `axis-y2`, `axis-y2-title` | `/axes/y`, `/axes/y/title` | estimation difference axis |
| `axis-x-break`, `axis-y-break` | `/axes/x/breaks`, `/axes/y/breaks` | gap glyphs |
| `frame` | `/axes/frame` | box frame |
| `colour-bar` | `/style/palette` | heatmap scale |
| `dendrogram-rows`, `dendrogram-columns` | `/options/cluster_rows`, `/options/cluster_columns` | |
| `estimates` | `/options` | forest number column |
| `at-risk` | `/options/at_risk` | survival table |
| `legend`, `legend-item-{i}` | `/legend`, `/style/datasets/{d}` | legend and its entries |
| `title` | `/title` | heading |

In layouts, panel `i` is `panel-{i}` → `/graphs/{i}` and every id inside it is prefixed
`p{i}-` (`p1-axis-x`).

## Determinism

The same spec and data give the same bytes on every platform and in the browser:
fonts are bundled (Inter Regular, SemiBold, Bold, Italic) and measured from their own
tables, PNG and PDF never consult system fonts, jitter and bootstrap use a fixed-seed
SplitMix64 generator, and no output depends on hash-map order. Golden SVGs for every
family are in `crates/dioptra-plot/tests/snapshots/`; regenerate them with
`UPDATE_SNAPSHOTS=1 cargo test -p dioptra-plot`. Series with more than 20,000 points are
decimated to one point per 0.75 pt cell (a pixel at 96 dpi) at render time, and the SVG
says so in a comment.
