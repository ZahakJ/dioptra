# The complaint map

Before any code was written, the project collected what working scientists dislike about
GraphPad Prism and about the spreadsheet step most people take before it. The sources were
GraphPad's own documentation and knowledge base (which documents many limits candidly),
published papers on spreadsheet errors and data presentation, and review sites. Each
complaint got a number, P1 to P29, and each design decision in Dioptra traces back to one
or more of them. The research, with every source, is `research/02-complaints.md` in the
repository.

This is not a list of Prism's failures. Prism is well liked: around 4.6 to 4.7 out of 5 on
the review sites. People complain about the edges of a tool they mostly value. Ten things
they value (K1 to K10) are listed at the end, and Dioptra keeps all of them.

## The story in five parts

**Cost and platform (P1–P4).** Prism is a subscription, priced per user per year, and some
of the features that make statistics honest (effect sizes for ANOVA, in-table formulas)
moved to higher tiers in Prism 11. There is no Linux version, and the Windows and Mac
versions differ in what they can import and export. Dioptra is MIT-licensed, has no
account and no licence server, builds for Linux, Windows and macOS from one codebase, and
renders the same bytes on all three.

**The spreadsheet step (P5–P13).** Most data reach Prism through Excel, and Excel changes
data on the way: gene symbols become dates (in 2016 about a fifth of papers with Excel gene
lists had such errors), pasted values are the rounded display rather than the stored
number, negative numbers formatted in parentheses arrive positive. Prism's own contingency
and survival tables drop decimal points on paste. Reshaping data to fit a table format is
manual, and formulas, the main reason Excel stays in the loop, arrived in Prism late and
only in one table kind. Dioptra's answer is to make the spreadsheet step unnecessary:
paste raw text, keep text as text, read spreadsheet files directly at full precision,
reshape with stored transforms, and compute with formulas in every table.

**Statistics (P14–P19).** Prism documents assumptions in help pages but does not check
them on the results; asterisks are the default summary; methods are under-reported in
the literature; estimation graphics came late; nested data are easy to analyse as if every
cell were independent. Dioptra puts the checks on the results sheet, leads with the
estimate and its interval, writes a methods sentence, draws estimation plots and
SuperPlots, and analyses nested tables on the experimental unit.

**Records and files (P20–P24).** Prism files are hard to inspect, change format between
versions, and do not diff. Dioptra's file is documented JSON in a zip, versioned, diffable
after unzip, and carries the full command history.

**Figures (P25–P29).** Getting a Prism figure into a journal often means Illustrator:
releasing clipping masks, fixing fonts. Colour-blind-safe palettes came late; bar charts
remain the default; large tables are slow. Dioptra exports SVG with real text and PDF with
embedded fonts, sizes figures in millimetres, ships colour-blind-safe palettes by default,
shows points on every column graph, and handles a million cells interactively.

## The full map

| # | Complaint | Dioptra's answer |
|---|---|---|
| P1 | Price and subscription licensing | MIT licence, free binaries for every platform, no account, no expiry |
| P2 | Effect sizes and formulas behind higher tiers | Effect sizes with confidence intervals in every results sheet; formulas in every table |
| P3 | No Linux | AppImage, deb and rpm on every release; developed on Linux |
| P4 | Windows and Mac versions differ | One Rust engine and one UI; the same SVG, PDF and PNG bytes on every system (bundled fonts) |
| P5 | Excel round trips corrupt data (gene names to dates) | No Excel step: paste raw text; text stays text, never coerced; import reads stored cell values |
| P6 | Paste copies displayed, rounded values | Full precision stored; display decimals are a column property; xlsx import reads stored values |
| P7 | Contingency and survival tables rewrite "1.23" as "123" | Tables never rewrite input; the analysis validates and says what it expected |
| P8 | Direct Excel import fragile and Windows-only | Built-in reader for xlsx, xls and ods, identical on every system, no Office needed |
| P9 | Choosing the table format is hard | Each kind explained in one sentence with a sketch; the paste dialog suggests a kind; Multiple variables is the safe default |
| P10 | Replicates laid out differently per table | Transforms convert between layouts (stack, pivot, average replicates) with the recipe stored |
| P11 | Long (tidy) data must be reshaped by hand | Multiple variables tables are first-class; pivot long to wide is one derived table |
| P12 | No formulas where expected | Column and cell formulas in a formula bar, dependency tracked, recomputed on edit |
| P13 | Missing values break designs | Missing is a value; each analysis states how it handled missing data; mixed models are planned |
| P14 | P-hacking by dialog | Every analysis is an object in the project and every run is in the history, including deleted ones |
| P15 | Assumptions documented, not checked | Assumption checks run with the analysis, shown first, with the alternative named |
| P16 | Asterisks and "significant" as the default | APA-style exact p values by default; the summary leads with the estimate and CI; asterisks optional |
| P17 | Methods under-reported | Every results sheet has a methods sentence (test, variant, sidedness, correction, version) |
| P18 | Estimation came late and only for t tests | Estimation plots with bootstrap CIs of the difference |
| P19 | Hierarchical data analysed as independent | Nested tables analyse unit means and say so; the default graph is a SuperPlot |
| P20 | No scripted record of the analysis | Formulas, specifications and the history in JSON; recompute reproduces every number; a command-line runner is planned |
| P21 | Opaque file format | `.dioptra` is documented JSON in a zip ([File format](file-format.md)) |
| P22 | Version lock-in | Versioned format; newer readers open every older file; CSV and XLSX export always |
| P23 | No version control | Stable-ordered, pretty-printed JSON that diffs after unzip; line-oriented history |
| P24 | Collaboration is file passing | An open file makes git-based collaboration possible; real-time sync is not planned |
| P25 | Export needs Illustrator workarounds | SVG with real text and no clipping masks; PDF with embedded subset fonts |
| P26 | Figures finished in other software | Sizes in mm and points, journal presets, multi-panel layouts with panel letters |
| P27 | Limited colour and theme control | Designed themes, colour-blind-safe palettes by default, per-series colours |
| P28 | Modern distribution plots late; bars the default | Violin, raincloud, estimation, before–after and SuperPlot; column graphs show points by default |
| P29 | Slow or unstable with large tables | Columnar storage, viewport fetches, a 10⁶-cell paste in about 0.13 s ([Performance](performance.md)) |

## What is kept from Prism

| # | What people like | How Dioptra keeps it |
|---|---|---|
| K1 | Guided analysis by table kind | The Analyze list shows what fits the table and says why the rest does not |
| K2 | Results linked to data | Edit a value and every dependent analysis and graph recomputes |
| K3 | Graphs that follow the data | Graphs are specifications over tables and analyses, not pictures |
| K4 | Publication-quality defaults | Designed themes, typographic sizes in points, bundled fonts |
| K5 | The analysis checklist | *Before you report this* on every results sheet |
| K6 | Documentation that teaches | This manual explains each default and each test |
| K7 | Nonlinear regression done well | The equation library, constraints, shared parameters, model comparison, ROUT, profile CIs |
| K8 | Raw and summary data both accepted | Mean/SD/N and other summary formats in XY and grouped tables |
| K9 | Excluded values stay visible | Struck through, kept, counted |
| K10 | Works offline, no code | A desktop program; nothing leaves your machine |
