# Compliance matrix

21 CFR Part 11 and EU GMP Annex 11, clause by clause. Each row names the Dioptra control
that supports the requirement, the customer procedure it depends on, or why it does not
apply. Controls are described in the manual chapter
[Records for regulated labs](../manual/src/regulated.md) ("Regulated" below); requirement
and test IDs refer to [urs-frs-traceability.md](urs-frs-traceability.md).

Dioptra is a desktop application without a server. It is a closed system in the sense of
§11.3(b)(4) when the customer controls access to the computers and the storage holding the
records. Records are project files (`.dioptra`); the users file (`lab-users.json`) is
kept per computer.

Legend: **D** Dioptra control. **C** customer procedure. **N/A** not applicable.

## 21 CFR Part 11

| Clause | Requirement | Type | Dioptra control / customer procedure | Ref. |
|---|---|---|---|---|
| 11.10(a) | Validation: accuracy, reliability, consistent intended performance, ability to discern invalid or altered records | D, C | Automated OQ in every build (NIST StRD, oracle comparisons); hash-chained audit trail verified on open, detecting altered records. Validation is the customer's. | URS-01, URS-09; [oq-protocol.md](oq-protocol.md) |
| 11.10(b) | Accurate and complete copies, human readable and electronic | D | Project file in a documented open format ([FILE-FORMAT.md](../FILE-FORMAT.md)) holding data, analyses, graphs and the trail; audit report as PDF and CSV; project report as PDF or DOCX. | URS-10 |
| 11.10(c) | Protection of records for accurate and ready retrieval through the retention period | C, D | Storage, backup and retention are customer procedures. The trail and signatures stay verifiable without a license, so archived records remain readable. | URS-10 |
| 11.10(d) | Limiting system access to authorized individuals | D, C | Named users with passwords; a change to a regulated project needs a signed-in user. Access to the computer, the operating-system account and the files is a customer procedure. | URS-02 |
| 11.10(e) | Secure, computer-generated, time-stamped audit trail of operator entries and actions that create, modify or delete records; previous values not obscured; retained and available for review and copying | D, C | Append-only trail inside the project file: UTC time, user, action, command, objects, reason, version, content hash, chained by SHA-256. Undo and redo are recorded as changes. Previous states are kept in the trail and in the command history. Retention of the files is a customer procedure. | URS-04, URS-05 |
| 11.10(f) | Operational system checks to enforce permitted sequencing of steps and events | D | A change needs a signed-in user and a reason; a signed version is locked until a new version is started with a reason. | URS-05, URS-07 |
| 11.10(g) | Authority checks | D, C | Roles: only administrators manage users; only reviewers and administrators sign as reviewer or approver. Assigning roles is a customer procedure. | URS-03 |
| 11.10(h) | Device checks for the validity of data input sources | C | Instrument files and their transfer are outside Dioptra. Imported files are shown before they become a table; the change is recorded in the trail. | — |
| 11.10(i) | Persons who develop, maintain or use the system have the education, training and experience | C, D | Customer training records for users and administrators. Vendor: see [supplier-assessment.md](supplier-assessment.md). | — |
| 11.10(j) | Written policies holding individuals accountable for actions under their electronic signatures | C | Customer SOP. | — |
| 11.10(k) | Controls over systems documentation: distribution, access, use, revision and change control | C, D | The manual and this pack are versioned with the source and released with each version ([change-control.md](change-control.md)). Controlled copies are a customer procedure. | — |
| 11.30 | Controls for open systems | N/A | Dioptra does not transmit records over a network. When the customer sends project files over an open network, the customer applies its own controls (encryption, signatures). | — |
| 11.50(a) | Signature manifestation: printed name, date and time, meaning | D | Each signature records full name, user ID, UTC date and time and meaning (authored, reviewed, approved); shown in the audit-trail viewer and the audit report. The OQ report carries an execution signature with the same elements. | URS-06 |
| 11.50(b) | Manifestation subject to the same controls as records and included in human-readable displays and printouts | D | Signatures are entries of the hash-chained trail; the viewer and the PDF and CSV reports show them with their status (valid, superseded, content changed since signing). | URS-06, URS-09 |
| 11.70 | Signatures linked to their records so they cannot be excised, copied or transferred to falsify a record | D | A signature holds the SHA-256 of the project content and the hash of the trail head; a copied or edited signature fails verification. The chain has no secret key: a complete, consistent rewrite of the trail is not detected (Regulated, "Limits of a desktop system"); file access control is a customer procedure. | URS-06, URS-09 |
| 11.100(a) | Each electronic signature unique to one individual, not reused or reassigned | D | User IDs are unique and never reused; users are disabled, never deleted. | URS-02 |
| 11.100(b) | Verify the identity of the individual before assigning a signature | C | Customer procedure for issuing accounts. | — |
| 11.100(c) | Certification to FDA of the intent that electronic signatures are the legally binding equivalent of handwritten signatures | C | Customer letter to FDA. | — |
| 11.200(a)(1) | Two distinct identification components (ID and password); first signing in a session uses both, later signings at least the password | D | Sign-in takes user ID and password; every signature asks for the password again. | URS-02, URS-06 |
| 11.200(a)(1)(ii) | Signings outside a continuous session use both components | D | Signing needs a signed-in user (ID and password) and the password again. | URS-06 |
| 11.200(a)(2) | Used only by their genuine owners | C, D | Customer SOP (no sharing of passwords). Lockout after five failed attempts. | URS-02 |
| 11.200(a)(3) | Use by anyone other than the owner requires collaboration of two or more individuals | D, C | Passwords are stored as PBKDF2-HMAC-SHA256 hashes (600,000 iterations, random salt); no one, administrators included, can read them. An administrator reset is recorded in the access log. | URS-02 |
| 11.200(b) | Biometric signatures | N/A | Dioptra has no biometric signatures. | — |
| 11.300(a) | Uniqueness of each ID and password combination | D | IDs are unique and never reused. | URS-02 |
| 11.300(b) | Periodic checking, recall or revision of IDs and passwords | C, D | Customer procedure; administrators reset passwords and disable accounts, recorded in the access log. | URS-03 |
| 11.300(c) | Loss management: deauthorize lost credentials, issue replacements | C, D | Administrator reset and disabling, recorded in the access log. | URS-03 |
| 11.300(d) | Safeguards against unauthorized use; reporting of attempts | D, C | Lockout after five failed sign-ins or signature attempts, recorded in the access log. Reporting to management is a customer procedure. | URS-02 |
| 11.300(e) | Testing of devices that bear or generate ID or password information | N/A | Dioptra uses no tokens or cards. | — |

## EU GMP Annex 11

| Section | Requirement | Type | Dioptra control / customer procedure | Ref. |
|---|---|---|---|---|
| Principle | Application validated; IT infrastructure qualified | C, D | Validation pack; IQ and OQ protocols. | [README.md](README.md) |
| §1 | Risk management throughout the life cycle | C | Customer risk assessment; inputs from this pack. | — |
| §2 | Personnel: cooperation, qualifications, access levels, responsibilities | C | Customer procedure. | — |
| §3.1 | Formal agreements with suppliers and service providers | C | Customer agreement with the vendor (license terms; support terms on request). | [supplier-assessment.md](supplier-assessment.md) |
| §3.2 | Competence and reliability of the supplier; need for an audit based on risk | C, D | Supplier questionnaire answered; the source code is public for review. | [supplier-assessment.md](supplier-assessment.md) |
| §3.3 | Documentation of commercial off-the-shelf products reviewed | C, D | Manual, this pack, CHANGELOG.md, public source and test suite. | — |
| §3.4 | Quality system and audit information available to inspectors | C, D | Supplier assessment and public development history. | — |
| §4.1 | Validation documentation covers the life cycle | C | Customer validation plan and report. | — |
| §4.2 | Change control records and deviation reports | C, D | Customer change control; vendor release notes. | [change-control.md](change-control.md) |
| §4.3 | Up-to-date listing of relevant systems and their GMP functionality | C | Customer inventory. | — |
| §4.4 | User requirements specifications, traceable through the life cycle | C, D | Template requirements and traceability supplied. | [urs-frs-traceability.md](urs-frs-traceability.md) |
| §4.5 | System developed under an appropriate quality management system | D | Development process, testing and release gate described in the supplier assessment. | [supplier-assessment.md](supplier-assessment.md) |
| §4.6 | Customized systems: formal assessment and reporting of quality and performance measures | N/A | Dioptra is not customized per customer. | — |
| §4.7 | Evidence of appropriate test methods and scenarios; parameter limits, data limits, error handling | D, C | Automated OQ with stated acceptance criteria per check; manual OQ scripts for the records controls; vendor test suite. | [oq-protocol.md](oq-protocol.md) |
| §4.8 | Data transfer or migration: value and meaning unchanged | C, D | File format versioned and migrated on open ([FILE-FORMAT.md](../FILE-FORMAT.md)). Migration of data from other systems is verified by the customer. | — |
| §5 | Built-in checks for correct and secure entry and processing of data exchanged electronically | D, C | Each trail entry holds the SHA-256 of the project content; content changed outside the trail is reported when the file is opened. Transfers from instruments are verified by the customer. | URS-09 |
| §6 | Accuracy checks for critical data entered manually | C, D | Second-person check is a customer procedure. The trail lists each entry with user, time and reason for review; signatures record review and approval. | URS-04, URS-06 |
| §7.1 | Data secured against damage; accessibility, readability and accuracy checked | C, D | Storage is a customer procedure. Verification of the chain and content on every open; verification needs no license. | URS-09 |
| §7.2 | Regular backups; integrity and accuracy of backup and restore checked | C | Customer procedure. A restored file verifies with Records → Audit trail…. | — |
| §8.1 | Clear printed copies of electronically stored data | D | Audit report (PDF), project report (PDF, DOCX), graph export. | URS-10 |
| §8.2 | Printouts indicate whether data were changed since the original entry | D | The audit report lists every change after the trail was turned on, with the chain and signature status. | URS-10 |
| §9 | Audit trail of GMP-relevant changes and deletions, with reasons; available, convertible to readable form, regularly reviewed | D, C | Trail with a reason for every change and deletion; viewer, PDF and CSV report. Review is a customer procedure. | URS-04, URS-05, URS-10 |
| §10 | Changes to the system made in a controlled manner | C, D | Customer change control; vendor versioning and release notes. | [change-control.md](change-control.md) |
| §11 | Periodic evaluation | C | Customer procedure; the automated OQ can be re-run at each evaluation. | [pq-guidance.md](pq-guidance.md) |
| §12.1 | Physical and logical controls restricting access to authorized persons | D, C | Users and passwords in Dioptra; physical, operating-system and network controls are customer procedures. | URS-02 |
| §12.2 | Extent of security controls based on criticality | C | Customer risk assessment. | — |
| §12.3 | Creation, change and cancellation of access authorizations recorded | D | Hash-chained access log in the users file: creation, disabling, password reset, lockout. | URS-03 |
| §12.4 | Systems record the identity of operators entering, changing, confirming or deleting data, with date and time | D | Audit trail. | URS-04 |
| §13 | Incidents reported and assessed; root cause identified | C, D | Customer incident procedure; vendor defect handling in [supplier-assessment.md](supplier-assessment.md). | — |
| §14(a) | Electronic signatures have the same impact as handwritten signatures within the company | C | Customer policy. | — |
| §14(b) | Electronic signatures permanently linked to their record | D | Signature bound to the content hash and trail head. | URS-06 |
| §14(c) | Electronic signatures include the time and date | D | UTC date and time in each signature. | URS-06 |
| §15 | Batch certification and release by a Qualified Person | N/A | Dioptra is not a batch-release system. If its records support a release decision, the customer's release procedure applies. | — |
| §16 | Business continuity for systems supporting critical processes | C, D | Customer procedure. Dioptra runs offline without a server or license server; the file format is versioned and older files are migrated on open. | — |
| §17 | Archiving: accessibility, readability and integrity checked; migration tested | C, D | Customer procedure. The file format is documented and open; records verify without a license. | URS-10 |
