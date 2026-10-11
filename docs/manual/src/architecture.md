# Architecture and internals

This chapter is for people who want to read or change the code. It explains how the parts
fit, why they are split the way they are, and how to add the two things people most often
want to add: an analysis and a graph family. `CONTRIBUTING.md` in the repository has the
setup steps and the rules for numerical code.

## The shape of the system

```text
 apps/dioptra (React UI)
        │  Command (JSON)            ▲ Response (JSON)
        ▼                            │
 ┌─ Tauri: invoke("dispatch") ──┐  or  ┌─ browser: wasm Engine.dispatch ─┐
 └──────────────┬───────────────┘      └───────────────┬─────────────────┘
                ▼                                      ▼
            dioptra-core  ── Session: one dispatcher, the document model
             │        │
             ▼        ▼
     dioptra-stats   dioptra-plot
     (numerics)      (scene, SVG, PDF, PNG)
```

Everything that computes is Rust. The UI is TypeScript and React and never does
arithmetic on data. It sends commands and draws responses. This split exists for two
reasons. First, one engine means one set of answers: the desktop app, the browser build
used in tests and the command line give identical numbers because they run identical code.
Second, Rust compiles to native code for the desktop and to WebAssembly for the browser
without changes.

## The crates

**`dioptra-stats`** is the numerical library: special functions (incomplete beta and
gamma, their inverses), distributions (t, F, chi-square, noncentral t, studentised range,
Dunnett), descriptive statistics, t tests, rank tests, ANOVA, post-hoc comparisons,
contingency tables, correlation, linear regression with a refined QR solver,
Levenberg–Marquardt nonlinear regression with the equation library, normality tests,
outliers, survival, ROC, agreement, PCA, bootstrap and permutation. It knows nothing about
tables or files: numbers in, numbers out. Every public function names its reference in
`docs/METHODS.md`.

**`dioptra-core`** is the document model: tables of the eight kinds, columns with
provenance, the formula language (lexer, Pratt parser, evaluator), derived tables, the
dependency graph and pull-based recomputation, analyses as specifications with results
sheets, the command history with inverses for undo, import and export, the `.dioptra` file,
and the API. `compute/` wires each analysis kind to `dioptra-stats` and writes its results
sheet, including the assumption checks (`compute/checks.rs`).

**`dioptra-plot`** turns a graph specification and extracted data into a *scene*: a
device-independent list of shapes and text measured in points. Text is measured with the
bundled Inter fonts' own metrics, so layout never depends on the system. The scene is
written as SVG by the crate's own writer, as PNG through resvg and as PDF through svg2pdf.

**`dioptra-wasm`** is a thin `wasm-bindgen` facade: an `Engine` with one method,
`dispatch(json) -> json`.

**`dioptra-cli`** runs the same engine from the shell: `dioptra analyze data.csv --kind
column --test one_way_anova`, and `info`, `run`, `import`, `render`, `export`, `bench` and
`equations` on a project file, for scripted pipelines and continuous integration.

**`apps/dioptra`** is the Tauri 2 desktop app with the React UI. `src-tauri/src/lib.rs`
exposes one Tauri command, `dispatch`. `src/engine/` holds the bridge with two backends,
Tauri and wasm, behind one interface, and a queue that keeps commands in order.

## The Command/Response bridge

The whole API is two enums in `crates/dioptra-core/src/api.rs`. `Command` is every request
the UI can make: create a table, paste text at a position, fetch a viewport of cells, set a
column formula, list the analyses that fit a table, create an analysis, get its results
sheet, render a graph to SVG, patch a graph specification, undo, save. `Response` is every
answer: `Ok` with what changed, `Error` with a message, `Cells`, `PasteReport`,
`Results`, `Svg` with hit-test targets, `Bytes` for files. Both are serde-tagged JSON.

```json
{"type": "paste", "table": "t1", "row0": 0, "col0": 0,
 "text": "1,5\t2,5\n3,0\t4,5", "options": {}}
```

`Session::dispatch_json` parses the command, runs it, and serialises the response. Commands
that mutate the project record an inverse for undo and a line in the history.

Why JSON across the bridge instead of typed bindings? Because the same string goes through
Tauri's IPC, through wasm-bindgen, and into a test, and it can be logged and replayed. The
cost is a parse per call; at the sizes involved (a viewport of a few hundred cells, a
results sheet) it is a fraction of a millisecond, measured in [Performance](performance.md).

Large data never cross the bridge whole. The grid asks for the cells in view
(`GetCells` with a range), so a million-cell table costs the UI only what is on screen.

## The WebAssembly build

`scripts/build-wasm.sh` compiles `dioptra-wasm` for `wasm32-unknown-unknown` and runs
`wasm-bindgen` into `apps/dioptra/src/engine/wasm/pkg`. The UI then runs in a plain browser
against the same engine. This build is how the end-to-end tests run: Playwright drives
Chrome through paste, analysis, graph, export, save and reopen, with no desktop window
involved. A hosted web version would use the same build; it is deferred.

## Adding an analysis

Take a hypothetical *sign test* as the example.

1. **The numerics** go in `dioptra-stats`: a function from values to a result struct (the
   statistic, the p value, a confidence interval). Name the reference in its doc comment
   and in `docs/METHODS.md`.
2. **The oracle.** Add cases to `scripts/oracle/gen_fixtures.py` that compute the same
   quantities with SciPy (`scipy.stats.binomtest` for a sign test), regenerate the fixture
   JSON, and write a test that reads it and compares at the stated tolerance. Include the
   awkward cases: ties, zeros, n = 1, all values equal.
3. **The specification.** Add a `SignTestSpec` with `spec_struct!` in
   `crates/dioptra-core/src/analysis.rs`, a variant of `AnalysisSpec`, an entry in `KINDS`
   (the display name), a default in `default_for`, and the table kinds it accepts in
   `accepts`. The JSON form is `{"kind": "sign_test", ...}`.
4. **The computation.** Write `fn sign_test(ctx: &AnalysisContext) -> Result<ResultsSheet,
   String>` in `crates/dioptra-core/src/compute/` and register it in `register_all`. Use
   `extract` to get clean values with notes about what was left out, `checks` for the
   assumption checks, and the helpers for the methods line, summary sentences in the
   project's p-value style, and typed sections.
5. **The UI.** Add a one-line description to `apps/dioptra/src/analysis/descriptions.ts`
   and the options to `schema.ts`; the options panel is generated from the schema.
6. **The documentation.** An entry in `docs/ANALYSES.md`: when to use it, assumptions,
   options, outputs, reference, worked example.

## Adding a graph family

1. Write a module in `crates/dioptra-plot/src/families/` with
   `pub fn plot(ctx: &Ctx) -> Plot`: read the extracted data and the spec's options, and
   return axis plans and layers of marks in data units. The shared layout code measures
   labels, places axes and legends, and maps everything to points.
2. Add the name to `FAMILIES` and a branch in `build` in `families/mod.rs`.
3. Give every layer an id and a target pointer so the graph editor can select it
   (the table of targets is in [the graph reference](graphs/reference.md)).
4. Add a snapshot test: a golden SVG per theme in `crates/dioptra-plot/tests/snapshots/`.
   `UPDATE_SNAPSHOTS=1 cargo test -p dioptra-plot` writes it; look at it before committing.
5. Add the family to the UI's list in `apps/dioptra/src/graph/families.ts` and document it
   in `docs/GRAPHS.md`.

## The test strategy

Each layer is tested against the strongest reference available for it.

- **Numerics** against NIST StRD certified values and oracle fixtures from SciPy,
  statsmodels, lifelines and scikit-learn, plus identities and textbook examples (see
  [Correctness](correctness.md)). Exact quantities are compared at 10⁻¹² relative.
- **The document model** with unit tests per table kind, golden tests for the formula
  parser, recompute tests, undo/redo tests over every command, and a round-trip test: save,
  open, compare, field by field.
- **The plot engine** with golden SVGs per family and theme, PDF structure checks (header,
  cross-reference table, embedded fonts), PNG size checks (mm × dpi), and timing benchmarks.
- **The UI** with TypeScript's type checker, vitest for pure logic (keyboard navigation,
  the command registry, option schemas), and Playwright end-to-end tests on the wasm build.
- **The desktop build** launches under a virtual display in CI to prove it starts and
  renders.

`./verify` runs all of it and is the release gate: formatting, clippy with warnings as
errors, every cargo test, the wasm build, type check and unit tests, the frontend and
desktop builds, and the end-to-end suite.
