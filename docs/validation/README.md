# Dioptra validation pack

Vendor documentation for laboratories that validate Dioptra for regulated work under
21 CFR Part 11, EU GMP Annex 11 or GLP. Dioptra supplies technical controls and evidence
that support compliance; the regulated company decides whether its validated system,
procedures and records are compliant.

The pack applies to the release it ships with. Its version is the version of Dioptra
(see [change-control.md](change-control.md)).

## Contents

| Document | Purpose | Used by |
|---|---|---|
| [compliance-matrix.md](compliance-matrix.md) | Part 11 and Annex 11, clause by clause: Dioptra control, customer procedure, or not applicable | QA, validation lead |
| [urs-frs-traceability.md](urs-frs-traceability.md) | User requirements → functional specifications → vendor tests and OQ checks | Validation lead |
| [iq-protocol.md](iq-protocol.md) | Installation qualification: install per operating system, verify release checksums | IT, validation lead |
| [oq-protocol.md](oq-protocol.md) | Operational qualification: the automated OQ and manual test scripts for the records controls | Validation lead, tester |
| [pq-guidance.md](pq-guidance.md) | What the customer covers in performance qualification | Validation lead, process owner |
| [change-control.md](change-control.md) | Versioning, release notes, what triggers re-validation | QA, IT |
| [supplier-assessment.md](supplier-assessment.md) | Answers to a supplier questionnaire | QA, purchasing |

The user documentation of the controls is the manual chapter
[Records for regulated labs](../manual/src/regulated.md). Numerical accuracy is documented
in [CORRECTNESS.md](../CORRECTNESS.md).

## Roles

| Activity | Vendor | Customer |
|---|---|---|
| Software development, testing and release under the process in [supplier-assessment.md](supplier-assessment.md) | Performs | Assesses |
| Release artifacts and SHA-256 checksums | Publishes | Verifies at IQ |
| Automated OQ (NIST StRD and oracle comparisons, embedded in every build) | Supplies and maintains | Executes on each installation, reviews and approves the report |
| Manual OQ scripts for the records controls | Supplies | Executes, records, approves |
| Compliance matrix and traceability matrix | Supplies | Reviews; adds customer procedures |
| Risk assessment, validation plan and report | Supplies inputs | Writes and approves |
| PQ, standard operating procedures, training | Supplies guidance | Writes, performs, approves |
| Users, roles, storage, backup, retention of records | Supplies the controls | Configures and operates |
| Change control of installed versions | Publishes release notes | Assesses each update, decides on re-validation |

## How to use it

1. Assess the vendor with [supplier-assessment.md](supplier-assessment.md).
2. Write the validation plan. Classify Dioptra by your own procedure (GAMP 5: a
   configurable off-the-shelf product; the statistical engine is not configured by the
   user).
3. Map your requirements against [compliance-matrix.md](compliance-matrix.md) and
   [urs-frs-traceability.md](urs-frs-traceability.md). Each "customer procedure" row needs an
   SOP or a documented decision.
4. Install and execute the [IQ](iq-protocol.md) on every computer, or on a representative
   computer per approved image.
5. Execute the [OQ](oq-protocol.md): the automated OQ on every installation, the manual
   scripts once per version.
6. Perform the [PQ](pq-guidance.md) for your intended use.
7. Release the system for use, and manage updates by [change-control.md](change-control.md).

## Vendor contact

**Contact (to be completed by the vendor before issue):** name, e-mail address and
postal address for validation and quality questions.

Defects and numerical discrepancies: <https://github.com/ZahakJ/dioptra/issues>.
