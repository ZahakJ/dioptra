# Graphs

A graph in Dioptra is a specification, not a picture: which table or analysis it draws,
which family (column, box, violin, scatter, Kaplan–Meier, and so on), how data map to
marks, a style, and annotations. The plot engine turns the specification into a scene
measured in points and writes it as SVG. What you see in the editor is that SVG; what you
export is the same SVG, or a PDF or PNG made from it. There is no separate "export
renderer" that can disagree with the screen.

This page is about using the editor. The [graph reference](reference.md) lists every
family with the data it needs and the options it reads, the themes and palettes in
detail, and the specification format.

## Making a graph

- From a table: **Graph → New graph** (Ctrl-Shift-G). The family defaults to the natural
  one for the table kind: scatter for XY, Kaplan–Meier for survival, SuperPlot for nested,
  column with points for the others.
- From a results sheet: **Make graph**. Brackets for the comparisons, fitted curves and
  their confidence bands come from the analysis and stay linked to it. Paired and
  repeated-measures analyses offer **Graph with connecting lines**: bars with a line per
  subject ([Connecting lines and paired graphs](connecting-lines.md)).

A column graph shows every data point by default, with the error bar you choose (SD, SEM,
95% CI, range or IQR). A bar alone hides the distribution: two groups with the same mean
and SD can look identical as bars and completely different as points. If you want bars
without points, untick *Points*; it is your choice to make, not the default.

## The editor

Click anything in the graph (an axis, its title, a tick label, a data series, the legend,
a bracket) and it is outlined, and the inspector on the right shows its properties: for an
axis, its title, scale (linear, log10 or log2), limits, tick interval, minor ticks, tick
direction, number format, label angle and breaks; for a series, its colour, marker, size
and line style. Edit a value and the graph redraws.

Drag the legend or an annotation to move it. Double-click text to edit it in place.
**Graph → Add** places a significance bracket, a reference line or a text label. Every
change is undoable.

Text in titles accepts a small markup: `^` for superscript and `_` for subscript
(`cm^2`, `CO_2`), `\alpha`-style names or plain Unicode for Greek letters.

## Size: millimetres and points

Journals specify figures in physical units, so Dioptra does too. Figure sizes are in
millimetres (or inches); font sizes, line widths and marker sizes in points, where 1 pt is
1/72 inch. The size menu has presets for a single journal column (85 × 70 mm), a double
column (174 × 100 mm) and a slide (120 × 90 mm); any other size is typed in. The default figure is 85 × 70 mm with 8 pt tick
labels and 9 pt axis titles, which is legible when printed at that size.

Why not pixels? Because a pixel has no size until you print it. A figure designed at
"800 pixels" will have 4 pt text at one resolution and 12 pt at another. A figure designed
at 85 mm with 8 pt text has 8 pt text in the journal.

## Themes

| Theme | For |
|---|---|
| **Instrument** (default) | Papers and posters. White, thin dark axes on two sides, ticks outside, no gridlines, Inter. |
| **Night** | Slides with a dark background. The same geometry, light lines and text. |
| **Journal** | Black-and-white print. Series differ by marker shape, fill and dash rather than colour; slightly smaller type. |

Font size, axis line width and marker size can be overridden for one graph without
leaving the theme.

## Palettes

The default palette is **Okabe–Ito**: eight colours chosen so that people with the common
forms of colour blindness can still tell them apart. About one man in twelve has some
colour-vision deficiency, so a figure that relies on red against green loses part of its
audience. The other palettes are Paul Tol's *bright*, *muted* and *light* sets, *viridis*
and *cividis* for ordered data, *Tol sunset* for diverging data such as fold changes, and
a grey ramp. Any series can be given its own colour.

## Export

**Export** (Ctrl-Alt-E) offers three formats.

- **PDF**, with the fonts embedded as subsets. Opens identically everywhere and goes
  straight into a journal submission system or Illustrator, without missing-font
  substitutions.
- **SVG**, with real text (not outlines), grouped and named layers, and no clipping masks
  or invisible objects, so it edits cleanly in Illustrator or Inkscape.
- **PNG** at any resolution. The pixel size is the physical size times the resolution:
  85 mm at 300 dpi is 85 / 25.4 × 300 = 1003.9, so 1004 pixels. The file records its
  resolution, so it imports at the right size into Word or PowerPoint.

The dialog states the exact file it will write before you write it. *Copy graph as SVG*
puts the SVG on the clipboard.

Because the fonts are bundled and the engine does its own text measurement, the same
graph produces the same bytes on Linux, Windows and macOS.

## Layouts

A **layout** places several graphs on one page with panel letters (A, B, C…), for a
multi-panel figure. **Graph → New layout**. Each panel stays a live graph; edit it and the
layout follows. Export a layout exactly as a single graph.

![An estimation plot: both groups' points, and the mean difference with its bootstrap 95% confidence interval on a floating axis](../images/figure-estimation.png)
