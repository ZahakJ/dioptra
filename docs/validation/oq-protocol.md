# Operational qualification protocol

Verifies, on the installed system, that Dioptra's statistical engine computes accurately
(automated OQ, part A) and that the records controls work as specified (manual scripts,
part B). Execute after a passed [IQ](iq-protocol.md).

## 1. Prerequisites

- IQ passed on this computer.
- Dioptra Lab license key entered (Help → Dioptra Lab…).
- For part B: a test computer or a test account whose users file may be changed, and two
  test users created in MOQ-1. Do not use production users.
- Tester and reviewer named in the validation plan.

## 2. Part A: automated OQ (OQ-A)

Every build of Dioptra contains the reference data of the vendor test suite and the code
that compares the engine against it:

| Section | Reference | Checks | Acceptance criterion per check |
|---|---|---:|---|
| `NIST-UNI` | NIST StRD univariate, 9 sets | 54 | LRE of mean ≥ 14; SD and lag-1 autocorrelation ≥ the minimum for the set's difficulty; ≥ 13 against the exact statistics of the inputs as parsed into doubles |
| `NIST-LIN` | NIST StRD linear regression, 11 sets | 25 | Minimum LRE over parameters, SDs, R², residual SD and ANOVA table ≥ 12 or 13 per set |
| `NIST-ANOVA` | NIST StRD one-way ANOVA, 11 sets | 22 | Minimum LRE over SS, MS, F, R², residual SD ≥ the bar per set (3.9 to 13; the higher sets are capped by input rounding) |
| `NIST-NL` | NIST StRD nonlinear regression, 27 sets, both starting points | 270 | Converges; LRE of parameters ≥ 9; SDs, residual SS and residual SD ≥ 9 (Lanczos1 ≥ 2.5) |
| `ORC-*` | 28 families of oracle fixtures: SciPy, statsmodels, lifelines, scikit-learn, 32-digit mpmath | 7,841 | \|got − expected\| ≤ abs or ≤ rel × \|expected\|, with the tolerance stored with each fixture case |
| | **Total** | **8,212** | |

LRE is the log relative error (McCullough 1998), the number of correct significant digits.
The per-set bars and their reasons are in [CORRECTNESS.md](../CORRECTNESS.md) and
`crates/dioptra-stats/tests/nist_*.rs`. The OQ makes exactly the comparisons of the vendor
test suite; a test of the OQ itself checks that its counts match the family table of
CORRECTNESS.md.

### Execution

Desktop app:

1. Records → Run OQ…. A progress bar shows each step. A run takes seconds to about a minute,
   depending on the computer.
2. Read the result line: **Pass** or **Fail**, checks passed of checks run.
3. Record Build, System, Fixture set and Report SHA-256 from the dialog.
4. Sign in first (Records → Sign in…) to sign the report as executed. Export PDF… asks for
   the password and records name, user ID, UTC time and the meaning "executed" on the PDF.
   Without a signed-in user, the PDF carries a manual signature block. Export CSV writes
   every check, one per row.
5. Print or file the PDF; keep the CSV with it. Both carry the same report SHA-256.

Command line: `dioptra oq` runs the same checks in the installed command-line tool.

### Acceptance criteria

- Result **Pass**: every check meets its criterion, and the total is 8,212.
- Build version and target match the IQ record; the fixture-set SHA-256 matches the value
  recorded at IQ (identical for every installation of one version).
- The PDF is signed as executed (electronically or by hand) and reviewed by QA.

### Report contents

Product and version, build target triple, operating system and architecture, UTC date and
time, the SHA-256 of the embedded fixture set, totals and run time, a table of sections
with checks, failures and time, every NIST check with the LRE achieved and required, every
failed check with its reason, and the signature blocks. The CSV adds every oracle
comparison: ID, section, reference, quantity, expected, got, tolerance, result.

## 3. Part B: manual test scripts for the records controls

Record the actual result of every step and pass or fail. Use screenshots where the
validation plan requires them. Test users: `qa-admin` (administrator, created first),
`qa-analyst` (analyst).

### MOQ-1: users, roles and lockout

| # | Step | Expected result |
|---|---|---|
| 1 | Records → Set up users…: create `qa-admin` with a full name and an 8-character password. | `qa-admin` is created as administrator and signed in. |
| 2 | Records → Set up users… → Add user…: create `qa-analyst`, role Analyst. | Listed as Analyst, Active. |
| 3 | Add another user with ID `QA-ANALYST`. | Refused: the ID exists already. |
| 4 | Add a user with a 7-character password. | Refused: at least 8 characters. |
| 5 | Sign out; sign in as `qa-analyst`. Open Records → Set up users…. | The list shows; there are no Add, Disable or Reset buttons. |
| 6 | Sign out. Sign in as `qa-analyst` with a wrong password five times. | Attempts 1–4 report the attempts left; the fifth locks the account. |
| 7 | Sign in as `qa-analyst` with the correct password. | Refused: the account is locked. |
| 8 | Sign in as `qa-admin`; open Users; Access log. | Entries for both creations and the lock, with time and actor. |
| 9 | Reset and unlock `qa-analyst` with a new password. Sign in as `qa-analyst`. | Sign-in succeeds; the access log records the reset. |
| 10 | As `qa-admin`, disable a third test user. | Listed as Disabled; its ID cannot be added again; sign-in with it is refused. |

### MOQ-2: reason required and audit trail

| # | Step | Expected result |
|---|---|---|
| 1 | Signed in as `qa-analyst`, create a project with one data table and enter values. | — |
| 2 | Records → Make this project regulated…, reason "MOQ-2". | Status line: Regulated, version 1. |
| 3 | Change a cell. Cancel the reason dialog. | The cell is not changed; a notice says a reason is needed. |
| 4 | Change the cell again; enter the reason "MOQ-2 step 4". | The cell changes. |
| 5 | Edit → Undo; give a reason. | Undone; the change needs a reason too. |
| 6 | Sign out. Change a cell; cancel the sign-in dialog. | Not changed: nobody is signed in. |
| 7 | Sign in. Records → Audit trail…. | Entries for steps 2, 4 and 5: time (UTC), user and name, action, objects, reason, version. Chain intact. |
| 8 | Check every menu (Records, File, Edit) for a command that turns the trail off; run Records → Make this project regulated… again. | No such command; making it regulated again is refused or disabled. |

### MOQ-3: signing locks the record

| # | Step | Expected result |
|---|---|---|
| 1 | On the project of MOQ-2, as `qa-analyst`, Records → Sign…. | Only the meaning Authored is offered. |
| 2 | Enter a wrong password. | Refused; it counts towards the lockout. |
| 3 | Enter the correct password. | Signed: Authored by the full name; the project is locked. |
| 4 | Change a cell. | Refused: the project is signed and locked. |
| 5 | Sign in as `qa-admin`; Records → Sign… as Approved. | Signed; the trail shows both signatures, each with name, UTC time and meaning, status Valid. |
| 6 | Records → New version…, reason "MOQ-3 step 6". Change a cell with a reason. | The change succeeds; version 2. In the trail, the signatures of version 1 are Superseded. |
| 7 | Records → Export audit report… as PDF and as CSV. | The PDF shows chain status, signatures with status and every entry; the CSV holds every field of every entry. |

### MOQ-4: verification detects tampering

A `.dioptra` file is a zip archive; the trail is `extensions/lab-audit.jsonl` inside it
([FILE-FORMAT.md](../FILE-FORMAT.md)). Use a copy of the MOQ-3 project for each step and a
zip tool that rewrites one member.

| # | Step | Expected result |
|---|---|---|
| 1 | Save the MOQ-3 project; close and open it. | Opens without a warning; Audit trail: chain intact. |
| 2 | Copy A: in `lab-audit.jsonl`, change one character of a reason. Open copy A. | Warning: the audit trail is broken at that entry ("the entry was changed after it was written"). |
| 3 | Copy B: delete one line other than the last. Open copy B. | Warning: broken at the following entry (an entry before it is missing, or out of order). |
| 4 | Copy C: swap two lines. Open copy C. | Warning: broken at the first swapped entry. |
| 5 | Copy D: in `tables/<id>.json`, change one data value; leave the trail as it is. Open copy D. | Warning: the project was changed outside its audit trail. |
| 6 | Copy E: change one character of a signature entry. Open copy E; Records → Audit trail…. | Warning: broken at that entry. The record cannot be signed until restored. |
| 7 | Remove the license key (Help → Dioptra Lab…). Open copy A and the original. | Verification still runs: copy A shows the warning; the original verifies intact and stays locked where signed. |

## 4. Acceptance

The OQ passes when OQ-A passes and every step of MOQ-1 to MOQ-4 meets its expected result,
or every deviation is closed as in section 5.

## 5. Deviations

A deviation is any result that differs from the expected result, any step that cannot be
executed, and any change to this protocol during execution.

1. Stop the affected test; record the deviation with an ID, the step, the actual result and
   evidence (screenshot, exported report, CSV).
2. Classify it by the customer's procedure (for example critical, major, minor) and assess
   the impact on intended use.
3. For an automated OQ failure, export the CSV, rerun once to exclude an interrupted run, and
   report the failing check IDs to the vendor (issue tracker or contact in
   [README.md](README.md)) with the CSV, Build, System and fixture-set SHA-256. A failed
   check is not overridden locally: the version is not released for use, or the affected
   method is excluded from intended use by a documented, QA-approved decision.
4. Close each deviation with a root cause, corrective action and QA approval before the
   OQ is approved. Repeat the affected steps after a correction.
