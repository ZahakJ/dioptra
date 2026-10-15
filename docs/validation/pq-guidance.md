# Performance qualification guidance

Performance qualification shows that Dioptra, as installed and operated under the
customer's procedures, is fit for the customer's intended use. The vendor cannot write the
PQ: it depends on the customer's analyses, data, people and procedures. This page lists
what a PQ for Dioptra usually covers.

## 1. Define intended use

Write down, in the validation plan or URS:

- the analyses used for regulated decisions (for example one-way ANOVA with Dunnett's test,
  four-parameter logistic fit, Kaplan–Meier with log-rank test), with their options;
- the data sources (instrument exports, LIMS, manual entry) and file formats;
- the records kept (project files, reports, exported figures) and who signs them, with
  which meaning;
- the computers, storage locations and backup arrangements.

Methods outside the intended use need no PQ; exclude them in writing.

## 2. Test the intended use with known answers

For each analysis in scope:

1. Take one or more data sets with an independent answer: a previous validated system, a
   published worked example, a calculation in a second package, or a reference data set.
   The oracle and NIST cases in `fixtures/` and the methods chapter of the manual
   ([analyses/METHODS.md](../manual/src/analyses/METHODS.md)) are sources of such examples.
2. Enter or import the data as users will (same file formats and import path).
3. Run the analysis with the options of the SOP.
4. Compare every reported value used for the decision with the reference, within an
   acceptance criterion set before the test (for example agreement to the number of
   decimals reported).
5. Export the report or figure as users will and check that it shows the same values.

Include at least one data set with missing values, excluded points and the largest size
expected in routine use.

## 3. Test the records workflow under the SOPs

Run one complete regulated workflow as it will be performed in routine use, with the
production configuration of users and roles:

1. An analyst signs in, creates or opens the project, makes it regulated, imports data,
   runs the analyses, makes the figures, giving reasons as the SOP requires.
2. The analyst signs as author; a reviewer reviews the audit trail (Records → Audit trail…)
   and signs as reviewed; an approver signs as approved.
3. A correction after signing follows the SOP: new version, reason, change, new signatures.
4. The project file is stored, backed up and restored per procedure; the restored copy
   verifies intact (Records → Audit trail… → Verify).
5. The audit report (PDF) is filed as the SOP requires.

Check that the SOPs, training records and access arrangements named in the
[compliance matrix](compliance-matrix.md) as customer procedures exist and are followed.

## 4. Acceptance and report

The PQ passes when every result agrees with its reference within the stated criterion and
the workflow completes as written, or each deviation is closed per the customer's
procedure. Summarise IQ, OQ and PQ in the validation report and release the system for its
intended use.

## 5. Keeping it validated

- Re-run the automated OQ (Records → Run OQ…) after every update, operating-system upgrade
  or hardware change, and at periodic review (Annex 11 §11).
- Assess each new version under [change-control.md](change-control.md); repeat the PQ
  tests of any analysis whose behaviour the release notes say changed.
- Review the users' access log and the audit trails of regulated projects at the
  frequency the SOP sets.
