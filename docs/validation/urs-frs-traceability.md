# Requirements traceability

User requirements (URS) of a typical regulated laboratory, the Dioptra functions that meet
them (FRS), and the tests that verify each function. Adopt, edit or extend the URS for your
intended use; keep the trace to the FRS and tests.

Test references:

- **Vendor tests** are Rust tests run on every commit and before every release
  (`./verify`). `records.rs::name` is a test in `crates/dioptra-lab/tests/records.rs`;
  `users.rs::name`, `audit.rs::name` are unit tests in `crates/dioptra-lab/src/`;
  `oq.rs::name` is in `crates/dioptra-lab/tests/oq.rs`.
- **OQ checks** run in the installed build (Records → Run OQ…, or `dioptra oq`). Each check
  ID starts with its section code: `NIST-UNI-*`, `NIST-LIN-*`, `NIST-ANOVA-*`, `NIST-NL-*`
  for the NIST StRD sets, `ORC-<family>-NNNN` for the oracle comparisons.
- **Manual OQ** scripts are in [oq-protocol.md](oq-protocol.md) (`MOQ-n`).

## User requirements

| ID | Requirement |
|---|---|
| URS-01 | Statistical results are numerically accurate, and the accuracy is demonstrated on the installed system. |
| URS-02 | Only identified, authorized individuals change regulated records; identities are unique and protected against misuse. |
| URS-03 | Access rights are managed by designated administrators, and every change to access is recorded. |
| URS-04 | Every change to a regulated record is recorded with who, when, what and the affected objects. |
| URS-05 | Every change to a regulated record has a documented reason, and the audit trail cannot be turned off. |
| URS-06 | Records are signed electronically with the signer's name, date and time and meaning, the password confirmed at signing, and the signature bound to the record. |
| URS-07 | A signed record cannot be changed; a change starts a new version that is signed again, keeping the earlier signatures. |
| URS-08 | Controls on regulated records stay in force when the software license ends. |
| URS-09 | Alteration, deletion or reordering of audit-trail entries, and changes to a record outside the audit trail, are detected. |
| URS-10 | Records and their audit trail are available as accurate, complete, human-readable and electronic copies. |
| URS-11 | Execution of the qualification is documented in a report identifying the build, the system, the date, every check and its result, signed by the executor. |

## Functional specifications and tests

| URS | FRS | Function | Vendor tests | OQ |
|---|---|---|---|---|
| URS-01 | FRS-01.1 | Mean, SD and lag-1 autocorrelation meet the minimum LRE per NIST difficulty class on the 9 univariate StRD sets. | `dioptra-stats` `nist_univariate_lre`; `oq.rs::oq_runs_and_every_check_passes` | `NIST-UNI-*` |
| URS-01 | FRS-01.2 | Linear regression parameters, SDs, R², residual SD and ANOVA table meet the minimum LRE on the 11 linear StRD sets. | `nist_linear_lre`, `nist_simple_path` | `NIST-LIN-*` |
| URS-01 | FRS-01.3 | One-way ANOVA SS, MS, F, R² and residual SD meet the minimum LRE on the 11 ANOVA StRD sets. | `nist_anova_lre` | `NIST-ANOVA-*` |
| URS-01 | FRS-01.4 | Nonlinear fits converge from both certified starting points and meet the minimum LRE on the 27 nonlinear StRD sets. | `nist_nonlinear_lre` | `NIST-NL-*` |
| URS-01 | FRS-01.5 | Distributions, tests, post hoc comparisons, regression, survival, ROC, agreement and PCA agree with SciPy, statsmodels, lifelines, scikit-learn and mpmath references within each case's stated tolerance (7,841 comparisons, 28 families). | `dioptra-stats` fixture tests (`tests/*.rs`) | `ORC-*` |
| URS-02 | FRS-02.1 | A change to a regulated project is refused unless a user is signed in. | `records.rs::signed_out_users_cannot_change` | MOQ-2 |
| URS-02 | FRS-02.2 | Passwords are stored as PBKDF2-HMAC-SHA256 hashes with a random salt and compared in constant time; at least 8 characters. | `users.rs::passwords_hash_and_verify` | — |
| URS-02 | FRS-02.3 | Five failed sign-ins or signature attempts in a row lock the account; the lock is recorded in the access log. | `users.rs::lockout_after_five_failures`, `records.rs::lockout_through_the_engine` | MOQ-1 |
| URS-02 | FRS-02.4 | User IDs are unique and never reused; users are disabled, not deleted. | `users.rs::only_admins_manage_users` | MOQ-1 |
| URS-03 | FRS-03.1 | Only administrators add, disable and reset users. | `users.rs::only_admins_manage_users` | MOQ-1 |
| URS-03 | FRS-03.2 | Creation, disabling, reset and lockout are written to a hash-chained access log; a users file whose accounts differ from its log is refused. | `users.rs::reset_unlocks_and_edits_outside_show`, `records.rs::the_users_file_round_trips_through_the_shell` | MOQ-1 |
| URS-04 | FRS-04.1 | Each change appends an entry with UTC time, user ID and name, action, command, objects, reason, version and the SHA-256 of the content after it. | `records.rs::changes_need_a_reason_and_are_chained`, `audit.rs::reads_and_changes` | MOQ-2 |
| URS-04 | FRS-04.2 | The trail is saved inside the project file and survives save and open. | `records.rs::save_and_open_keep_the_trail` | MOQ-4 |
| URS-05 | FRS-05.1 | A change without a reason is refused. | `records.rs::changes_need_a_reason_and_are_chained` | MOQ-2 |
| URS-05 | FRS-05.2 | The audit trail cannot be turned off once on: no command or menu item turns it off, and making a regulated project regulated again is refused. | — | MOQ-2 |
| URS-06 | FRS-06.1 | Signing asks for the password again and records name, ID, UTC time, meaning, content hash and trail head as a trail entry. | `records.rs::signing_locks_and_a_new_version_unlocks` | MOQ-3 |
| URS-06 | FRS-06.2 | Analysts sign only as author; reviewer and approver need the reviewer or administrator role. | `records.rs::analysts_sign_only_as_author` | MOQ-3 |
| URS-07 | FRS-07.1 | A signed project refuses every change until a new version is started with a reason; earlier signatures show as superseded. | `records.rs::signing_locks_and_a_new_version_unlocks` | MOQ-3 |
| URS-08 | FRS-08.1 | Verification, viewing and export need no license; locks and the reason requirement hold with the license lapsed. Regulating and signing need the license. | `records.rs::verification_and_locks_work_with_lab_lapsed`, `records.rs::regulating_and_signing_need_the_license` | — |
| URS-09 | FRS-09.1 | Opening or verifying a project detects a changed, deleted or reordered entry, and content changed outside the trail, and names the first failing entry. | `records.rs::tampering_shows` | MOQ-4 |
| URS-09 | FRS-09.2 | A signature whose content changed shows as "content changed since signing". | `records.rs::tampering_shows` | MOQ-4 |
| URS-10 | FRS-10.1 | The audit report exports as PDF (chain status, signatures, entries) and CSV (every field). | `records.rs::audit_report_pdf` | MOQ-4 |
| URS-11 | FRS-11.1 | The automated OQ reports build version, target triple, operating system, UTC date, the SHA-256 of the embedded fixture set, each check (ID, reference, expected, got, tolerance, result) and totals, as PDF and CSV. | `oq.rs::oq_runs_and_every_check_passes` | OQ-A |
| URS-11 | FRS-11.2 | The OQ PDF carries an execution signature (name, ID, UTC time, meaning "executed") after the signed-in user's password is confirmed; without a signed-in user it carries a manual signature block. A wrong password counts towards the lockout. | `oq.rs::oq_runs_and_every_check_passes` | OQ-A |
| URS-11 | FRS-11.3 | Running the OQ does not change an open regulated project and adds nothing to its trail. | `oq.rs::oq_is_not_a_change_to_a_regulated_project` | — |
