# Correctness

A statistics program is a measuring instrument, and an instrument is only as good as its
calibration. This chapter explains how Dioptra's numbers are checked, then gives the full
record: every comparison, every NIST data set, and the cases where a difference was
investigated.

The short version: **all 58 NIST StRD reference data sets** used (univariate, linear
regression, one-way ANOVA and nonlinear regression) pass, and **8,327 comparisons in 300
oracle cases across 28 families** of methods agree with SciPy, statsmodels, lifelines and
scikit-learn within the tolerance stored with each case, with no failures.

## What "correct" means for a number

A computer stores a number in 64 bits (IEEE 754 double precision), which holds about 15 to
16 significant decimal digits. Every arithmetic step can lose some of them. Most of the
time the loss is invisible. Sometimes it is catastrophic: subtract two nearly equal large
numbers and the leading digits cancel, leaving mostly rounding noise.

A classic example is the variance. The textbook "one-pass" formula

> s² = (Σx² − (Σx)²/n) / (n − 1)

is exact in algebra and dangerous in a computer. Take the values 100000001, 100000002,
100000003. The answer is 1. But Σx² is about 3 × 10¹⁶, beyond the range in which doubles
hold every integer, and the subtraction of two such numbers leaves almost nothing of the
right digits. A program using that formula can report a variance of 0, or a negative one.
The two-pass formula (compute the mean, then sum squared deviations from it) gets 1
exactly. Both formulas are "correct"; only one is correct in a computer.

So the useful question is not "is the formula right?" but "how many digits of the answer
are right?"

## LRE: counting correct digits

The **log relative error** (LRE) answers that question with one number. If the certified
value is *c* and the program reports *x*,

> LRE = −log₁₀( |x − c| / |c| )

An LRE of 15 means the first 15 significant digits agree; 10 means 10 digits; 0 means
none. If *c* = 1.2345678901 and *x* = 1.2345678312, the relative error is about 4.8 × 10⁻⁸
and the LRE is 7.3: seven digits right. (When *c* is 0, the absolute error is used
instead.) McCullough (1998) introduced LRE for exactly this purpose: judging statistical software
on problems with known answers. As a rough scale, 4 digits is enough for any figure or
table in a paper, 7 is what a sound algorithm reaches on ordinary data, and 15 is all a
double can hold.

LRE is capped at the number of digits the reference certifies: 15 for most NIST sets,
11 for nonlinear regression. A cap means "as right as can be checked".

## NIST StRD: certified answers to hard problems

The US National Institute of Standards and Technology publishes the **Statistical Reference
Datasets** (StRD): data sets with answers certified to 15 digits by computations in
multiple-precision arithmetic. They are graded by difficulty. The lower sets are ordinary
data. The higher ones are built to break naive algorithms: values like 1000000000000.4 that
differ only in the last digits (ANOVA SmLs07–09), a tenth-degree polynomial on nearly
collinear predictors (Filip), nonlinear models whose parameters are almost redundant
(MGH09, Bennett5).

Dioptra's test suite reads each NIST file, runs the same routine the app uses, and
computes the LRE of every certified quantity. The test fails if any falls below a set bar.
The tables below show the result for every data set.

Some LREs are lower than 15 for a reason no program can beat. The certified value of a
data set with entries like 1000000.1 is computed from the decimal numbers, but a computer
cannot hold 1000000.1 in binary; it stores the nearest double, which is slightly different.
The exact answer for the numbers the computer actually holds therefore differs from the
certified answer in the ninth or tenth digit. To separate the algorithm's error from this
input rounding, the tables have starred columns: the LRE against the exact answer for the
doubles, computed in rational arithmetic. There Dioptra reaches 15 digits, or close to it,
nearly everywhere.

## Oracle fixtures: agreeing with the reference implementations

NIST covers only four families of methods. For everything else (t tests, rank tests, the
studentised range, Fisher's exact test, Kaplan–Meier, ROC, PCA and so on) the reference is
the established open-source implementation: SciPy, statsmodels, lifelines and
scikit-learn. A Python script in the repository (`scripts/oracle/`) runs those libraries on
fixed inputs and writes the inputs and outputs to JSON fixture files with a tolerance for
each case. Dioptra's tests read the fixtures and compare.

The tolerance is chosen per quantity: tight (10⁻¹² relative) for exact rational results
such as exact p values, looser where the reference itself is approximate (SciPy's
randomised Dunnett integration, for example). Where Dioptra and a reference disagree beyond
rounding, the disagreement was investigated to its source, often in high-precision
arithmetic. Several of the notes below record cases where the reference library, not
Dioptra, was the less accurate of the two.

A method with no reference check does not ship. The few methods without an oracle fixture
(intraclass correlation, Cohen's kappa, the bootstrap, permutation tests) are tested against
published worked examples and mathematical identities, listed at the end of the record.

## Reproducing the record

Everything below is regenerated by

```sh
cargo run -p dioptra-stats --example correctness_report
```

and the same comparisons run in `cargo test -p dioptra-stats` on every change.

---

{{#include ../../CORRECTNESS.md:3:}}
