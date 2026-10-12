# 06 — Connecting lines on column and grouped graphs

What Prism offers for joining points across bars ("before–after" lines, connecting each
replicate, connecting means), read from its own guides on 2026-10-11, and what Dioptra 0.3
does with it. Page links are next to each claim. Where the guide is silent, that is said.

---

## 1. Prism's column graphs (one-way data)

The Format Graph dialog of a column graph sets a **style per data set**: "simple bar (with or
without error bars), scatter plot, aligned dot plot, box and whiskers, before and after plot,
or floating bars", and styles can be mixed, e.g. before–after for data sets A and C and an
aligned dot plot for B and D
(https://www.graphpad.com/guides/prism/7/user-guide/using_format_graph_one_way.htm).

- **Before–after lines join the values of a row.** "Prism joins every point in a row", which is
  why unrelated data sets must be switched to another style
  (https://graphpad.com/support/faq/how-to-make-a-before-after-before-after-graph-with-prism).
  Matching is by row; there is no subject-ID column on a column table.
- **Line format.** The *Lines* section "sets the color and thickness of lines for means on
  scatter plots, and of the connecting lines on before and after plot"; the guide suggests
  arrows (same Format Graph page; the grouped page says "Choose the format of lines connecting
  data points for a before-and-after plot. Consider using arrows").
- **One colour for every pair.** On a column table "every pair the same color"; per-subject
  colours need a grouped table where each subject or group is its own data set, or right-
  clicking single symbols
  (https://graphpad.com/support/faq/before-after-graphs-with-different-colors-for-different-subjects).
- **No error bars on before–after graphs.** "Prism's before and after graphs don't have the
  option of having error bars"; the workaround is to duplicate the columns and overlay a
  mean-with-SEM style
  (https://www.graphpad.com/support/faq/graph-tip-how-to-show-mean-andor-error-information-on-a-before-after-graph/).
- **Mean line on scatter.** Scatter (dot) plots draw a mean or median line with error bars;
  "Symbol (one symbol per column)" plots the mean or median alone (Format Graph page above).
- **Bars and boxes.** Fill colour, border, and "a fill pattern checkbox enables a choice of
  pattern type and color" (Format Graph page above).
- **Row labels.** "Label each point with its row title" on before–after and scatter plots.

## 2. Prism's grouped graphs (two-way data)

- **Styles** per data set: scatter dot plot, aligned dot plot, before-and-after, column or
  bar, box-and-whiskers, floating bar
  (https://graphpad.com/guides/prism/latest/user-guide/using_format_graph_two_way.htm).
- **Arrangement and spacing.** Data sets are interleaved, stacked, placed apart or
  superimposed; the spacing between columns, between groups, before the first and after the
  last is set as a percent of the normal width, and blank cells can take no space, full space
  or anything between
  (https://www.graphpad.com/guides/prism/latest/user-guide/using_changing_the_spacing_between_b_2.htm).
- **Repeated measures, two directions.** For two-way RM ANOVA, *repeated measures by row*:
  set every data set to "Before-After"; *by column*: set the appearance to "Each replicate"
  and "check the option to plot one line for each subcolumn"
  (https://graphpad.com/guides/prism/latest/statistics/stat_graphing_tips_repeated_measure.htm).
  So on grouped tables subcolumn *k* is subject *k*, and lines run either within a group
  across its data sets, or along a data set across the groups.
- **Per-subject colours** come from putting groups or subjects in separate data sets and
  choosing "connect each replicate" (knowledgebase article above).
- **Mixing bars with lines.** A grouped graph can show some data sets as points with a
  connecting line and others as bars
  (https://www.graphpad.com/support/faq/graph-tip-combining-grouped-bars-with-xy-points-and-connecting-lines/).

## 3. Prism 11's categorical graphs (multiple-variables tables, Pro and Enterprise)

Prism 11 builds graphs from "data objects" (GDOs) that can be switched on and off: strip
plot, summary symbol plot, bar-and-strip, **before–after bar and strip plot** ("bars,
individual points, and connecting lines drawn across levels"), **before–after strip plot**,
box, violin, bar chart; a categorical variable on Y makes a horizontal graph; colour is a
single colour, a continuous variable or a categorical variable; level order is set on its own
tab (https://www.graphpad.com/guides/prism/latest/user-guide/mv_graphs_categorical.htm).

- **Connecting lines object**: "Show Connecting Lines"; one set of lines for all points, or
  "Connect points by group" chosen from a categorical **Variable** with per-level settings;
  colour, transparency, thickness, pattern and style per set; "Leave gap at symbols"; "When
  axis is discontinuous, also place gap in line" (recommended on)
  (https://www.graphpad.com/guides/prism/latest/user-guide/mv_graphs_connecting_line_opt.htm).
- **Summary connecting lines** "connect the summary values across the levels"; one point per
  group; Mean, Median or Geometric mean for the whole object; "groups with no data produce a
  gap in the line"; colour, thickness in points, style Solid, Dash, Dot, DashDot, DashDotDot;
  no colour legend
  (https://www.graphpad.com/guides/prism/latest/user-guide/mv_graphs_summary_connecting_lines.htm).
- **Error bars object**: direction "Both", "Above only", "Below only"; caps "None", "Short",
  "Long"; thickness in points; the same five styles; types mean with SD, SEM, CI or range,
  geometric mean with geometric SD or CI, median with range, CI or IQR
  (https://graphpad.com/guides/prism/latest/user-guide/mv_graphs_error_bars.htm).

**Not documented.** Neither guide says how individual connecting lines treat a missing value
(Prism's own column graphs simply have no point there), nor how lines meet jittered points.
Prism's aligned dot plot and before–after styles do not jitter; the scatter dot plot does.

---

## 4. Dioptra 0.3

Connecting lines are an option of the **column, strip, box and violin** families
(`options.connecting`), not a family of their own, so any bar style can carry them and a family
switch keeps them.

| Prism | Dioptra 0.3 |
|---|---|
| Before–after lines joining a row | `connect: individuals` (or `both`), matched by row |
| Connect means / summary connecting line (mean, median) | `connect: means`, `medians`; `both` uses the graph's summary |
| Line colour, thickness, style (solid, dash, dot) | `colour`, `width_pt`, `style` (solid, dashed, dotted, dash-dot); the summary line has its own three |
| Transparency | `opacity` (0–1, default 0.55) |
| One colour per pair (column tables) | `colour_by: single` |
| Colours per subject (needs a grouped table in Prism) | `colour_by: subject` with a `palette`, on any table |
| Connect points by group (categorical variable) | `colour_by: group` with `group_column`: lines coloured by a column's values, with a legend; the column is not plotted |
| Colour by data set | `colour_by: dataset`: each segment takes the colour of the bar it starts from |
| Gap for groups with no data (summary lines) | `missing: gap` (default) for both line kinds; `skip` joins the values either side |
| Gap at a broken axis | Always: every line is cut where it crosses an axis break |
| Grouped: before–after (by row) / each replicate (by column) | `direction: within_groups` (subcolumn k across the data sets of a group) / `across_groups` (each data set's subcolumn k across the groups) |
| Before–after has no error bars | Any bar style: bars, dot plot, mean ± SD/SEM/CI, box, violin, all with error bars where the style has them |
| Aligned (unjittered) vs scatter (jittered) | Lines attach to the jittered point positions, so lines and points always meet; jitter 0 gives the aligned look |
| Horizontal graph (categorical variable on Y) | `orientation: horizontal` on column, strip, box, violin; `axes.y` still means the value axis |
| Error-bar direction, caps | `error_direction` (both, above, below), `cap_pt` |
| Fill pattern | `style.datasets[].pattern` (diagonal either way, crosshatch, horizontal, vertical, dots) on bars |
| Spacing between bars and groups | `bar_width` (fraction of the slot), `group_width` (fraction of the category) |
| Arrows on lines, "leave gap at symbols", geometric mean, label points with row titles, per-data-set mixed styles | Deferred (docs/ROADMAP.md) |

Paired t, Wilcoxon, repeated-measures ANOVA and Friedman now default to the `paired` preset:
bars of the mean with SEM, the points unjittered, and a line per subject. Format 1.0 graphs of
the retired `before_after` family open as the `strip` spec that draws the same picture.
