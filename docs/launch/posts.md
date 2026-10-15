# Launch posts

Drafts. Do not post until the week in [README.md](README.md). Replace `0.x.y` with the
current version and check every number against the README on the day.

Rules for every thread:

- Post from your own account, with your own name or usual handle. Say you built it.
- One post per community. No cross-posting the same day, no vote requests, no alt accounts.
- Reply to every comment for the first 6 hours. Thank critics; fix what they find; post
  the release link when you do.
- Do not mention Dioptra Lab or prices in the free launch. If asked: "The app is free and
  MIT and stays so. I may sell a paid add-on later for batch work and regulated labs."

## r/labrats

Read the sidebar first. r/labrats allows "I made this" posts that ask for feedback; it
removes ads. Use the *Discussion* flair if there is no *Software* flair. Post Tuesday to
Thursday, 14:00–16:00 UTC (morning in the US, afternoon in Europe).

**Title:** I built a free, open-source alternative to Prism (opens .pzfx files, runs on
Linux). Looking for feedback from people who make figures every week.

**Body:**

> I'm a [your role] and I got tired of the spreadsheet → Prism → Illustrator loop, and of
> our department's Prism licence getting more expensive every year. So I wrote Dioptra.
>
> What it is:
>
> - A desktop app for Windows, macOS and Linux. Free, MIT licence, no account, no
>   telemetry, works offline.
> - Paste from Excel without it turning SEPT2 into a date. Every cell remembers where it
>   came from.
> - 20 analyses (t tests, ANOVA with post hoc tests, nonlinear dose-response fits,
>   survival, ROC, Bland–Altman, PCA…). Assumption checks come first, then effect sizes
>   with CIs, then an APA sentence you can paste.
> - 23 graph families, including SuperPlots, estimation plots, raincloud and paired
>   lines. Sized in mm; PDF with embedded fonts and SVG with real text, so it goes straight
>   into a figure.
> - Opens Prism `.pzfx` files, so old projects are not stuck when a licence ends.
>
> Numbers are checked against NIST reference data and against SciPy/statsmodels; the full
> record is public.
>
> What I'd like to know: what did you make in Prism last week that you could not make in
> this? What breaks? Bug reports with a file are gold.
>
> Download: https://github.com/ZahakJ/dioptra/releases/latest
> Manual: https://zahakj.github.io/dioptra/manual/
>
> Windows and macOS builds are not signed with a paid certificate yet, so you will get a
> "More info → Run anyway" or "Open Anyway" prompt once. The manual says what to click.

Image: the three-panel screenshot (grid, results, graph editor), light mode.

## r/bioinformatics

Shorter, more technical. Check that the subreddit allows tool announcements that week
(some weeks are restricted to the pinned thread; post there if so).

**Title:** Dioptra: free MIT desktop app for lab stats and figures (Rust engine, CLI, reads
.pzfx)

**Body:**

> I built Dioptra for wet-lab colleagues who use Prism and Excel, but parts of it may be
> useful here:
>
> - Rust engine; the same code runs the desktop app, a CLI and a WebAssembly build.
> - `dioptra analyze data.csv --kind column --test one_way_anova` for scripted pipelines;
>   `render` and `export` produce the same SVG/PDF bytes on every platform.
> - Reads Prism `.pzfx`; project files are a zip of documented JSON that diffs under git.
> - Checked against NIST StRD (58/58) and 8,327 comparisons with SciPy, statsmodels,
>   lifelines and scikit-learn.
>
> Linux AppImage/deb/rpm, Windows, macOS. Feedback and numerical bug reports welcome.
> https://github.com/ZahakJ/dioptra

## Show HN

Post Tuesday or Wednesday, 14:00–15:00 UTC. URL field: the GitHub repo. Leave the text
field empty; post the first comment yourself within a minute.

**Title (≤80):** Show HN: Dioptra – a free, open-source GraphPad Prism alternative in Rust

**First comment:**

> Hi HN, I'm the author. Dioptra replaces the spreadsheet + GraphPad Prism + Illustrator
> chain that most biology labs use for small-data statistics and figures. It's MIT, runs on
> Linux, Windows and macOS, and has no telemetry.
>
> Some technical notes:
>
> - Everything that computes is Rust: `dioptra-stats` (special functions, distributions,
>   tests, Levenberg–Marquardt with an equation library, survival, PCA),
>   `dioptra-core` (tables with per-cell provenance, a formula language with a Pratt parser,
>   a dependency graph with pull-based recomputation, undo by stored inverses) and
>   `dioptra-plot` (a device-independent scene measured with the bundled font's own metrics,
>   written to SVG, to PNG through resvg and to PDF through svg2pdf). Because layout never
>   asks the OS about fonts, exports are byte-identical on every platform.
> - The UI is React in Tauri 2 and never does arithmetic. It sends JSON commands to one
>   `dispatch` function. The same engine compiles to wasm, so the end-to-end tests drive
>   the real engine in a headless browser, and the CLI gives the same numbers.
> - Correctness: all 58 NIST StRD data sets used pass, and 8,327 comparisons against SciPy,
>   statsmodels, lifelines and scikit-learn agree within tolerance. Every function cites its
>   reference in METHODS.md. The record is in the manual's correctness chapter.
> - The project file is a zip of documented JSON with the data, formulas, analysis specs
>   and the command history; results are recomputed on open.
> - It reads Prism's `.pzfx` (XML). Prism encrypts the analysis and graph parts of its own
>   files, so only data come across.
>
> The Windows and macOS builds aren't signed with paid certificates yet, so expect one OS
> prompt. I'd especially like bug reports where a number disagrees with R or Prism.

Answer "why not R/Python": "Most people who use Prism will not write code. Dioptra gives
them a GUI whose numbers match R and SciPy, and a CLI for those who do."

## Bluesky thread

Post from your account, tag #AcademicChatter #labrats #SciSky on the first post only.
Each post ≤300 characters (count them in the composer).

1. I built a free, open-source replacement for GraphPad Prism + Excel. It's called
   Dioptra. Paste data, get the assumptions checked, export a journal-ready figure.
   Windows, macOS and Linux. MIT licence, no account, no tracking. (1/5)
   #AcademicChatter #labrats #SciSky
   *Image: grid screenshot. Alt: "Dioptra's data grid: four dose groups in columns, a
   selected cell, and a side panel saying the value was pasted from the clipboard at
   10:42."*

2. Every analysis opens with its assumption checks: normality, equal variances, and what
   to use if they fail. Then the effect size with its CI, then a sentence in APA style you
   can paste into the paper.
   *Image: results sheet. Alt: "A one-way ANOVA results sheet: two assumption checks
   passed, a summary sentence with eta squared and its 95% confidence interval, and a table
   of Tukey comparisons."*

3. Graphs are sized in millimetres. PDF embeds the fonts, SVG keeps real text, so the
   figure goes into the paper without a trip through Illustrator. SuperPlots, estimation
   plots, raincloud, paired lines, Kaplan–Meier.
   *Image: estimation plot. Alt: "An estimation plot: two groups' data points on the left
   and the mean difference with its bootstrap 95% confidence interval on the right."*

4. It opens Prism .pzfx files, so your old projects don't die with the licence. The
   numbers are checked against NIST reference data and 8,327 comparisons with SciPy and
   statsmodels.

5. Download: github.com/ZahakJ/dioptra/releases/latest
   Manual: zahakj.github.io/dioptra/manual
   I'd love to hear what you make in Prism that Dioptra can't do yet. Replies and bug
   reports welcome.

## Founding member launch (week 8)

Only after the free launch has run for 4 weeks and Help → Dioptra Lab is live. Post once on
r/labrats as a reply in your own launch thread (not a new post), once on Bluesky, and in
the GitHub release notes. Email the same text to people who asked to be told. Check the
pricing page is live first.

> Dioptra stays free and MIT. For labs that run the same analysis on many plates, there is
> now **Dioptra Lab**: batch templates and a watch folder, plate-reader import into a
> template, journal presets with a figure check, and a methods/report export.
>
> The first 200 founding members pay $49 a year (academic list price $79), and the price
> holds while you renew. The key works offline; nothing phones home.
>
> Core facilities and departments: a free 60-day site pilot, reply or email me.
>
> https://zahakj.github.io/dioptra/pricing.html
