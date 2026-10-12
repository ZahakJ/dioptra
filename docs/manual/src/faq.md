# FAQ

## Does it run on Linux?

Yes, as a first-class platform: it is developed on Linux. Each release has an AppImage (any
distribution), a `.deb` (Debian, Ubuntu, Mint) and an `.rpm` (Fedora, RHEL, openSUSE). See
[Getting started](getting-started.md#linux).

## What does it cost?

Nothing. Dioptra is free and open source under the MIT licence. There is no account, no
licence server, no trial period and no tier. Every analysis, effect size and export format
is in the one version there is.

## Can I use it at work, in a company, or in a clinical setting?

The MIT licence allows any use, commercial included, without asking. It also comes with no
warranty, as all open-source licences do. For regulated work, validate it as you would any
software: the [correctness record](correctness.md) and the test suite are there to make
that easier.

## Can I import my Excel files?

Yes. **File → Import** reads `.xlsx`, `.xls` and `.ods` directly, with no Office
installation, the same way on every system. It reads the stored values at full precision,
not the rounded values the sheet displays. Dates become ISO dates. You can also paste
straight from Excel; then the clipboard carries what is displayed, so show enough decimals
first.

## Can I open my Prism files?

Prism's XML format, `.pzfx`, can be imported: the data tables come across. Prism stores its
analyses and graphs in a form that is not documented, so they are not converted; re-run the
analyses in Dioptra. The binary `.pzf` and the newer `.prism` formats are not read yet. If
you have only those, export the data tables from Prism as text or `.pzfx`. Dioptra does not
write Prism formats; it exports CSV, TSV and XLSX for anyone who needs the data elsewhere.

## Where are my data? Does anything leave my computer?

Your data live in the `.dioptra` files you save, wherever you save them, and nowhere else.
Dioptra makes no network connections: no telemetry, no analytics, no update check, no
cloud. It works with the network cable unplugged.

## How do I know an old analysis will give the same result next year?

The project file stores the data, every formula as text, every analysis as a
specification, every graph as a specification, and the version of Dioptra that wrote it.
Opening it recomputes everything from the data. The file format is versioned and newer
versions read older files. And because the file is documented JSON, the data and the
recipe can be read without Dioptra at all, by `unzip` and a text editor. See
[File format](file-format.md).

## Why is Welch's t test the default instead of Student's?

Student's t test assumes the two groups have the same variance. When they do, Welch's test
gives nearly the same answer. When they do not, especially with unequal group sizes,
Student's test can give a p value that is far too small or far too large. Welch costs almost
nothing when the assumption holds and protects you when it does not. Student's test is one
option away. The longer argument is in [Analyses](analyses/overview.md#the-defaults-and-why).

## Can I run a one-tailed test?

Yes; every test with a direction has the option. It is not the default because a one-tailed
test is valid only when the direction was decided before the data were collected and an
effect in the other direction would be treated as no effect. When you choose it, the
methods sentence says so, so a reader knows.

## Why are there no asterisks on my graphs?

Because the default p-value style is APA: brackets show `p = .012`, not `*`. Asterisks
reduce a continuous measure of evidence to a grade, and readers stop at the grade. If your
journal or lab needs them, set the project's p-value style to *GP* (inspector, *Project*,
*P values*); every sheet and bracket switches at once, and switching back costs nothing.

## How large a data set can it handle?

A table of a million cells pastes in about 0.13 seconds and scrolls without lag, because
the grid fetches only the cells on screen. A column formula over a million rows computes in
under a tenth of a second. Graphs with more than 20,000 points are thinned for drawing (one
point per 0.75 pt cell, invisible at print size) but never for analysis. The measurements are
in [Performance](performance.md).

## Are the statistics right?

They are checked against the strongest references available: the NIST Statistical
Reference Datasets, whose answers are certified to 15 digits, and the established
implementations in SciPy, statsmodels, lifelines and scikit-learn. All 58 NIST data sets
pass, and 8,327 comparisons in 300 oracle cases agree within their stated tolerances. The
whole record, including the places where a reference library turned out to be the less
accurate one, is in [Correctness](correctness.md). If you find a disagreement, please open
a *numerical discrepancy* issue with the data and the reference value.

## What is missing compared with Prism?

Mixed-effects models, three-way ANOVA, two-way ANOVA with repeated measures on both
factors, multiple logistic and Cox regression, and a few smaller analyses. They are listed,
with the reason each waits, in the [roadmap](roadmap.md).

## How do I cite Dioptra?

The repository has a `CITATION.cff` file; GitHub's *Cite this repository* button turns it
into APA or BibTeX. Cite the version you used (it is in every methods sentence), for
example:

> ZahakJ. Dioptra: data, statistics and publication graphs without the spreadsheet
> (version 0.2.0) [Computer software]. 2026. https://github.com/ZahakJ/dioptra

## How can I help?

Report bugs and numerical discrepancies as issues, with a small file that shows the
problem. Contributions are welcome; read `CONTRIBUTING.md` in the repository first, because
numerical code has rules: a named reference and a test against a known value for every
change.
