# Supplier assessment

Answers to the questions of a typical supplier questionnaire for GxP software. Every claim
can be checked in the public repository, `https://github.com/ZahakJ/dioptra`.

## 1. Company and product

| Question | Answer |
|---|---|
| Supplier | Dioptra (ZahakJ), copyright holder of the source code. |
| Product | Dioptra: desktop, web and command-line statistics and graphing software. Dioptra Lab: licensed features for laboratories, including the records controls and this validation pack. |
| Licenses | Core: MIT (`LICENSE`). Lab: Dioptra Lab License 1.0 (`LICENSE-LAB`). |
| Intended use | Statistical analysis and graphing of laboratory data. Dioptra is not a medical device and is not intended for diagnosis. |
| Regulated-use claims | Dioptra supplies technical controls that support compliance with 21 CFR Part 11 and EU GMP Annex 11. Compliance of a regulated process is the customer's responsibility. |

## 2. Quality management

| Question | Answer |
|---|---|
| Certified QMS (ISO 9001, ISO 13485)? | None is claimed. Development follows the documented process below; its records (commits, reviews, test results, releases) are public. |
| Documented development process? | Yes: requirements and design notes in `docs/` and `research/`, the release gate `./verify`, the release workflow, and the changelog. |
| Document control? | Documentation is versioned with the source, reviewed through the same commits, and released with each version. |
| Personnel qualification | Developers' qualifications are not certified. Numerical methods are documented with their references ([METHODS.md](../METHODS.md)) and verified against independent references (section 4). |
| Audits | The source, history, tests and build pipeline are public and can be reviewed without a site visit. On-site or remote audits by arrangement with the vendor contact. |

## 3. Software development life cycle

| Question | Answer |
|---|---|
| Model | Iterative. Each change is a commit in a public Git repository; each release is a tag. |
| Requirements and design | Specifications per area in `docs/` (analyses, graphs, file format, formulas, Lab tier) and research notes in `research/`. Requirements for the records controls: [urs-frs-traceability.md](urs-frs-traceability.md). |
| Source control | Git, hosted on GitHub. Full history public. |
| Change review | Contributions use a pull-request template recording what changed, how it was verified (release-gate output; for numerical changes the fixture or reference) and whether tests and method documentation were updated. Every change must pass the release gate before release. |
| Static analysis | Rust: `rustc` with `#![forbid(unsafe_code)]` in the Lab crate, clippy with warnings as errors. TypeScript: strict type checking. |
| Build | Reproducible from the tagged source with pinned toolchains (Rust 1.99.0, Node 26, pnpm 11, wasm-bindgen 0.2.129) and lock files (`Cargo.lock`, `pnpm-lock.yaml`). Release installers are built by GitHub Actions from the tag, not on a developer's machine. |
| Third-party components | Declared in `Cargo.lock` and `pnpm-lock.yaml`, pinned by version and checksum. |
| Security | License keys are signed statements verified offline against a public key compiled into the app; no license server is contacted. Passwords are stored as PBKDF2-HMAC-SHA256 hashes. |

## 4. Testing

| Question | Answer |
|---|---|
| Unit and integration tests | Rust tests per crate (statistics engine, core, plotting, Lab, CLI), TypeScript unit tests (Vitest), end-to-end browser tests (Playwright). |
| Numerical verification | All 58 NIST StRD data sets, scored by log relative error; 7,841 comparisons in 28 families against SciPy, statsmodels, lifelines, scikit-learn and 32-digit mpmath, each within a stated tolerance. Results: [CORRECTNESS.md](../CORRECTNESS.md). The oracle generators cross-check libraries against each other; see `research/05-nist-and-oracle-notes.md`. |
| When are tests run? | On every push (continuous integration) and before every release (release gate). A release with a failing test is not tagged. |
| Test evidence for the customer | The automated OQ runs the NIST and oracle comparisons inside the installed build and writes a signed report ([oq-protocol.md](oq-protocol.md)). Records controls: vendor tests in `crates/dioptra-lab/tests/records.rs` and manual scripts MOQ-1 to MOQ-4. |
| Regression testing | The full suite runs on every change; graph output is checked against stored snapshots. |

## 5. Defects and changes

| Question | Answer |
|---|---|
| How are defects reported? | GitHub issues, with templates for bug reports and numerical discrepancies; or the vendor contact. |
| How are defects handled? | Each report is reproduced, classified by impact (numerical result, records control, other), fixed with a test that fails before the fix, and released in a new version. Numerical discrepancies are investigated against an independent reference; the outcome is recorded in the issue and, when results change, in CORRECTNESS.md and the changelog. |
| Are customers notified? | Release notes in `CHANGELOG.md` and on the GitHub release page. Defects affecting numerical results or records controls are stated as such. Notification to Lab customers on request through the vendor contact. |
| Change control | [change-control.md](change-control.md). |
| Known defects | Open issues on GitHub. |

## 6. Source code and continuity

| Question | Answer |
|---|---|
| Is the source available to the customer? | Yes, all of it. The core is MIT-licensed. The Lab code, including the records controls and the automated OQ, is readable under `LICENSE-LAB`, which permits reading, copying and modifying it to study, audit and validate it. |
| Source escrow | Not needed: the complete source, build scripts and history of every released version are public. |
| If the supplier stops trading | The software keeps running offline; license keys are verified without a server. The file format is documented and open ([FILE-FORMAT.md](../FILE-FORMAT.md)); records and their audit trails verify without a license. |
| Data portability | Project files are zip archives of JSON. Tables export to CSV, TSV, XLSX and Prism `.pzfx`; audit trails to PDF and CSV. |

## 7. Support and training

| Question | Answer |
|---|---|
| Support | Dioptra Lab: e-mail support with a reply within two working days; numerical questions answered with a reference. All users: GitHub issues. |
| Documentation | User manual (in the app's Help menu and online), this validation pack, release notes. |
| Training | The manual, including the chapter on [records for regulated labs](../manual/src/regulated.md). Customer training is the customer's responsibility. |
| Validation services | The validation pack and automated OQ. Support with OQ execution under a site license. |

## 8. Contact

**Contact (to be completed by the vendor before issue):** name, e-mail address and
postal address for quality, validation and audit questions.
