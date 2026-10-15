# Journal figures

*Dioptra Lab.* SVG, PDF and PNG export at any size, multi-panel layouts and colour-blind-safe
palettes are free.

## Journal presets

**Journal** in the graph editor or layout toolbar. Pick a journal and a column width, then
**Apply to this graph** or **Apply to every figure** (every graph and layout in the project).
One undo takes the whole change back.

A preset sets the width, scales the height with it (up to the journal's maximum), sets the
type size and line width, and switches the slides theme to the light one. On a layout it
sets the page width, one type size and line width for every panel, and the journal's panel
letters.

Type: tick labels at 7 pt, raised where the smallest text the engine draws (85% of the tick
labels: point labels, numbers at risk) would fall below the journal minimum, and lowered to
keep axis titles (+1 pt) under the maximum. Axis lines at twice the journal's minimum line
width, at least 0.5 pt; data lines and outlines follow.

| Journal | Single | 1.5 | Double | Max height | Text | Lines | Raster (line / photo / combination) | Files | Letters |
|---|---|---|---|---|---|---|---|---|---|
| Nature | 89 mm | 120 mm | 183 mm | 170 mm | 5–7 pt, Helvetica or Arial | 0.25 pt | 1000 / 300 / 450 dpi | PDF, EPS, AI | a b c, 8 pt |
| Cell Press | 85 mm | 114 mm | 174 mm | 225 mm | 6–8 pt, Arial | 0.25 pt | 1000 / 300 / 500 dpi | TIFF (LZW), PDF, EPS, AI | A B C, 8 pt |
| Science | 57 mm | 121 mm | 184 mm | 230 mm | ≥ 6 pt, Helvetica | 0.28 pt | 1200 / 300 / 600 dpi | PDF, EPS, AI, TIFF | A B C, 9 pt |
| PLOS | 132 mm | — | 190.5 mm | 222.3 mm | 8–12 pt, Arial | 0.25 pt | 300 dpi | TIFF (LZW), EPS | A B C, 10 pt |
| eLife | 100 mm | — | 200 mm | — | 6–12 pt, sans-serif | 0.25 pt | 300 dpi | TIFF, PDF, EPS, AI, JPG | A B C, 10 pt |
| ACS | 84.7 mm | — | 177.8 mm | 232.8 mm | ≥ 4.5 pt, Helvetica or Arial | 0.5 pt | 1200 / 600 / 600 dpi | TIFF, PDF, EPS | A B C, 8 pt |
| Elsevier | 90 mm | 140 mm | 190 mm | — | ≥ 6 pt, Arial or Helvetica | 0.1 pt | 1000 / 300 / 500 dpi | EPS, PDF, TIFF, JPEG | A B C, 8 pt |
| APS (Physical Review) | 86 mm | 130 mm | 178 mm | 237 mm | ≥ 8 pt | 0.5 pt | 600 dpi | EPS, PS, PNG, JPG | a b c, 8 pt |
| The Journal of Physiology | 85 mm | — | 170 mm | 230 mm | ≥ 8 pt, sans-serif | 0.25 pt | 600 dpi | PDF, EPS, TIFF | A B C, 10 pt |

The figure check never accepts a line thinner than 0.25 pt, whatever the journal.

Sources, read on 14 October 2026:

- Nature: <https://research-figure-guide.nature.com/figures/building-and-exporting-figure-panels/>,
  <https://www.nature.com/documents/nature-final-artwork.pdf>,
  <https://research-figure-guide.nature.com/figures/extended-data-formatting-guidelines/>
- Cell Press: <https://www.cell.com/figureguidelines>
- Science: <https://www.science.org/content/page/instructions-preparing-revised-manuscript>
- PLOS: <https://journals.plos.org/plosone/s/figures>
- eLife: <https://elife-rp.msubmit.net/html/elife-rp_author_instructions.html>
- ACS: <https://researcher-resources.acs.org/publish/author_guidelines?coden=jacsat>
- Elsevier: <https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-sizing>,
  <https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-types>
- APS: <https://journals.aps.org/authors/style-basics>
- The Journal of Physiology: <https://jp.msubmit.net/cgi-bin/main.plex?form_type=display_requirements>

Journals revise these pages; the author guidelines of the journal you submit to decide.

## Figure check

The Journal popover checks the figure against the chosen journal each time it changes. It
reads the drawing that is exported, so it checks what the journal receives.

| Rule | Flags |
|---|---|
| Text size | Text below the journal minimum, super- and subscripts below it, text above the maximum. A figure wider than the journal's full width is checked at the size the journal reduces it to. |
| Line width | Lines thinner than the journal minimum or 0.25 pt. Strokes in the background colour (gaps between slices or cells) are not lines. |
| Resolution | PNG and TIFF below the journal's resolution: line art for black-and-white figures, combination for colour. |
| Colour vision | Two data-set colours that differ for normal vision but not for a reader with deuteranopia, protanopia or tritanopia. |
| Panels | Panels of a layout whose tick labels or axis lines differ in size. |
| Size | A figure wider than the journal's full width or taller than its maximum; a width between its columns. |

Click an issue to select the element in the graph editor.

Colour vision is simulated with Machado, Oliveira and Fernandes (2009),
doi:10.1109/TVCG.2009.113, at full severity on linear RGB. Two colours count as distinct when
their CIEDE2000 difference (Sharma, Wu and Dalal 2005, doi:10.1002/col.20070) is at least 10.
Neighbouring Okabe–Ito colours pass in all three simulations; a red and a green of similar
lightness fail for deuteranopia.

## Layouts at final size

**Journal → Final size** in the layout view:

- **Letters**: A B C, a b c or none; **Letter size** in points (bold).
- **Page width**: the panels are resized, keeping their shape, so the columns and their
  letter gutters fill exactly this width. Type and lines are not scaled.
- **Text** and **Lines**: one tick-label size and one axis line width in every panel,
  whatever each graph says.

Empty fields keep each graph's own setting. A layout made this way draws the same in every
copy of Dioptra; changing the settings needs Dioptra Lab.

## TIFF

**Export… → TIFF**: 8-bit RGB with LZW compression, the resolution recorded in the file,
rendered from the same drawing as PNG on a white background. EPS is not offered; PDF and
SVG carry the same vectors.
