# 00 — Name and technical stack

## Name: Dioptra

Greek διόπτρα, the sighting instrument described by Hero of Alexandria (1st c. AD) for
surveying and astronomy: a graduated disc, a sighting bar, levelling screws. It measures
angles precisely and lets a surveyor lay out straight lines across uneven ground. That is the
promise here: an instrument for seeing data straight. It sits next to the owner's other
instrument names (Astrolabe, Gnomon, Horologion).

### Conflicts checked (2026-10-11)

- `ZahakJ/dioptra` is free on GitHub.
- **NIST "Dioptra"** (https://pages.nist.gov/dioptra/): a test platform for assessing the
  trustworthiness of machine-learning systems (adversarial robustness experiments). Niche,
  government, different domain, no desktop app, no overlap in audience with bench
  scientists. Noted because Dioptra cites NIST StRD everywhere; the two are unrelated.
- **Dioptas** (https://pypi.org/project/dioptas/): a Python GUI for 2-D X-ray diffraction
  data at synchrotrons. Different spelling, different field.
- No statistics, graphing or spreadsheet product uses the name. No trademark in software
  class found in a plain web search.

Decision: keep **Dioptra**. The NIST tool is not well-known software in the sense of the
brief (it is a research testbed with a handful of GitHub stars outside its programme), and
the two names will never appear in the same search context for a biologist. Recorded here so
the decision is auditable.

## Stack decisions

| Layer | Choice | Why |
|---|---|---|
| Language | Rust 2021, toolchain pinned 1.99.0 | Same engine compiles to native (Tauri) and `wasm32-unknown-unknown` (browser build, e2e tests). |
| Special functions | own implementations in `dioptra-stats::special`, with `libm` (pure-Rust musl port) for `erf`, `erfc`, `lgamma`, `tgamma` | The regularised incomplete beta/gamma, their inverses, the studentised range and non-central t are the backbone of every p-value; owning them means every digit is testable against the oracle table. `statrs` was considered and rejected for the core path: its inverse CDFs are bisection-based and its API churns; `puruspe` covers only a subset. Both remain available as cross-checks in tests. |
| Linear algebra | `nalgebra` | Mature, `no_std`/wasm-friendly, has QR, SVD, Cholesky and symmetric eigendecomposition. Needed for linear regression, LM (QR of the Jacobian), PCA. |
| Nonlinear regression | own Levenberg-Marquardt (Moré 1978 scaling, MINPACK `lmder` structure) with numerical or analytic Jacobians, parameter constraints by transformation, robust (Lorentzian) loss for ROUT | `argmin` exists but its LM is thin and pulls heavy generic machinery; NIST StRD gives us 27 certified problems to prove our own. |
| Plot rendering | own scene graph → own SVG writer; `usvg`/`resvg` for PNG raster; `svg2pdf` (text feature, font subsetting via `subsetter`) for PDF | One scene, three outputs, all from Rust; the SVG shown in the UI *is* the export. Fonts bundled (Inter, OFL) so output is identical on every OS and in wasm. |
| Import | `csv` (TSV/CSV), `calamine` (xlsx/xls/ods read), `quick-xml` (Prism `.pzfx`), clipboard text via the UI | All pure Rust, wasm-compatible. |
| Export | `csv`, `rust_xlsxwriter`, own project writer | `rust_xlsxwriter` is pure Rust and documents wasm support. |
| Project file | `.dioptra` = zip (deflate) of pretty-printed JSON with stable key order; tables stored columnar | Inspectable with `unzip`, diffable after extraction, no Arrow/Parquet dependency in the wasm build; see `docs/FILE-FORMAT.md`. SQLite rejected: not diffable, heavy in wasm. Arrow rejected: binary, large dependency, no benefit at Prism-scale data (a 1,000,000-cell table is 8 MB as f64 and ~4 MB deflated JSON). |
| Desktop shell | Tauri 2 (webkit2gtk on Linux, WebView2 on Windows, WKWebView on macOS) | The brief; the owner's Loci project already ships this way with a known-good `tauri-action` release workflow. |
| UI | TypeScript, React 19, zustand, TanStack Virtual, Vite 8, vitest, Playwright (system Chrome) | Known-good in this workspace; virtualised grid with both-axis virtualisation is the critical piece. |
| Bridge | one JSON `Command` → `Response` dispatcher exposed both as a Tauri command and as a wasm-bindgen export | The UI never knows which backend it runs on; e2e runs the real engine in the browser. |
| Serialisation | `serde`, `serde_json` with `float_roundtrip` + `preserve_order` | Round-trip exactness for data and stable diffs. |

Versions resolved on crates.io on 2026-10-11: nalgebra 0.35, libm 0.2, usvg/resvg 0.48, svg2pdf 0.13, fontdb 0.24, calamine 0.36, rust_xlsxwriter 0.99, zip 9, csv 1.4, quick-xml 0.42, wasm-bindgen 0.2.129 (CLI installed locally at the same version; the crate is pinned `=0.2.129`).
