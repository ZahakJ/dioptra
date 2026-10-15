# Records for regulated labs

Part of Dioptra Lab. Dioptra supplies technical controls that support compliance with
21 CFR Part 11 and EU GMP Annex 11. Compliance also depends on the lab's procedures,
validation and storage.

## The controls

**Users and roles.** Users are kept per computer, in `lab-users.json` in the app's config
directory. Each has a user ID, a full name, a password and a role:

| Role | May |
|---|---|
| Analyst | change regulated projects, sign as author |
| Reviewer | also sign as reviewer or approver |
| Administrator | also add users, disable them and reset passwords |

Passwords are stored as PBKDF2-HMAC-SHA256 hashes (600,000 iterations, random 16-byte salt).
Five failed sign-ins in a row lock an account until an administrator resets its password.
Users are disabled, never deleted, so a user ID is never reused. Every creation, disabling,
password reset and lockout is written to the users file's own hash-chained access log.
Dioptra refuses a users file whose accounts do not match its log.

The browser version keeps users in browser storage. It is not suitable for regulated work.

**Audit trail.** *Records → Make this project regulated…* turns on the audit trail of the
open project. It cannot be turned off. From then on every change (data, formulas,
exclusions, analysis options, graphs, layouts, deletions, undo and redo) needs a signed-in
user and a reason, and is appended to `extensions/lab-audit.jsonl` inside the project file:

| Field | Content |
|---|---|
| `seq` | 1, 2, 3, … |
| `at` | time, UTC |
| `user`, `name` | user ID and full name |
| `action` | what was done, in words |
| `command` | the command, with large payloads summarised |
| `objects` | ids of the tables, analyses, graphs and layouts affected |
| `reason` | why |
| `version` | record version |
| `content` | SHA-256 of the project content after the change |
| `prev` | hash of the previous entry |
| `hash` | SHA-256 of the canonical JSON of the entry without `hash` |

Opening a regulated project checks the chain. A changed, missing or reordered entry, or
project content that differs from the last entry, is reported at once.

**Electronic signatures.** *Records → Sign…* asks for the password again, then records the
signer's name and ID, the date and time, the meaning (authored, reviewed or approved), the
SHA-256 of the project content and the hash of the trail head, as an entry of the trail. A
signed project is locked. *Records → New version…* records a reason and starts the next
version; the earlier signatures stay in the trail, marked superseded. The audit-trail
viewer and the audit report show each signature with its name, date and time, meaning and
status: valid, superseded, or content changed since signing.

**Verification.** *Records → Audit trail…* lists the entries, filters by user and object,
and verifies the chain. *Records → Export audit report…* writes it as PDF or CSV.
Verifying, viewing and exporting need no license, and a signed record stays locked when a
license ends. Setting up users, making a project regulated, signing and starting a new
version need Dioptra Lab.

## What the controls cover

| Requirement | Covered by |
|---|---|
| 11.10(a) validation | customer procedure |
| 11.10(b) accurate and complete copies | project file (open JSON), audit report (PDF, CSV) |
| 11.10(c) protection and retrieval of records | customer procedure (storage, backup, retention) |
| 11.10(d) limited system access | users and passwords; access to the computer and files is a customer procedure |
| 11.10(e) audit trail | audit trail: who, when, what, why; append-only, hash-chained; retention is a customer procedure |
| 11.10(f) operational checks | reason and sign-in required for every change; signed records locked until a new version |
| 11.10(g) authority checks | roles: only administrators manage users, only reviewers and administrators sign as reviewer or approver |
| 11.10(h) device checks | customer procedure |
| 11.10(i) training | customer procedure |
| 11.10(j) accountability for signatures | customer procedure |
| 11.10(k) systems documentation | customer procedure; this manual |
| 11.50 signature manifestation | name, date and time, meaning, in the viewer and the report |
| 11.70 signature and record linking | signature bound to the content hash and the trail head |
| 11.200(a)(1) two components | user ID and password; the password is asked again for every signature |
| 11.200(a)(2), (a)(3) genuine owner only | lockout after five failures; customer procedure |
| 11.300(a) unique IDs | IDs never reused; users disabled, not deleted |
| 11.300(b) periodic password revision | customer procedure; administrators reset passwords |
| 11.300(c) lost credentials | administrator reset, recorded in the access log |
| 11.300(d) unauthorised use | lockout, recorded in the access log; reporting is a customer procedure |
| 11.300(e) device testing | customer procedure |
| Annex 11 §9 audit trails | audit trail with reasons; PDF and CSV report; review is a customer procedure |
| Annex 11 §12.1, §12.2 access control | users and roles; physical and network security are customer procedures |
| Annex 11 §12.3 access changes recorded | access log of the users file |
| Annex 11 §12.4 identity, date and time of changes | audit trail |
| Annex 11 §14 electronic signatures | signatures in the trail: permanently linked to the record, with date and time |

## Limits of a desktop system

Dioptra runs on the lab's own computers without a server, as a closed system in the sense of
11.10. The project file and the users file are ordinary files. Anyone who can write them,
such as an operating-system administrator, can delete or replace them, including removing the
trail from a project. The hash chain uses no secret key: it makes an edit evident, not
impossible, and someone who rewrites the whole chain consistently is not detected. Keep
regulated projects and the users file on storage with access control, backups and a
retention policy, and restrict administrator rights on the computers.

Validation (IQ, OQ, PQ) and the standard operating procedures are the lab's responsibility.
The [validation pack](https://github.com/ZahakJ/dioptra/tree/main/docs/validation) supplies the
compliance matrix, traceability, IQ and OQ protocols and the automated OQ (*Records → Run OQ…*).
