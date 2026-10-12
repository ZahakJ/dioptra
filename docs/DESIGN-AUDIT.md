# Design audit (v0.2.0)

This is the check of the desktop interface against DESIGN.md §12: the contrast of every token
pair that carries text or meaning, the control set and where each control is used, the menu
and chrome decision, and the screen captures the audit was made from. Regenerate the table with
`node scripts/contrast.mjs --markdown`; the script exits with an error when a pair falls below
WCAG AA, and when the two copies of the dark theme in `tokens.css` disagree.

## 1. Contrast

Body text needs 4.5:1, interface marks (glyphs, row numbers, placeholders, rulers, focus rings,
assumption marks) need 3:1. Both themes, measured from `apps/dioptra/src/styles/tokens.css`.

| Foreground | Background | Class | Light | Dark |
|---|---|---|---|---|
| `--fg` | `--bg-app` | body (4.5:1) | 14.33 | 13.65 |
| `--fg` | `--bg-panel` | body (4.5:1) | 15.36 | 12.55 |
| `--fg` | `--bg-sheet` | body (4.5:1) | 16.02 | 11.84 |
| `--fg` | `--bg-raised` | body (4.5:1) | 16.02 | 10.93 |
| `--fg` | `--bg-inset` | body (4.5:1) | 13.47 | 14.16 |
| `--fg` | `--accent-soft` | body (4.5:1) | 13.34 | 9.00 |
| `--fg-muted` | `--bg-app` | body (4.5:1) | 5.36 | 6.81 |
| `--fg-muted` | `--bg-panel` | body (4.5:1) | 5.75 | 6.27 |
| `--fg-muted` | `--bg-sheet` | body (4.5:1) | 6.00 | 5.91 |
| `--fg-muted` | `--bg-raised` | body (4.5:1) | 6.00 | 5.46 |
| `--fg-muted` | `--bg-inset` | body (4.5:1) | 5.04 | 7.07 |
| `--fg` | `--bg-sheet-alt` | body (4.5:1) | 15.22 | 11.39 |
| `--fg` | `--bg-col-tint` | body (4.5:1) | 14.58 | 12.28 |
| `--fg` | `--bg-head` | body (4.5:1) | 15.36 | 12.55 |
| `--fg-muted` | `--bg-head` | body (4.5:1) | 5.75 | 6.27 |
| `--fg-muted` | `--bg-col-tint` | body (4.5:1) | 5.45 | 6.13 |
| `--fg-faint` | `--bg-head` | ui (3:1) | 3.57 | 3.56 |
| `--fg-faint` | `--bg-sheet-alt` | ui (3:1) | 3.54 | 3.23 |
| `--fg-faint` | `--bg-app` | ui (3:1) | 3.33 | 3.87 |
| `--fg-faint` | `--bg-panel` | ui (3:1) | 3.57 | 3.56 |
| `--fg-faint` | `--bg-sheet` | ui (3:1) | 3.72 | 3.36 |
| `--fg-faint` | `--bg-inset` | ui (3:1) | 3.13 | 4.02 |
| `--accent` | `--bg-panel` | body (4.5:1) | 4.66 | 6.66 |
| `--accent` | `--bg-sheet` | body (4.5:1) | 4.86 | 6.29 |
| `--accent` | `--bg-app` | ui (3:1) | 4.35 | 7.25 |
| `--accent-fg` | `--accent` | body (4.5:1) | 4.86 | 7.22 |
| `--accent-fg` | `--accent-hover` | body (4.5:1) | 6.20 | 8.83 |
| `--ok` | `--bg-panel` | ui (3:1) | 4.83 | 7.54 |
| `--caution` | `--bg-panel` | ui (3:1) | 4.29 | 7.59 |
| `--fail` | `--bg-panel` | ui (3:1) | 5.26 | 5.64 |
| `--ok` | `--bg-sheet` | ui (3:1) | 5.04 | 7.11 |
| `--caution` | `--bg-sheet` | ui (3:1) | 4.48 | 7.16 |
| `--fail` | `--bg-sheet` | ui (3:1) | 5.48 | 5.32 |
| `--focus` | `--bg-panel` | ui (3:1) | 4.66 | 6.66 |
| `--tooltip-fg` | `--tooltip-bg` | body (4.5:1) | 12.10 | 7.92 |

Every pair meets WCAG AA. `--fg-disabled` is exempt (WCAG 1.4.3 excludes inactive controls).
Selection washes (`--sel-fill`, `--accent-wash`) are not text surfaces: the grid draws the
selection with `mix-blend-mode` (multiply on paper, screen on graphite), so cell text inside a
selection keeps its own contrast.

## 2. Controls

No native form control is visible anywhere (the e2e test `(h)` asserts there is no `select`,
native checkbox, radio, number input or `title` attribute in the document). The set lives in
`apps/dioptra/src/ui/`.

| Control | What it is | Where it is used |
|---|---|---|
| `Button` | primary, secondary, quiet, danger; icon-only buttons carry a tooltip label | every dialog footer, table tools (Analyze, Graph), graph toolbar (Add, Export, More), zoom −/+, inspector actions, notices, project rail |
| `Select` | popover list, keyboard navigable, typeahead, swatches and secondary text | graph toolbar (family, size, theme, palette, error bars), graph inspector, analysis options, new-table dialog, transform dialog, column properties |
| `Segmented` | small enums as one strip, arrow keys move the choice | formula bar (Cell/Column), theme control in the top bar, zoom presets (Fit · 50 · 100 · 200), paper (white / theme / transparent), layout columns, axis ticks, export format, import options, analysis options, transforms |
| `Toggle` | drawn switch | graph inspector switches (an axis's Hidden), export dialog, analysis options, transforms |
| `Checkbox` | drawn check | graph toolbar (Points), analysis options, layout dialog, transforms |
| `NumberField` | stepper with unit suffix; arrows ±1, Shift ±10; mm/pt/dpi aware | graph inspector (sizes, limits, ticks), export dialog, column decimals, analysis options, new-table dialog, transforms |
| `TextInput` / `TextArea` | text field with the app's focus ring; mono where the content is code | prompts, inspector fields, graph titles, import (paste box), transforms |
| `ColorSwatch` | palette grid plus hex field | data set colours in the graph inspector |
| `Menu` / `ContextMenu` | command list with headings, checks, mono shortcuts and submenus | top bar Menu (whole command tree), grid cell and header context menus, project rail context menu, graph Add and More menus, Copy sheet |
| `DialogFrame` | modal with scrim, focus trap, header and footer | confirm and prompt dialogs, keyboard shortcuts, every feature dialog |
| `RadioList` | drawn radio rows | prompts with choices, table export format |
| `Field` | label, hint and error around a control | new table, import, transform and layout dialogs |
| `Popover` | positioned floating surface | the base of `Select`, `Menu` and `ColorSwatch` |
| Tooltip (`data-tip`) | one host, 400 ms delay, also on keyboard focus, optional shortcut | every icon-only button, column headers, grid cells (provenance of the active cell), graph targets, results numbers (full precision) |
| `Tabs` | labels with the accent underline | available in the set; the work-area tabs are drawn by `workArea.tsx` with the same marker |

Scrollbars are themed (`scrollbar-width: thin` plus `::-webkit-scrollbar`), and every focusable
element has the 2 px accent focus ring at a 2 px offset.

## 3. Menus and chrome (DESIGN.md §12.5)

- On macOS the command tree is the system menu bar (Tauri's native menu API); the in-window
  Menu button is hidden there.
- On Linux and Windows there is no in-window menu bar. A slim top bar holds the app mark, the
  project name (click to rename), the command field that opens the palette, the theme control
  and a compact Menu button that opens the same command tree as a designed popover.
- Reasons: an in-window menu bar was the strongest "old desktop app" tell; native GTK and
  WebView2 menus do not follow the app's theme; the palette (Ctrl/⌘-K) is the primary surface.
- The window follows the OS theme by default, with an explicit light/dark override in the top
  bar, View and the palette; the title bar follows through `window.setTheme`. Window size,
  panel widths, collapsed inspector sections, grid density and the graph paper per graph are
  remembered across launches.

## 4. What round 2 changed (DESIGN.md §12.6–12.8)

- **Grid.** A bounded sheet on the desk: rows end 12 below the data (never fewer than 20), with
  "+ 20 rows" under the sheet and "+ Data set" beside it; nothing lettered or numbered beyond.
  Data set headers carry a 12 px kind glyph and a 10 px swatch in the data set's Okabe–Ito
  colour (the colour the graph uses). The X column and the row-title column are tinted.
  Selection: 60% accent-soft fill, 1.5 px accent edge, square corner handles; the active cell
  a 2 px ring. Alternate rows differ by 2% luminance. Density: comfortable (28 px) or compact
  (22 px), in View and the palette, remembered. The formula bar: the reference in a mono well,
  Cell/Column, an ƒ mark and the expression in a mono well that grows to two lines while it is
  edited. Provenance times read "today 23:18" or "11 Oct 2026, 09:12" in the inspector and the
  active cell's tooltip.
- **Results.** A 72-character reading column (centred alone, left beside the table): title
  22/600, methods 12 px, section headings 15/600, hairline-ruled tables with numbers in mono
  tabular figures, significant p values at weight 500 and never coloured, the assumption card
  ruled in the colour of its worst check, Lucide marks per check, alternatives as a muted line
  after an arrow, summary sentences selectable whole, a quiet checklist, and Options / Copy
  sheet / Make graph as a quiet toolbar aligned to the column.
- **Graph editor.** The desk is `--bg-app`; the figure sits on paper (white by default, theme,
  or a transparency checkerboard; per graph) with a hairline and elevation; millimetre rulers
  (ticks every 5 mm, labels every 10 mm) track the zoom; zoom is Fit · 50 · 100 · 200 plus −/+
  and a mono readout; the size reads in mm in the status strip. Hover draws a 1 px accent halo
  2 px outside a target, selection a 1.5 px frame with handles; nothing is filled. The toolbar
  never wraps: below 980 px of width its button labels become icons, below 760 px theme,
  palette, error bars, points and paper move into a More menu.
- **First launch.** "A sheet for your data. Paste, test, export." over three cards (Paste data,
  Open a sample project, Read the manual) and the keys that matter. The sample is
  `apps/dioptra/public/samples/cytokine-study.dioptra`, built by `scripts/make-sample.mjs` with
  the engine itself and opened from bytes on both backends.
- **Icon.** A sighting bar with its two vanes over a graduated disc, verdigris `#5FB3AD` on
  graphite `#1C2024`, 12% corner radius (`apps/dioptra/public/icon.svg`); a simplified mark
  (`icon-small.svg`: four graduations, heavier strokes) serves 16–44 px, the top bar and the
  favicon. The platform set in `src-tauri/icons` is generated by `pnpm tauri icon`, with the
  16/24/32 px frames of `icon.ico` and the 30, 32 and 44 px PNGs taken from the small mark.

## 5. Captures

Made with `.scratch/design/capture.mjs` against the production build (`vite preview`), with the
real WebAssembly engine and the Cytokine study data. Every capture was looked at in both themes
before v0.2.0 was tagged.

- Before the design round: `.scratch/design/before/`.
- After round 1: `.scratch/design/after-round1/`.
- v0.2.0: `.scratch/design/after/` — `empty`, `grid`, `results`, `graph`, `palette`,
  `dialog-analyze`, `export`, `survival`, `dose-response`, `layout`, `grid-compact` and
  `paste-import`, each as `<name>-<light|dark>-<1440|1024>.png`; the landing page as
  `site-<light|dark>-<1440|390>.png` and `site-captures-<theme>-1440.png`.
- The README, manual and landing set: `.scratch/final3/` (1440 × 900, both themes) and the
  2× set `.scratch/final3-2x/` the banner crops come from.

## 6. The GitHub presence

- `docs/media/banner.png` (1640 × 640, drawn for 820 × 320 at 2×) heads the README and is the
  landing page's Open Graph image (`site/assets/banner.png`). Its source is
  `docs/media/banner.svg` with two real dark-mode crops of v0.2.0
  (`banner-crop-grid.png`, `banner-crop-graph.png`); `node scripts/make-banner.mjs` renders it.
- The README carries the release, licence, CI and platform badges and a strip of the grid,
  results sheet and graph editor in both themes (`docs/media/*-light.png`, `*-dark.png`).
- The landing page's hero sits on the same millimetre desk as the banner, and every capture on
  it follows the page's theme (system by default, or the toggle).
