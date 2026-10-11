# Dioptra — plan

Phases run in order; each has a verification gate. Sub-agents implement; the architect
reviews numerical correctness and design at every gate. Build output stays inside the
workspace (`target/`, `.scratch/`), never in a tmpfs scratchpad.

## Phase 0 — Research and design (done before code)

- `research/00..05`: name and stack, Prism feature surface, complaints P1…Pn, competitors,
  algorithm references, NIST and oracle notes.
- `fixtures/nist/*` and `fixtures/oracle/*.json` with `scripts/oracle/*.py` generators.
- `DESIGN.md`, `PLAN.md`, `docs/DEFERRED.md`.
- Gate: every fixture file loads; every algorithm in the v0.1 list has a named reference.

## Phase 1 — Workspace skeleton

- Cargo workspace: `crates/dioptra-stats`, `crates/dioptra-core`, `crates/dioptra-plot`,
  `crates/dioptra-wasm`, `crates/dioptra-cli`, `apps/dioptra/src-tauri`.
- Pinned toolchain 1.99.0 with `wasm32-unknown-unknown`; `rustfmt.toml`; `verify` script;
  `.gitignore`; MIT licence; CI workflow.
- The bridge contract: `dioptra_core::api::{Command, Response}` (serde), one dispatcher.
- Gate: `cargo build --workspace` and `cargo build -p dioptra-wasm --target wasm32-unknown-unknown`.

## Phase 2 — The statistical engine (`dioptra-stats`)

Split in three jobs so two can run at once:

- **2A special functions and distributions, descriptives, t tests, nonparametrics.**
  `special`: lgamma/erf (libm), regularised incomplete beta and gamma with inverses,
  log-binomial; `dist`: normal, t, chi², F, beta, gamma, binomial, hypergeometric,
  non-central t (Lenth AS 243), studentised range (Copenhaver–Holland integration),
  Dunnett's two-sided via multivariate-t integration (or numerical integration against the
  normal, as in R's `multcomp`); `descriptive`; `ttest` (one-sample, Student, Welch, paired,
  ratio paired; CIs, Cohen's d, Hedges' g); `rank` (ties, midranks); `mannwhitney` (exact by
  recurrence for n₁n₂ ≤ 10⁴ without ties, normal with tie correction otherwise;
  Hodges–Lehmann); `wilcoxon` (exact generating function, Pratt and Wilcoxon zero handling).
- **2B ANOVA, multiple comparisons, contingency, correlation, regression.**
  One-way (with Welch and Brown–Forsythe), two-way with interaction (type I/II/III via the
  general linear model with sum-to-zero coding), repeated-measures one-way with
  Greenhouse–Geisser and Huynh–Feldt and Mauchly's W; Kruskal–Wallis, Friedman; Tukey–Kramer,
  Dunnett, Dunn, Šidák, Holm, Bonferroni, Benjamini–Hochberg; chi-square (with and without
  Yates), goodness of fit, Fisher exact 2×2 with conditional MLE odds ratio CI, Fisher r×c by
  the network algorithm (Mehta–Patel) with an enumeration fallback for tiny tables, McNemar,
  relative risk and odds ratio with Woolf CIs, Cochran–Armitage; Pearson with Fisher-z CI,
  Spearman (exact for n ≤ 9), Kendall τ-b; simple and multiple linear regression with
  diagnostics (leverage, Cook's D, Durbin–Watson, runs test), confidence and prediction
  bands, Deming.
- **2C nonlinear regression, normality, outliers, survival, ROC, agreement, PCA.**
  LM (Moré 1978), analytic Jacobians for the built-in library and finite differences for
  custom equations, weighting (none, 1/Y, 1/Y², 1/SD²), robust Lorentzian loss, parameter
  constraints, asymptotic and profile-likelihood CIs, R², Sy.x, AICc, extra-sum-of-squares F,
  replicates test, runs test, the equation library (4PL/5PL with EC50/IC50 and Hill slope,
  log-agonist variants, one- and two-phase exponential decay and association, plateau
  variants, Michaelis–Menten, Hill, substrate inhibition, saturation and competition binding,
  Gaussian, Lorentzian, polynomial 1–6, linear, segmental linear, custom expression),
  Shapiro–Wilk (Royston AS R94), D'Agostino–Pearson K², Anderson–Darling (Stephens),
  Kolmogorov–Smirnov and Lilliefors; Grubbs; ROUT (Motulsky–Brown 2006); Kaplan–Meier with
  Greenwood and log-log CIs, median survival with CI, log-rank, Gehan–Breslow–Wilcoxon,
  Mantel–Haenszel hazard ratio; ROC with DeLong CI, Youden index; Bland–Altman with CIs of
  the limits; PCA by SVD with a documented sign convention; bootstrap CI of differences for
  estimation plots.
- Gate: all oracle fixtures pass at the stated tolerances; NIST LRE table generated
  (`docs/CORRECTNESS.md`); `cargo clippy -D warnings`; `docs/METHODS.md` names a reference
  per function; architect review of every p-value path and of LM on the higher-difficulty
  NIST sets.

## Phase 3 — The document model (`dioptra-core`)

Tables of all eight kinds; columns with sources and the sparse edit log; the formula
language (lexer, Pratt parser, evaluator, broadcasting, unit conversion table); derived
tables; dependency graph with pull recomputation; analyses as specs with results sheets
built from `dioptra-stats`; p-value styles and APA sentences; the command log with inverses;
import (clipboard text with locale detection, CSV/TSV, XLSX via calamine, `.pzfx`); export
(CSV, XLSX, `.dioptra`); the project file (`docs/FILE-FORMAT.md`); the `Command` /
`Response` API with viewport cell fetches.

- Gate: unit tests for every table kind, parser golden tests, recompute tests, round-trip
  tests (save → open → identical), a 1,000,000-cell paste and recompute benchmark recorded
  in `docs/PERFORMANCE.md`.

## Phase 4 — The plot engine (`dioptra-plot`)

Scene graph (points, mm, pt), typography with bundled Inter via `fontdb`/`rustybuzz`
through usvg's text layout, axes (linear, log, categorical, broken, breaks), scales,
legends, every graph family in DESIGN §6, themes and palettes, SVG writer with element ids
for hit-testing, PDF via svg2pdf with embedded fonts, PNG via resvg at any DPI, multi-panel
layouts.

- Gate: SVG snapshot tests per family and theme; PDF files parse (header, xref, fonts
  embedded); PNG dimensions match requested mm × DPI; redraw timing for 10⁴ and 10⁵ points.

## Phase 5 — The app (`apps/dioptra`)

- 5A shell and grid: Tauri setup, bridge (`engine.ts` with Tauri and wasm backends), project
  rail, virtualised grid with full keyboard set, paste, formula bar, inspector, status line,
  command palette, undo/redo, themes, open/save dialogs.
- 5B analyses and graphs: analysis picker with fit/unfit reasons, options panel, results
  sheets with copy, graph editor (inline SVG with hit-testing, inspector, drag, in-place
  text), export dialog, layouts.
- Gate: `tsc`, vitest for pure UI logic, Playwright e2e against the wasm build in Chrome
  (paste → ANOVA → graph → export SVG → save → reopen), desktop debug build launches under
  `timeout` with Xvfb and renders.

## Phase 6 — CLI, wasm, docs, screenshots

`dioptra` CLI: `dioptra run project.dioptra`, `dioptra import data.csv --kind column`,
`dioptra render graph --pdf`, `dioptra bench`. Wasm package built by `wasm-bindgen` into
`apps/dioptra/src/engine/wasm/`. README with screenshots captured from the Linux build.

## Phase 7 — Release

`verify` green; `ci.yml` green on push; `release.yml` on `v0.1.0` building Linux (AppImage,
deb, rpm), Windows (msi, nsis exe), macOS (aarch64, x86_64 dmg); README install lines point at
the assets. Tag once.

## What v0.1 contains

Everything in DESIGN §4–§7 and the analyses listed in Phase 2. Deferred items with reasons:
`docs/DEFERRED.md`.

## Resource rules

`CARGO_BUILD_JOBS=6` while `free -g` shows > 10 GB available, else 4. At most two sub-agents
at once, one headless browser. Kill by PID or port only.
