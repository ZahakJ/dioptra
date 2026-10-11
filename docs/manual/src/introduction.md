# Introduction

Dioptra is a desktop program for the part of science that happens between the instrument
and the paper. You paste the numbers your plate reader, cytometer or notebook produced.
You shape them: subtract a blank, convert units, normalise to a control. You test them.
You draw the figure the journal will print. One file holds all of it.

It does the job GraphPad Prism does, and the job of the spreadsheet most people open
before Prism. It runs on Linux, Windows and macOS, it is free, and the source is open
under the MIT licence.

The name comes from the dioptra of Hero of Alexandria, a sighting instrument for
measuring angles and levels. The aim is the same as a good instrument's: read the
quantity accurately, show the scale, and do not decorate.

![The grid holding a pasted 1,000 by 1,000 plate-reader table, with the inspector showing where the selected cell came from](images/grid-light.png)

## What it is built on

Four ideas run through every chapter of this manual.

**Raw data is never changed.** An analysis reads your table; it never writes to it. A
transform makes a new column or a new table and remembers how. Excluding an outlier
strikes the value through; it does not delete it. Every cell can tell you where it came
from: typed by hand, pasted at a certain time, imported from a file and column, or
computed by a formula.

**Every assumption is shown.** A t test assumes something about your data. So does an
ANOVA, a regression, a chi-square test. Dioptra checks what can be checked, shows the
check above the result, and names the alternative when a check fails. The defaults follow
current statistical practice (Welch's t test rather than Student's, two-sided tests,
Tukey–Kramer for all pairs, Holm rather than plain Bonferroni) and the results sheet says
which one ran.

**The file is the record.** A `.dioptra` file is a zip of readable JSON. It holds the data,
each formula as text, each analysis as a specification, each graph as a specification, and
the history of commands. Open it in a year and every number comes back the same way.

**The figure on screen is the figure you export.** The plot engine writes one SVG; the PDF
and PNG are made from it, with the fonts bundled, so a figure rendered on Linux is
byte-identical on a Mac. Sizes are in millimetres and points, because that is how journals
specify them.

## How this manual is organised

- [Getting started](getting-started.md) takes you from download to an exported figure in
  about ten minutes.
- [Data tables](data-tables.md) explains the eight table kinds and how to choose one.
- [Entering and shaping data](entering-data.md) covers paste, missing and excluded values,
  provenance and transforms. [Formulas](formulas.md) is the formula reference.
- [Analyses](analyses/overview.md) explains the guided flow, assumption checks and p-value
  styles, then documents each analysis.
- [Graphs](graphs/editor.md) explains the editor, themes, palettes and export, then each
  graph family.
- [File format](file-format.md), [Correctness](correctness.md) and
  [Performance](performance.md) are the record: what is stored, how the numbers were
  verified, and how fast it runs.
- [Architecture and internals](architecture.md) is for people who want to change the code.
- [The complaint map](complaint-map.md) lists what scientists dislike about the Prism
  workflow and what Dioptra does about each item.

The statistics are explained where they are used, briefly, with a reason for each default.
If a chapter tells you to do something, it should also tell you why.
