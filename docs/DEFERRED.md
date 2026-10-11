# Deferred from v0.1

Each item says why it waits and what would unblock it.

| Item | Why deferred | Unblocked by |
|---|---|---|
| WebGL/canvas rendering for > 10⁶ points | The engine's SVG with level-of-detail decimation covers v0.1 data sizes; a second renderer doubles the surface to keep in sync. | A profiling case from real use. |
| Hosted web app | The wasm build exists and runs the e2e suite; hosting adds auth, storage and support burden. | Owner decision. |
| Mixed-effects models, nested ANOVA inference | Need REML and a model-formula layer; nested tables ship with descriptives and plotting. | Phase "statistics 2" after v0.1 feedback. |
| Multiple logistic regression, Cox regression | Cox hazard ratio ships via Mantel–Haenszel; full regression needs IRLS/Newton infrastructure already planned for v0.2. | v0.2. |
| Three-way ANOVA, two-way repeated measures | Two-way with GG correction on one factor is the v0.1 ceiling. | v0.2. |
| Prism `.pzfx` write | Read is enough to escape; writing a proprietary format is not a goal. | Never, unless requested. |
| Scripting (Python/Lua) | The formula language and the CLI cover reproducibility; a scripting surface needs API stability. | v0.3. |
| Collaboration / cloud sync | The file is diff-friendly for git; realtime sync is out of scope. | — |
| Plugin system | Premature before the analysis API settles. | v0.3. |
| System font picker in figures | Bundled Inter keeps output identical across OS; system fonts are listed but not embedded in PDF yet. | Font subsetting of arbitrary system fonts via svg2pdf is supported; UI is the missing part. |
| Localised UI | English only. | Translators. |
