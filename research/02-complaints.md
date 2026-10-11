# 02 — What practitioners dislike about Prism and the Excel-then-Prism workflow

Numbered complaints **P1-P29**, each with the complaint in one sentence, the evidence, and a one-line note on what a design must do to answer it ("Design:"). A second list, **K1-K10**, records what people like and Dioptra must keep.

## Method and limits of the evidence

Sources, in order of weight:

1. **GraphPad's own documentation and knowledgebase.** Many pain points are documented by the vendor itself, as workarounds, limits or troubleshooting pages. These are the strongest evidence because they describe the product as shipped.
2. **Published papers** on data presentation and spreadsheet errors (Weissgerber et al. 2015 and 2018; Ziemann et al. 2016; Abeysooriya et al. 2021; Motulsky 2014; Lord et al. 2020).
3. **Review sites** (Capterra, G2, Software Advice). Quoted phrases come from the review listings; Capterra blocks automated fetching, so quotes were taken from the search-engine index of those pages and are attributed to the listing rather than to a named reviewer.
4. **Commentary** (blogs, vendor comparison pages). Pages by competitors (for example MetricGate) are marked as such and used only where they agree with stronger evidence.

Reddit (r/labrats, r/statistics, r/bioinformatics), ResearchGate threads and X/Twitter were searched on 2026-10-11 but are not reachable: Reddit's search API returned an HTML block page and none of the three search queries surfaced an indexed thread. No complaint below rests on those sources. This is a gap worth closing with a short user survey before the 1.0 design freeze.

Overall satisfaction is high, which matters for scope: Capterra lists Prism at 4.7/5 from 143 reviews and G2 at 4.6/5 (https://www.capterra.com/p/119786/GraphPad-Prism/reviews/, https://www.g2.com/products/graphpad-prism/reviews). People complain about the edges of a tool they mostly like.

---

## A. Cost, licensing, platform

**P1. Price and subscription licensing.** Prism costs $260 a year per academic, $520 per corporate user, $142 per student; perpetual licences cost $1,600/$3,200 and GraphPad itself says "We do not recommend this option" (no upgrades, "No guaranteed support for future operating system updates").
- Evidence: price list, https://www.graphpad.com/how-to-buy/. Capterra reviewers: "the program is expensive for students even after the discount", "even the academic pricing is still pretty steep", "a bit pricy" (https://www.capterra.com/p/119786/GraphPad-Prism/reviews/?page=2 to `?page=6`). G2 reviewers mention cost limiting access (https://www.g2.com/products/graphpad-prism/reviews). University terms describe a subscription that is "designed to cease to function upon expiration" (https://ecu.teamdynamix.com/TDClient/1409/Portal/KB/ArticleDet?ID=67736); one university notes GraphPad "no longer offers concurrent use" licences (https://uoa.freshservice.com/support/solutions/articles/50000164241-prism).
- Design: free and open source, no licence server, no expiry; files open forever.

**P2. Statistical-honesty features sit behind a higher tier.** Since Prism 11, eta squared, partial eta squared, Cohen's f, phi and Cramer's V, and in-table formulas are marked "available with our new Pro and Enterprise plans"; a file containing those results opened on a lower licence can be viewed but "you cannot recalculate them".
- Evidence: https://www.graphpad.com/guides/prism/latest/statistics/stat_anova_effect_sizes.htm, https://www.graphpad.com/guides/prism/latest/statistics/stat_chi-square_or_fishers_test.htm, https://www.graphpad.com/guides/prism/latest/user-guide/data_inspector_overview.htm.
- Design: every effect size and interval is in the base product and shown by default.

**P3. No Linux.** GraphPad: "No plans to create a Linux version of Prism or InStat", and it "has not tried to use Wine and cannot provide technical support".
- Evidence: https://www.graphpad.com/support/faq/will-you-port-prism-or-instat-to-linux-does-prism-work-with-the-windows-emulator-wine/. A G2 reviewer notes the company still resists a Linux build (https://www.g2.com/products/graphpad-prism/reviews). Many core facilities and HPC users work on Linux.
- Design: first-class Linux, Windows and macOS builds from one codebase, plus a browser build.

**P4. Windows and Mac are not the same product.** Linking to Excel files, embedding source files, direct Excel import, EMF export and EPS text-as-text exist only on Windows; PNG transparency only on Windows; Mac EPS always converts text to outlines.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/features-in-windows-but-not-ma.htm, https://www.graphpad.com/guides/prism/latest/user-guide/using_linking_and_embedding_objects.htm, https://www.graphpad.com/guides/prism/latest/user-guide/chosing_an_export_format.htm.
- Design: one rendering and import path for every platform; no OS-specific features.

## B. The Excel step and data entry

**P5. Excel round trips corrupt data before Prism sees it.** Excel's defaults turn gene symbols into dates; about one fifth of papers with Excel gene lists had such errors in 2016, 30.9% in 2021.
- Evidence: Ziemann M, Eren Y, El-Osta A. Gene name errors are widespread in the scientific literature. Genome Biol 2016;17:177, https://doi.org/10.1186/s13059-016-1044-7. Abeysooriya M et al. Gene name errors: Lessons not learned. PLoS Comput Biol 2021;17(7):e1008984, https://doi.org/10.1371/journal.pcbi.1008984. Commentary: https://retractionwatch.com/2023/09/20/guest-post-genomics-has-a-spreadsheet-problem.
- Design: make the spreadsheet step unnecessary: typed columns (text stays text), raw-data entry and import directly into the analysis tool, no silent type coercion.

**P6. Paste copies what Excel displays, not what Excel stores.** With Excel 2007 and later Prism can only paste the displayed (rounded) value; negatives formatted red-without-minus or in parentheses arrive positive; Prism for Windows refuses cells mixing digits and text.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/options_when_pasting_data_from.htm, https://www.graphpad.com/guides/prism/latest/user-guide/why_do_negative_numbers_become.htm.
- Design: read `.xlsx` directly (stored values, full precision), show a paste preview that flags rounding, sign formats and mixed cells before committing.

**P7. Some tables silently rewrite pasted numbers.** In contingency and survival tables "If the data you are importing or pasting has a decimal point, this will be ignored, so '1.23' will paste or import as '123'", and minus signs are dropped.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/why_do_negative_numbers_become.htm.
- Design: never transform input silently; reject or flag invalid values in place with the reason.

**P8. Direct Excel import is fragile.** It is Windows only, reads only the worksheet that was active when the workbook was saved, and drives a hidden Excel over OLE that "simply doesn't work on some computers"; the guide recommends copy and paste instead.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/using_importing_native_excel_files_w.htm.
- Design: native `.xlsx`/`.ods`/CSV readers with a sheet and range picker, no Office dependency.

**P9. Choosing the right table format is the hardest part for new users.** Reviewers: "It can initially be difficult to understand how to input data correctly"; "users must know how to input that data correctly. Building this experience takes time"; "I am used to the flexibility of excel and graphpad is so confusing and limited for me". The guide needs a page just to tell Nested from Grouped tables, and warns "Don't think 'make a bar graph'".
- Evidence: Capterra listing pages 2-6 (https://www.capterra.com/p/119786/GraphPad-Prism/reviews/); https://www.graphpad.com/guides/prism/latest/user-guide/the-difference-between-nested-.htm; https://www.graphpad.com/guides/prism/latest/user-guide/using_change_graph_type.htm.
- Design: ask about the experiment (factors, pairing, replicates, nesting) and derive the layout; allow re-viewing the same data under another design without re-entry.

**P10. Replicates are laid out differently in different tables.** Side-by-side subcolumns in XY and Grouped tables, stacked in Column tables, stacked within subcolumns in Nested tables; moving data between designs means rearranging it by hand.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/stacked_vs__side-by-side_repli.htm.
- Design: store long-format observations once; render each table type as a view.

**P11. Long ("tidy") data must be reshaped to fit.** Only the Multiple variables table is long-format; importing indexed data into other tables uses an "unstack" filter whose group column "must contain integers (not text)"; placement "by rows" ignores the subcolumn/data-set boundary.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_filter.htm, https://www.graphpad.com/guides/prism/latest/user-guide/using_paste_special_placement.htm, https://www.graphpad.com/guides/prism/latest/user-guide/multiple-variable-tables.htm.
- Design: import long data with text factors directly; pivot to any design view by choosing columns.

**P12. No formulas where people expect them.** Derived values are produced by Transform, Normalize or Remove-baseline analyses that write new results tables; in-table formulas arrived only in Prism 11, only in Multiple variables tables, only on Pro/Enterprise plans. This is a main reason the Excel step survives.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/using_transform.htm, https://www.graphpad.com/guides/prism/latest/user-guide/data_inspector_overview.htm.
- Design: calculated columns in every table, recorded as expressions in the file, recalculated live.

**P13. Missing values break some designs.** Classical repeated-measures ANOVA cannot have missing cells (Prism switches to a mixed model); unbalanced two-way ANOVA from mean/SD/N data gives only approximate results by "unweighted means"; nested tables reject averaged data.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/missing_values.htm, https://www.graphpad.com/guides/prism/latest/user-guide/two_grouping_variable_table.htm, https://www.graphpad.com/guides/prism/latest/user-guide/nested-tables.htm.
- Design: state on the results sheet which method ran and why (for example "mixed model because 2 cells missing"), and warn when results are approximate.

## C. Statistics: assumptions, choices, reporting

**P14. "P-hacking by dialog".** Every test is one click from every other; nothing records which tests were tried before the one reported. GraphPad's own guide carries "Don't be a P-hacker", "Advice: Don't P-Hack" and "Advice: Don't automate the decision to use a nonparametric test", and its founder lists P-hacking as the first common misconception ("reanalyzing a data set in many different ways ... until you get the result you want").
- Evidence: Statistics Guide TOC (https://www.graphpad.com/guides/prism/latest/statistics/js/hmcontent.js; pages `stat_advice_dont_p-hack.htm`, `using_a_normality_test_to_choo.htm`); Motulsky HJ. Common misconceptions about data analysis and statistics. Naunyn-Schmiedeberg's Arch Pharmacol 2014;387:1017-23, https://doi.org/10.1007/s00210-014-1037-6 (open copy https://pmc.ncbi.nlm.nih.gov/articles/PMC4203998/).
- Design: an append-only analysis log in the project file (every test run, every option change, every exclusion), visible and exportable; optional pre-registration of the planned test.

**P15. Assumptions are documented, not checked.** The analysis checklists are help pages reached from the results; whether data are paired, independent, Gaussian or equal-variance is left to the user, and outlier exclusion (manual or ROUT) is one click.
- Evidence: https://www.graphpad.com/guides/prism/latest/statistics/analysis_checklists2.htm; https://www.graphpad.com/guides/prism/latest/user-guide/excluding_values.htm; the guide's own "Advice: Beware of identifying outliers manually" (Statistics Guide TOC).
- Design: put the checklist on the results sheet with each item's evidence (residual plot, variance ratio, n per group) and record exclusions with a reason.

**P16. Asterisks and "significant" are the default summary.** Results and graphs lead with `*`/`ns`; the 8.0-8.2 dialog's example values `0.0332`, `0.0021`, `0.0002` were widely misread as thresholds, so GraphPad had to publish a correction. The guide itself advises avoiding the word "significant", in line with calls to retire significance thresholds.
- Evidence: https://www.graphpad.com/support/faq/what-is-the-meaning-of--or--or--in-reports-of-statistical-significance-from-prism-or-instat/; https://www.graphpad.com/guides/prism/latest/statistics/extremely_significant_results.htm; Amrhein V, Greenland S, McShane B. Scientists rise up against statistical significance. Nature 2019;567:305-7, https://doi.org/10.1038/d41586-019-00857-9.
- Design: lead every result with the effect size and its CI; exact P values; asterisks available but off by default and always defined in the legend.

**P17. Methods are under-reported, and the tool does not help.** In 225 physiology papers using ANOVA, many did not say which ANOVA, which factors or which post-test was used.
- Evidence: Weissgerber TL et al. Meta-Research: Why we need to report more than "Data were analyzed by t-tests or ANOVA". eLife 2018;7:e36163, https://elifesciences.org/articles/36163.
- Design: generate a methods paragraph and figure-legend text from the analysis record (test, design, sidedness, correction, n per group, software version).

**P18. Effect sizes and estimation came late and only partly.** Estimation plots arrived in Prism 9 and "are currently only available for t tests"; ANOVA effect sizes beyond R squared are tier-gated (P2).
- Evidence: https://www.graphpad.com/guides/prism/latest/statistics/stat_estimation_plots.htm; Ho J et al. Moving beyond P values: data analysis with estimation graphics. Nat Methods 2019;16:565-6, https://doi.org/10.1038/s41592-019-0470-3.
- Design: estimation graphics for every comparison (two-group, paired, multi-group shared control, repeated measures), with bootstrap CIs.

**P19. Hierarchical data are easy to analyse as if independent.** Cells from a few animals entered as replicates inflate n; Nested tables handle only two levels and the default graphs do not show the biological replicate.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/nested-tables.htm; Lord SJ, Velle KB, Mullins RD, Fritz-Laylin LK. SuperPlots: Communicating reproducibility and variability in cell biology. J Cell Biol 2020;219(6):e202001064, https://doi.org/10.1083/jcb.202001064.
- Design: make "what is the experimental unit?" a required question; offer SuperPlots and mixed models when the answer differs from the row.

## D. Reproducibility, files, collaboration

**P20. No scripted record of the analysis.** Prism keeps dialog settings with each result, but there is no human-readable, re-runnable record of the whole pipeline; its scripting language only automates imports, exports and loops. Commentators contrast this with code-based pipelines where the script is the audit trail.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/using_scripts.htm; review summary noting a "point-and-click workflow with limited scripting and no native notebook environment" and no version-control integration (https://us.fitgap.com/products/003466/graphpad-prism); "Are we kissing GraphPad Prism goodbye?" (Wildtype One, 2025), https://wildtypeone.substack.com/p/are-we-kissing-graphpad-prism-goodbye; competitor commentary (MetricGate, marked as vendor) https://metricgate.com/blogs/metricgate-vs-graphpad-prism/.
- Design: the project file is the script: a declarative, text-diffable list of data sources, transforms, analyses and figure specs that the app replays deterministically; export to R/Python for checking.

**P21. The file format has been opaque.** In `.pzfx` "The rest of the file is unreadable outside Prism" (graphs and analyses are a base64 blob); `.pzf` is binary. The new `.prism` (CSV + JSON in a zip) is a real improvement but has no published schema, and third-party readers are reverse-engineered.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/pzf_vs__pzfx_file_format.htm; https://www.graphpad.com/guides/prism/latest/user-guide/prism_file_format.htm; the `pzfx` R package (https://github.com/Yue-Jiang/pzfx) and `prism2R` (https://r-multiverse.r-universe.dev/prism2R).
- Design: publish the file schema with the 1.0 release, version it, and ship a reference reader in R and Python.

**P22. Version lock-in.** Prism 9 and earlier cannot open `.prism` files ("Unable to open file. It is probably not a GraphPad Prism file"); newer features force the new format; reviewers say "you can't easily transfer graphpad files between different versions".
- Evidence: https://graphpad.com/support/faq/opening-new-file-format-in-old-prism-versions; https://www.graphpad.com/guides/prism/latest/user-guide/compatibility_between_prism_versions.htm; Capterra listing (https://www.capterra.com/p/119786/GraphPad-Prism/reviews/).
- Design: forward-compatible format (unknown fields preserved), documented migrations, and an old-version reader kept in the test suite.

**P23. No version control.** Files are binary or zipped blobs that do not diff; Prism Cloud keeps full publish history only on Enterprise plans.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/prism_cloud_co_editing.htm.
- Design: a format that diffs line by line once unpacked (stable key order, one record per line), plus a built-in history view.

**P24. Collaboration is file passing.** Prism Cloud is "in beta", "only available with eligible subscriptions and in specific regions"; co-editing means downloading, editing locally and publishing a new version; personal licences "can't be shared"; colleagues without a licence get a read-only viewer after the trial.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/prism_cloud_co_editing.htm; https://www.graphpad.com/how-to-buy/; https://www.graphpad.com/guides/prism/latest/user-guide/a_complete_record_of_your_work.htm.
- Design: free software removes the licence problem; a plain file plus Git or any shared folder covers most teams; a read-only web viewer covers reviewers.

## E. Graphs and export

**P25. Export to journals and Illustrator needs workarounds.** EPS cannot embed fonts the Adobe way ("Adobe has not provided the hooks"), Mac EPS converts text to outlines, EPS and JPEG lose transparency, EMF+ can corrupt in older Illustrator, pasted graphs need "Release Clipping Mask" repeatedly and "Path > Clean Up" to remove invisible objects.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/exporting_to_journals.htm; https://www.graphpad.com/guides/prism/latest/user-guide/fonts_and_eps_files.htm; https://www.graphpad.com/support/faq/tips-to-get-a-prism-graph-into-adobe-illustrator/ (KB #1066).
- Design: PDF and SVG with embedded, subset fonts and real text; clean, grouped, named SVG layers with no clipping-mask clutter; exact physical sizes; one renderer for screen and export.

**P26. Figures still get finished in other software.** The export pages assume Illustrator as the final stop; reviewers say it "does not have the best graphic features" and that they keep copying graphs to keep them consistent ("I end up copying a lot of graphs, but that can be annoying").
- Evidence: KB #1066 above; Capterra and G2 listings (https://www.capterra.com/p/119786/GraphPad-Prism/reviews/, https://www.g2.com/products/graphpad-prism/reviews).
- Design: multi-panel layout, panel letters, shared styles (themes applied project-wide) and alignment inside the app, so the exported PDF is final.

**P27. Limited colour and theme control.** Colour-blind-safe schemes arrived only in 9.5; custom colours on Windows go through "a standard Windows dialog that we don't have control over"; the ggprism author notes most Prism palettes are not colour-blind friendly.
- Evidence: https://www.graphpad.com/support/faq/colorblind-safe-colors-schemes-and-transparency; https://www.graphpad.com/support/faq/im-having-trouble-creating-custom-colors-in-prism-windows/ (KB #1185); ggprism colours vignette, https://cran.r-project.org/web/packages/ggprism/vignettes/colours.html.
- Design: named, validated palettes (CVD-safe by default), project-wide themes as editable files, hex/OKLCH entry.

**P28. Modern distribution plots arrived late; bar charts stayed the default.** Violin plots are "new in Prism 8"; estimation plots arrived in Prism 9 for t tests only; the current guide has no raincloud or SuperPlot entry. Meanwhile 85.6% of physiology papers reviewed used bar graphs for continuous data.
- Evidence: https://www.graphpad.com/guides/prism/latest/user-guide/graphing-box-and-whisker-and-v.htm; https://www.graphpad.com/guides/prism/latest/statistics/stat_estimation_plots.htm; Weissgerber TL, Milic NM, Winham SJ, Garovic VD. Beyond bar and line graphs. PLoS Biol 2015;13(4):e1002128, https://doi.org/10.1371/journal.pbio.1002128; "Open letter to journal editors: dynamite plots must die", https://simplystatistics.org/posts/2019-02-21-dynamite-plots-must-die/; Allen M et al. Raincloud plots. Wellcome Open Res 2019;4:63, https://doi.org/10.12688/wellcomeopenres.15191.1.
- Design: default to showing every point; offer raincloud, SuperPlot, estimation and paired-slope plots as first-class types; make "bar of means" an explicit opt-in.

## F. Performance and reliability

**P29. Slow or unstable with large projects and tables.** GraphPad: "When a data table is huge (tens of thousands of rows), data analysis and graphing can be slow" (advice: prune or decimate); multiple comparisons on large two-way tables can appear frozen; two-way ANOVA has size limits. Reviewers: "GraphPad does not deal with lots of data very well. When a lot of tables/graphs are loaded, it can crash the program randomly"; a G2 reviewer reports hangs after more than eight new analysis files.
- Evidence: https://www.graphpad.com/support/faq/trimming-or-decimating-to-reduce-the-size-of-huge-data-tables (KB #1489); https://www.graphpad.com/support/faq/mult_comp_performance_formatted_text (KB #2255); https://www.graphpad.com/support/faq/how-many-rowscolumns-can-two-way-anova-analyze (KB #1985); Capterra and G2 listings; https://www.g2.com/survey_responses/graphpad-prism-review-3302764.
- Design: columnar storage, incremental recomputation of only the dependent analyses, background threads, and plots that decimate for display but never for analysis; benchmark with 10^6-row tables in CI.

Smaller recurring complaints, not numbered: slow e-mail support ("going back and forth ... via email for 3 weeks with no resolution", Capterra page 3); results harder to find after the Prism 8 interface change ("Now one has to dig around to look for results of analyses", https://softwareadvice.fr/software/278478/graphpad-prism?page=3); some analyses missing so users fall back to R ("requires me to use a more sophisticated analysis software (such as packages in R)", Capterra).

---

## What people like (and Dioptra must keep)

**K1. Guided analysis.** The table format narrows the analyses to ones that fit the design; the Analyze dialog groups them by table; the guide is written "for scientists, not statisticians" (https://www.graphpad.com/guides/prism/latest/user-guide/helps_you_learn_data_analysis.htm). Even critics credit "point-and-click simplicity and built-in stats" (https://wildtypeone.substack.com/p/are-we-kissing-graphpad-prism-goodbye).

**K2. Results linked to data.** Fix a typo and every result and graph updates; reopen any analysis and see every choice (https://www.graphpad.com/guides/prism/latest/user-guide/everything_is_hot_linked.htm).

**K3. Auto-updating graphs and repeatable families.** Graphs are born with the table; the Wand, templates and methods re-run a whole analysis on a new experiment without code (https://www.graphpad.com/guides/prism/latest/user-guide/use_the_wand_to_repeat_analyse.htm).

**K4. Publication-quality defaults.** Reviewers' most common praise; R users built `ggprism` because they "missed the 'look'" (https://cran.r-project.org/web/packages/ggprism/vignettes/ggprism.html). Even a competitor concedes Prism "keeps the edge on hand-styled figures" (https://metricgate.com/blogs/metricgate-vs-graphpad-prism/).

**K5. The analysis checklist.** A per-analysis list of assumptions to confirm, in plain language (https://www.graphpad.com/guides/prism/latest/statistics/analysis_checklists2.htm).

**K6. Documentation as a statistics course.** The Statistics and Curve Fitting Guides teach the reasoning (P values, CIs, multiple comparisons, weighting, model comparison), not just the buttons.

**K7. Nonlinear regression done well.** A large, named equation library with automatic initial values, global fitting with shared parameters, constraints, model comparison (F test and AICc), ROUT, interpolation, and clear failure flags ("Ambiguous", "Hit constraint") (https://www.graphpad.com/guides/prism/latest/curve-fitting/reg_models_built-in_to_prism.htm).

**K8. Raw data and summary data both accepted.** Mean/SD/N entry with correct downstream arithmetic lets people analyse published or instrument-summarised data.

**K9. Excluded values stay visible.** Blue italic with an asterisk: excluded from analysis but kept as a record (https://www.graphpad.com/guides/prism/latest/user-guide/excluding_values.htm).

**K10. Works offline, on the bench computer, with no code.** Credited even by competitors (https://metricgate.com/blogs/metricgate-vs-graphpad-prism/).

---

## Sources

GraphPad (vendor documentation and knowledgebase)
- How to buy (prices, licence terms), https://www.graphpad.com/how-to-buy/
- KB #326, Linux and Wine, https://www.graphpad.com/support/faq/will-you-port-prism-or-instat-to-linux-does-prism-work-with-the-windows-emulator-wine/
- KB, meaning of asterisks, https://www.graphpad.com/support/faq/what-is-the-meaning-of--or--or--in-reports-of-statistical-significance-from-prism-or-instat/
- KB #1066, Prism graphs into Illustrator, https://www.graphpad.com/support/faq/tips-to-get-a-prism-graph-into-adobe-illustrator/
- KB #1185, custom colours on Windows, https://www.graphpad.com/support/faq/im-having-trouble-creating-custom-colors-in-prism-windows/
- KB, colour-blind-safe schemes, https://www.graphpad.com/support/faq/colorblind-safe-colors-schemes-and-transparency
- KB #1489, huge tables, https://www.graphpad.com/support/faq/trimming-or-decimating-to-reduce-the-size-of-huge-data-tables
- KB #2255, slow multiple comparisons, https://www.graphpad.com/support/faq/mult_comp_performance_formatted_text
- KB #1985, two-way ANOVA size limits, https://www.graphpad.com/support/faq/how-many-rowscolumns-can-two-way-anova-analyze
- KB, `.prism` in older versions, https://graphpad.com/support/faq/opening-new-file-format-in-old-prism-versions
- User Guide pages cited inline under https://www.graphpad.com/guides/prism/latest/user-guide/
- Statistics Guide pages cited inline under https://www.graphpad.com/guides/prism/latest/statistics/

Papers
- Ziemann M, Eren Y, El-Osta A. Genome Biol 2016;17:177, https://doi.org/10.1186/s13059-016-1044-7
- Abeysooriya M, Soria M, Kasu MS, Ziemann M. PLoS Comput Biol 2021;17(7):e1008984, https://doi.org/10.1371/journal.pcbi.1008984
- Weissgerber TL, Milic NM, Winham SJ, Garovic VD. PLoS Biol 2015;13(4):e1002128, https://doi.org/10.1371/journal.pbio.1002128
- Weissgerber TL, Garcia-Valencia O, Garovic VD, Milic NM, Winham SJ. eLife 2018;7:e36163, https://elifesciences.org/articles/36163
- Motulsky HJ. Naunyn-Schmiedeberg's Arch Pharmacol 2014;387:1017-23, https://doi.org/10.1007/s00210-014-1037-6
- Amrhein V, Greenland S, McShane B. Nature 2019;567:305-7, https://doi.org/10.1038/d41586-019-00857-9
- Ho J, Tumkaya T, Aryal S, Choi H, Claridge-Chang A. Nat Methods 2019;16:565-6, https://doi.org/10.1038/s41592-019-0470-3
- Lord SJ, Velle KB, Mullins RD, Fritz-Laylin LK. J Cell Biol 2020;219(6):e202001064, https://doi.org/10.1083/jcb.202001064
- Allen M, Poggiali D, Whitaker K, Marshall TR, Kievit RA. Wellcome Open Res 2019;4:63, https://doi.org/10.12688/wellcomeopenres.15191.1

Reviews and commentary
- Capterra, GraphPad Prism reviews, https://www.capterra.com/p/119786/GraphPad-Prism/reviews/ (pages 2-6)
- G2, GraphPad Prism reviews, https://www.g2.com/products/graphpad-prism/reviews ; review 3302764, https://www.g2.com/survey_responses/graphpad-prism-review-3302764
- Software Advice (FR), GraphPad Prism reviews, https://softwareadvice.fr/software/278478/graphpad-prism?page=3
- FitGap product summary, https://us.fitgap.com/products/003466/graphpad-prism
- Wildtype One, "Are we kissing GraphPad Prism goodbye?", 2025, https://wildtypeone.substack.com/p/are-we-kissing-graphpad-prism-goodbye
- Simply Statistics, "Open letter to journal editors: dynamite plots must die", 2019, https://simplystatistics.org/posts/2019-02-21-dynamite-plots-must-die/
- Retraction Watch guest post, "Genomics has a spreadsheet problem", 2023, https://retractionwatch.com/2023/09/20/guest-post-genomics-has-a-spreadsheet-problem
- MetricGate comparison (competitor; used only where corroborated), https://metricgate.com/blogs/metricgate-vs-graphpad-prism/
- ggprism vignettes, https://cran.r-project.org/web/packages/ggprism/vignettes/ggprism.html and https://cran.r-project.org/web/packages/ggprism/vignettes/colours.html
- University licensing pages: https://ecu.teamdynamix.com/TDClient/1409/Portal/KB/ArticleDet?ID=67736 ; https://uoa.freshservice.com/support/solutions/articles/50000164241-prism
