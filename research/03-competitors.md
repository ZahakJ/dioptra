# 03 — Alternatives to Prism, and what to borrow

A short survey of the tools people use instead of, or next to, Prism. For each: what it is, the strengths that matter for Dioptra (honest statistics in the UI, estimation graphics, plot grammar, interactive editing, export), and the one or two ideas to take. Prices and platform facts are as of 2026-10-11.

## Summary table

| Tool | Kind | Licence / platforms | Borrow |
|---|---|---|---|
| JASP | GUI statistics | AGPL; Win/Mac/Linux | live results, Bayesian twin of every test, APA tables |
| jamovi | GUI statistics on R | AGPL; Win/Mac/Linux, cloud | spreadsheet with computed variables, "syntax mode" R code per analysis |
| R ggplot2 + ggpubr / ggstatsplot | code | GPL; all | layered grammar; stats printed on the plot with effect sizes |
| Python seaborn + statannotations | code | BSD/MIT; all | statistical plot types as one call; explicit test and correction on brackets |
| DABEST / estimationstats | code + web | BSD/Apache; all | Gardner-Altman and Cumming estimation plots with bootstrap CIs |
| Raincloud plots | code | MIT; all | half-violin + box + jittered points as a default distribution view |
| OriginPro | GUI plotting + analysis | commercial; Windows | graph templates, batch analysis, recalculation mode |
| SigmaPlot | GUI plotting + statistics | commercial; Windows | breadth of 2D/3D graph types |
| Datawrapper | web charting | SaaS; browser | paste-to-chart in four steps, accessibility checks, responsive export |
| Observable Plot | code (JS) | ISC; browser | concise marks/scales/transforms API for the renderer |
| Veusz | GUI + script plotting | GPL; Win/Mac/Linux | every GUI action is a command; text document format; vector-first export |
| LabPlot | GUI plotting + analysis | GPL; Win/Mac/Linux | open, cross-platform Origin-style workflow; notebooks next to worksheets |
| SciDAVis | GUI plotting | GPLv2; Win/Mac/Linux | Origin-like tables with column formulas, Python scripting |
| Tableau | BI visual analysis | commercial; Win/Mac, cloud | drag fields onto shelves (VizQL); "Show Me" chart recommendation |

---

## Statistics with a GUI

### JASP
Free, open-source (AGPL) statistics program from the University of Amsterdam; frequentist and Bayesian versions of the same analyses (t tests, ANOVA, regression, contingency) side by side (https://jasp-stats.org/, https://en.wikipedia.org/wiki/JASP).
- **Live results**: changing an option re-renders the output immediately, with no "OK" button, which makes the consequence of each choice visible.
- **APA-formatted tables** that paste into a word processor (https://services.anu.edu.au/information-technology/software-systems/jasp).
- **Assumption checks are options in the same panel** as the test (equality of variances, normality, Q-Q plots), so they sit next to the result rather than in a separate help page.
- Borrow: live recomputation without a modal dialog; Bayes factors as an optional companion to every frequentist test; assumption checks inline.

### jamovi
Free, open-source spreadsheet-plus-statistics program built on R (https://www.jamovi.org/).
- **Spreadsheet with computed and transformed variables**, so data cleaning happens in the same tool as analysis.
- **Syntax mode** shows, for each analysis, the equivalent R call from the `jmv` package, which can be copied and re-run in R (https://jamovi.readthedocs.io/en/latest/usermanual/um_6_jamovi_and_R/). The `.omv` file is readable from R with `jmvReadWrite` (https://cran.r-project.org/package=jmvReadWrite).
- **Module library** adds analyses without a new release.
- Borrow: every GUI analysis has a code twin the user can see and export; the data editor and the analysis share one window.

## Code-first plotting

### R: ggplot2, ggpubr, ggstatsplot
- **ggplot2** implements Wickham's layered grammar of graphics: data, aesthetic mappings, geoms, stats, scales, coordinates, facets and themes compose independently (Wickham H. A layered grammar of graphics. J Comput Graph Stat 2010;19(1):3-28, https://doi.org/10.1198/jcgs.2009.07098; https://ggplot2.tidyverse.org/). Themes are data, so a journal style is one object.
- **ggpubr** wraps ggplot2 for publication figures and adds `stat_compare_means()` and `stat_pvalue_manual()` for brackets (https://rpkgs.datanovia.com/ggpubr/).
- **ggstatsplot** puts the full statistical result on the plot: test statistic, df, P, effect size with CI, and Bayes factor in the subtitle; defaults to showing raw data (Patil I. Visualizations with statistical details: the 'ggstatsplot' approach. JOSS 2021;6(61):3167, https://doi.org/10.21105/joss.03167).
- **ggprism** reproduces Prism's look in ggplot2, which shows how much the Prism style is valued (https://cran.r-project.org/web/packages/ggprism/vignettes/ggprism.html).
- Borrow: an internal plot spec modelled on the layered grammar (so every Prism-style graph is a composition, not a special case); ggstatsplot's one-line statistical caption as the default figure annotation.

### Python: seaborn and statannotations
- **seaborn** provides statistical plot types (strip, swarm, box, violin, point, regression) from long-format data in one call, with sensible defaults (Waskom ML. seaborn: statistical data visualization. JOSS 2021;6(60):3021, https://doi.org/10.21105/joss.03021).
- **statannotations** adds significance brackets to seaborn plots with an explicit choice of test (Mann-Whitney, t test, Welch, Wilcoxon, Kruskal-Wallis, custom) and multiple-comparison correction (Bonferroni, Holm, Benjamini-Hochberg, Benjamini-Yekutieli) (https://pypi.org/project/statannotations/).
- Borrow: brackets that are generated from a named test and correction, never typed by hand, and that state both in the legend.

## Estimation graphics and distribution plots

### Gardner-Altman and Cumming plots (DABEST)
The Gardner-Altman plot shows both groups' raw data on one axis and the mean difference with its 95% CI on a second, floating axis aligned to the reference group; it descends from Gardner and Altman's case for confidence intervals over P values (Gardner MJ, Altman DG. Confidence intervals rather than P values: estimation rather than hypothesis testing. BMJ 1986;292:746-50, https://doi.org/10.1136/bmj.292.6522.746). The Cumming plot extends it to several groups with the effect sizes on a lower panel. Ho et al. made both routine with DABEST, which uses bias-corrected bootstrap CIs and ships for Python, R, Matlab and as a web app (Ho J, Tumkaya T, Aryal S, Choi H, Claridge-Chang A. Moving beyond P values: data analysis with estimation graphics. Nat Methods 2019;16:565-6, https://doi.org/10.1038/s41592-019-0470-3; https://www.estimationstats.com/; https://acclab.github.io/DABEST-python/).
- Prism has a version only for t tests (see 01, section 4).
- Borrow: Gardner-Altman for two groups, Cumming for shared control and multi-group, paired variants with slope lines; bootstrap distribution drawn as a half-violin on the effect axis.

### Raincloud plots
A half-violin (the "cloud"), a box plot and jittered raw points (the "rain") in one figure, presented as a robust replacement for bars with error bars, with code for R, Python and Matlab (Allen M, Poggiali D, Whitaker K, Marshall TR, Kievit RA. Raincloud plots: a multi-platform tool for robust data visualization. Wellcome Open Res 2019;4:63, https://doi.org/10.12688/wellcomeopenres.15191.1; v2 2021 adds van Langen and an R package, https://doi.org/10.12688/wellcomeopenres.15191.2).
- Borrow: a first-class raincloud type, with repeated-measures variants that connect each subject's points.

Related and worth copying: **SuperPlots**, which colour cell-level points by biological replicate and overlay replicate means (Lord et al. J Cell Biol 2020;219(6):e202001064, https://doi.org/10.1083/jcb.202001064), and **PlotsOfData**, a web app that defaults to showing data with summaries (Postma M, Goedhart J. PLoS Biol 2019;17(3):e3000202, https://doi.org/10.1371/journal.pbio.3000202).

## Desktop plotting and analysis suites

### OriginPro
Commercial plotting and analysis suite, Windows only (macOS/Linux via virtualisation) (https://www.originlab.com/; https://us.fitgap.com/products/originpro).
- **Graph templates and themes** save a finished figure's styling for reuse on new data; **analysis templates** and batch processing re-run a full analysis over many files.
- **Recalculation mode** keeps analysis outputs linked to inputs (auto, manual or none), like Prism's hot links but user-controlled.
- Scriptable through LabTalk, Origin C and an embedded Python with the `originpro` package (https://pypi.org/project/originpro/).
- Borrow: explicit per-result recalculation mode (auto/manual/frozen) and "apply this figure's template to that data".

### SigmaPlot
Commercial, Windows-only plotting and statistics package now developed by Grafiti LLC; known for a wide catalogue of 2D and 3D technical graph types (over 100) and basic statistical tests (https://grafiti.com/; https://en.wikipedia.org/wiki/SigmaPlot; https://york.ac.uk/it-services/tools/sigmaplot).
- Borrow little beyond coverage: its catalogue is a checklist of graph types long-time users will ask for (ternary, polar, 3D mesh).

### Veusz
Free (GPL) Python/Qt scientific plotting package aimed at publication-ready PDF and SVG output, with GUI, command-line and Python scripting interfaces; plots are built from a tree of widgets (graphs, axes, functions, data with errors, keys, labels, box, polar, ternary, contour) (https://veusz.github.io/; https://pypi.org/project/veusz/).
- "VSZ documents contain commands used to generate the document" (`Add('graph', name='foo')`, Python mixed in), so a saved figure is a replayable script that the GUI also edits (https://veusz.readthedocs.io/en/latest/api.html). Output: PDF, PostScript, SVG, EMF and bitmaps.
- Borrow: the figure document as human-readable, diffable text that the GUI edits; a widget tree as the scene model.

### LabPlot
Free (GPL) KDE application for plotting and data analysis on Linux, Windows and macOS; 2.11 (July 2024) added lollipop, Q-Q and KDE plots, spreadsheet linking and a worksheet preview (https://labplot.org/posts/labplot-2-11-released/).
- Worksheets, spreadsheets and analysis (fitting, FFT, smoothing) in one project; computational notebooks sit beside the worksheets.
- Borrow: proof that a cross-platform, open, Origin-style desktop tool is viable; its project tree as a model for Dioptra's navigator.

### SciDAVis
Free (GPLv2) Qt application for data analysis and plotting, forked in 2007 from QtiPlot, itself modelled on Origin; tables with column-based formulas, 2D and 3D plots, curve fitting (GSL), FFT and filters, Python scripting, export to PDF, EPS and SVG (https://en.wikipedia.org/wiki/SciDAVis; https://scidavis.sourceforge.net/).
- Borrow: per-column formulas ("set column values") directly in the data table, the feature Prism users keep Excel for.

## Web charting and business intelligence

### Datawrapper
Browser charting service used by newsrooms: paste or upload data, choose a chart, refine, publish, in four steps (https://www.datawrapper.de/).
- Strong defaults for text, number formatting and colour; checks for colour-blind safety; annotations and highlights; responsive and static (PNG/SVG/PDF) exports.
- Borrow: the paste-preview-chart flow for first-time users; built-in colour-vision-deficiency simulation in the theme editor.

### Observable Plot
A JavaScript library for exploratory charts built on marks, scales and transforms (bin, group, stack, window), by the authors of D3 (https://observablehq.com/plot/).
- Very concise specs; facets; transforms compute summaries inside the plot.
- Borrow: a compact spec vocabulary for Dioptra's renderer and its file format (mark + channels + transform), which keeps figure specs small and diffable.

### Tableau
Commercial visual-analysis tool; users drag fields onto shelves (rows, columns, colour, size) and the VizQL engine turns the arrangement into a query and a chart (Stolte C, Tang D, Hanrahan P. Polaris: a system for query, analysis, and visualization of multidimensional relational databases. IEEE TVCG 2002;8(1):52-65, https://doi.org/10.1109/2945.981851). "Show Me" recommends chart types from the selected fields (Mackinlay J, Hanrahan P, Stolte C. Show Me: automatic presentation for visual analysis. IEEE TVCG 2007;13(6):1137-44, https://doi.org/10.1109/TVCG.2007.70594).
- Borrow: field-to-encoding drag and drop for the Multiple variables table; chart recommendation driven by variable types and the experimental design.

---

## What to take, in priority order

1. **Estimation first** (DABEST, ggstatsplot): every comparison gets an effect size, a CI and an estimation plot by default; P values are secondary.
2. **A grammar under the hood** (ggplot2, Observable Plot, Veusz): figures are specs composed of layers, stored as text, edited by GUI or by hand.
3. **Code twin for every analysis** (jamovi syntax mode): show and export the equivalent R/Python call.
4. **Live, inline assumptions** (JASP): no modal dialogs; assumption checks next to the result.
5. **Distribution-first plot types** (raincloud, SuperPlots, PlotsOfData): raw data visible by default.
6. **Column formulas in the table** (SciDAVis, jamovi, Origin): remove the reason the Excel step exists.
7. **Templates and recalculation control** (Origin, Prism): re-run a whole figure family on new data; let the user freeze results deliberately.

## Sources

- JASP, https://jasp-stats.org/ ; https://en.wikipedia.org/wiki/JASP ; ANU IT page, https://services.anu.edu.au/information-technology/software-systems/jasp
- jamovi, https://www.jamovi.org/ ; jamovi and R (syntax mode), https://jamovi.readthedocs.io/en/latest/usermanual/um_6_jamovi_and_R/ ; jmvReadWrite, https://cran.r-project.org/package=jmvReadWrite
- Wickham H. J Comput Graph Stat 2010;19(1):3-28, https://doi.org/10.1198/jcgs.2009.07098 ; ggplot2, https://ggplot2.tidyverse.org/
- ggpubr, https://rpkgs.datanovia.com/ggpubr/
- Patil I. JOSS 2021;6(61):3167, https://doi.org/10.21105/joss.03167
- ggprism, https://cran.r-project.org/web/packages/ggprism/vignettes/ggprism.html
- Waskom ML. JOSS 2021;6(60):3021, https://doi.org/10.21105/joss.03021
- statannotations, https://pypi.org/project/statannotations/
- Gardner MJ, Altman DG. BMJ 1986;292:746-50, https://doi.org/10.1136/bmj.292.6522.746
- Ho J et al. Nat Methods 2019;16:565-6, https://doi.org/10.1038/s41592-019-0470-3 ; https://www.estimationstats.com/ ; https://acclab.github.io/DABEST-python/
- Allen M et al. Wellcome Open Res 2019;4:63, https://doi.org/10.12688/wellcomeopenres.15191.1 (v2: https://doi.org/10.12688/wellcomeopenres.15191.2)
- Lord SJ et al. J Cell Biol 2020;219(6):e202001064, https://doi.org/10.1083/jcb.202001064
- Postma M, Goedhart J. PLoS Biol 2019;17(3):e3000202, https://doi.org/10.1371/journal.pbio.3000202
- OriginLab, https://www.originlab.com/ ; originpro (Python), https://pypi.org/project/originpro/ ; FitGap summary, https://us.fitgap.com/products/originpro
- SigmaPlot: Grafiti, https://grafiti.com/ ; https://en.wikipedia.org/wiki/SigmaPlot ; University of York, https://york.ac.uk/it-services/tools/sigmaplot
- Veusz, https://veusz.github.io/ ; https://pypi.org/project/veusz/ ; API and VSZ format, https://veusz.readthedocs.io/en/latest/api.html
- LabPlot 2.11 release notes, https://labplot.org/posts/labplot-2-11-released/
- SciDAVis, https://en.wikipedia.org/wiki/SciDAVis ; https://scidavis.sourceforge.net/
- Datawrapper, https://www.datawrapper.de/
- Observable Plot, https://observablehq.com/plot/
- Stolte C, Tang D, Hanrahan P. IEEE TVCG 2002;8(1):52-65, https://doi.org/10.1109/2945.981851
- Mackinlay J, Hanrahan P, Stolte C. IEEE TVCG 2007;13(6):1137-44, https://doi.org/10.1109/TVCG.2007.70594
