"""Generate the Dioptra correctness-oracle fixtures in fixtures/oracle/.

Run from the project root:

    "$ORACLE_PY" -I scripts/oracle/gen_fixtures.py

where ORACLE_PY points at a Python with numpy, scipy, statsmodels, lifelines,
scikit-learn, pandas and mpmath. Output is deterministic: synthetic data come
from numpy.random.default_rng(20261011) (one generator, consumed in a fixed
order), and the classic textbook sets are typed in by hand below so the same
values can be cross-checked in R.

File layout (one file per test family):
    {"test": name, "reference": "...", "notes": [...],
     "cases": [{"name", "input": {...}, "expected": {...}, "tolerance": {...}}]}

Tolerance objects: {"rel": r, "abs": a} applies to every expected number
(pass if |got-exp| <= abs or |got-exp| <= rel*|exp|); an optional "fields"
map overrides per expected key.

Non-finite numbers are written as the strings "Infinity", "-Infinity", "NaN"
(JSON has no literal for them).
"""

import itertools
import json
import math
import os
import sys
import warnings

import numpy as np
import pandas as pd
import scipy
import scipy.optimize as opt
import scipy.special as sp
import scipy.stats as st
import statsmodels
import statsmodels.api as sm
import statsmodels.formula.api as smf
from statsmodels.stats.anova import AnovaRM, anova_lm
from statsmodels.stats.multicomp import pairwise_tukeyhsd
from statsmodels.stats.multitest import multipletests
from statsmodels.stats.oneway import anova_oneway
from statsmodels.stats.diagnostic import lilliefors
from statsmodels.stats.contingency_tables import mcnemar, Table2x2
import lifelines
from lifelines import KaplanMeierFitter, CoxPHFitter
from lifelines.statistics import logrank_test
import sklearn
from sklearn.decomposition import PCA
from sklearn.metrics import roc_curve, roc_auc_score
import mpmath

warnings.filterwarnings("ignore")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "fixtures", "oracle")
SEED = 20261011
RNG = np.random.default_rng(SEED)
VERSIONS = {"numpy": np.__version__, "scipy": scipy.__version__, "statsmodels": statsmodels.__version__,
            "lifelines": lifelines.__version__, "sklearn": sklearn.__version__, "pandas": pd.__version__,
            "mpmath": mpmath.__version__, "python": sys.version.split()[0]}

TOL_EXACT = {"rel": 1e-12, "abs": 1e-14}      # closed-form arithmetic
TOL_P = {"rel": 1e-8, "abs": 1e-10}           # p-values / special functions in the body
TOL_FIT = {"rel": 1e-6, "abs": 1e-9}          # iterative fits
Z975 = st.norm.ppf(0.975)


# ----------------------------------------------------------------- helpers
def clean(o):
    if isinstance(o, dict):
        return {str(k): clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [clean(v) for v in o]
    if isinstance(o, np.ndarray):
        return [clean(v) for v in o.tolist()]
    if isinstance(o, (bool, np.bool_)):
        return bool(o)
    if isinstance(o, (int, np.integer)):
        return int(o)
    if isinstance(o, (float, np.floating)):
        f = float(o)
        if math.isnan(f):
            return "NaN"
        if math.isinf(f):
            return "Infinity" if f > 0 else "-Infinity"
        return float(repr(f))
    if isinstance(o, mpmath.mpf):
        return clean(float(o))
    return o


WRITTEN = []


def write(test, reference, cases, notes=None):
    doc = clean({"test": test, "reference": reference, "notes": notes or [],
                 "generator": {"script": "scripts/oracle/gen_fixtures.py", "seed": SEED, "versions": VERSIONS},
                 "cases": cases})
    path = os.path.join(OUT, test + ".json")
    with open(path, "w") as fh:
        json.dump(doc, fh, indent=1, allow_nan=False)
    with open(path) as fh:
        json.load(fh)
    size = os.path.getsize(path)
    assert size < 400_000, (test, size)
    WRITTEN.append((test, len(cases), size))


def case(name, inp, exp, tol, **extra):
    d = {"name": name, "input": inp, "expected": exp, "tolerance": tol}
    d.update(extra)
    return d


def r(x, k=4):
    """Round synthetic inputs so they are exactly representable short decimals."""
    return [float(round(float(v), k)) for v in x]


# ------------------------------------------------- classic hand-entered datasets
# R datasets::sleep (Cushny & Peebles 1905, as used by Student 1908): extra sleep hours.
SLEEP_1 = [0.7, -1.6, -0.2, -1.2, -0.1, 3.4, 3.7, 0.8, 0.0, 2.0]
SLEEP_2 = [1.9, 0.8, 1.1, 0.1, -0.1, 4.4, 5.5, 1.6, 4.6, 3.4]
# R datasets::PlantGrowth (Dobson 1983): dried plant weight.
PG_CTRL = [4.17, 5.58, 5.18, 6.11, 4.50, 4.61, 5.17, 4.53, 5.33, 5.14]
PG_TRT1 = [4.81, 4.17, 4.41, 3.59, 5.87, 3.83, 6.03, 4.89, 4.32, 4.69]
PG_TRT2 = [6.31, 5.12, 5.54, 5.50, 5.37, 5.29, 4.92, 6.15, 5.80, 5.26]
# R ?wilcox.test examples (Hollander & Wolfe 1973): depression scale, paired.
HW_X = [1.83, 0.50, 1.62, 2.48, 1.68, 1.88, 1.55, 3.06, 1.30]
HW_Y = [0.878, 0.647, 0.598, 2.05, 1.06, 1.29, 1.06, 3.14, 1.29]
# R ?wilcox.test two-sample example (Hollander & Wolfe 1973 p.69): permeability.
HW2_X = [0.80, 0.83, 1.89, 1.04, 1.45, 1.38, 1.91, 1.64, 0.73, 1.46]
HW2_Y = [1.15, 0.88, 0.90, 0.74, 1.21]
# R ?kruskal.test example (Hollander & Wolfe 1973 p.116): mucociliary efficiency.
KW_X = [2.9, 3.0, 2.5, 2.6, 3.2]
KW_Y = [3.8, 2.7, 4.0, 2.4]
KW_Z = [2.8, 3.4, 3.7, 2.2, 2.0]
# Anscombe (1973) quartet, set I.
ANS_X = [10, 8, 13, 9, 11, 14, 6, 4, 12, 7, 5]
ANS_Y1 = [8.04, 6.95, 7.58, 8.81, 8.33, 9.96, 7.24, 4.26, 10.84, 4.82, 5.68]
# Gehan (1965) leukaemia remission times (weeks); event=0 means censored.
GEHAN_6MP_T = [6, 6, 6, 6, 7, 9, 10, 10, 11, 13, 16, 17, 19, 20, 22, 23, 25, 32, 32, 34, 35]
GEHAN_6MP_E = [1, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]
GEHAN_PL_T = [1, 1, 2, 2, 3, 4, 4, 5, 5, 8, 8, 8, 8, 11, 11, 12, 12, 15, 17, 22, 23]
GEHAN_PL_E = [1] * 21
# Bland & Altman (1986, Lancet) peak expiratory flow rate, Wright vs mini Wright meter (first readings).
BA_WRIGHT = [494, 395, 516, 434, 476, 557, 413, 442, 650, 433, 417, 656, 267, 478, 178, 423, 427]
BA_MINI = [512, 430, 520, 428, 500, 600, 364, 380, 658, 445, 432, 626, 260, 477, 259, 350, 451]


# ================================================================= descriptive
def tukey_hinges(x):
    """Tukey's hinges: medians of the lower and upper halves, the median itself
    included in both halves when n is odd (R fivenum)."""
    s = sorted(x)
    n = len(s)
    h = (n + 1) // 2  # size of each half including the middle point when n odd
    return float(np.median(s[:h])), float(np.median(s[n - h:]))


def describe(x):
    a = np.asarray(x, float)
    n = a.size
    mean = a.mean()
    sd = a.std(ddof=1)
    sem = sd / math.sqrt(n)
    tcrit = st.t.ppf(0.975, n - 1)
    out = {
        "n": n, "sum": float(a.sum()), "min": a.min(), "max": a.max(), "range": a.max() - a.min(),
        "mean": mean, "sd": sd, "variance": a.var(ddof=1), "sem": sem,
        "sd_population": a.std(ddof=0),
        "median": np.median(a),
        "cv": sd / mean,
        "ci95_mean": [mean - tcrit * sem, mean + tcrit * sem],
        "quartiles": {
            "linear_type7": [np.quantile(a, 0.25, method="linear"), np.quantile(a, 0.75, method="linear")],
            "weibull_type6": [np.quantile(a, 0.25, method="weibull"), np.quantile(a, 0.75, method="weibull")],
            "tukey_hinges": list(tukey_hinges(a)),
        },
        "percentiles_10_90": {
            "linear_type7": [np.quantile(a, 0.10, method="linear"), np.quantile(a, 0.90, method="linear")],
            "weibull_type6": [np.quantile(a, 0.10, method="weibull"), np.quantile(a, 0.90, method="weibull")],
        },
        "skewness_g1_biased": st.skew(a, bias=True),
        "skewness_G1_adjusted": st.skew(a, bias=False) if n >= 3 else None,
        "kurtosis_excess_g2_biased": st.kurtosis(a, fisher=True, bias=True),
        "kurtosis_excess_G2_adjusted": st.kurtosis(a, fisher=True, bias=False) if n >= 4 else None,
    }
    if np.all(a > 0):
        gm = st.gmean(a)
        lg = np.log(a)
        lsem = lg.std(ddof=1) / math.sqrt(n)
        out["geometric_mean"] = gm
        out["geometric_sd_factor"] = math.exp(lg.std(ddof=1))
        out["ci95_geometric_mean"] = [math.exp(lg.mean() - tcrit * lsem), math.exp(lg.mean() + tcrit * lsem)]
        out["harmonic_mean"] = st.hmean(a)
    else:
        out["geometric_mean"] = None
    return out


def gen_descriptive():
    sets = [
        ("sleep_drug1_classic", SLEEP_1),
        ("plantgrowth_ctrl_classic", PG_CTRL),
        ("n3_small", [2.0, 4.0, 9.0]),
        ("n4_even", [1.0, 2.0, 3.0, 10.0]),
        ("n5_odd", [3.1, 1.2, 5.5, 2.2, 4.0]),
        ("ties_many", [1, 2, 2, 2, 3, 3, 4, 4, 4, 4, 5, 7]),
        ("large_offset_numacc_like", [1e9 + 1, 1e9 + 2, 1e9 + 3, 1e9 + 4, 1e9 + 5.5]),
        ("extreme_magnitudes", [1e-8, 3e-8, 2.5e-8, 7e-8, 1.1e-7, 5e-9]),
        ("skewed_lognormal_n25", r(RNG.lognormal(1.0, 0.8, 25), 5)),
        ("normal_n200", r(RNG.normal(50.0, 7.0, 200), 4)),
        ("with_negatives_n9", [-3.5, -1.0, 0.0, 0.5, 2.0, 2.0, 4.5, 8.0, -0.25]),
        ("constant_offset_heavy_tail_n15", r(RNG.standard_t(2, 15) + 100, 5)),
    ]
    cases = [case(n, {"x": list(map(float, x))}, describe(x), TOL_EXACT) for n, x in sets]
    write("descriptive", "numpy mean/std(ddof)/quantile(method=linear|weibull); scipy.stats skew/kurtosis(bias=True|False), gmean, hmean; t-based CI",
          cases, notes=[
              "sd/variance/sem use n-1; sd_population uses n.",
              "quartiles.linear_type7 = numpy/scipy default (Hyndman-Fan type 7, Excel QUARTILE.INC).",
              "quartiles.weibull_type6 = Hyndman-Fan type 6, h=(n+1)p (Excel QUARTILE.EXC, Minitab, SPSS); see research/04-algorithms.md for which one Prism uses.",
              "tukey_hinges = R fivenum: median of each half, middle value included in both halves when n is odd.",
              "skewness_G1_adjusted = g1*sqrt(n(n-1))/(n-2); kurtosis_excess_G2_adjusted = ((n+1)g2+6)(n-1)/((n-2)(n-3)).",
              "geometric CI = exp(mean(ln x) +/- t(0.975,n-1)*sd(ln x)/sqrt(n)); geometric_sd_factor = exp(sd(ln x)).",
              "cv = sd/mean (fraction, not percent).",
          ])


# ================================================================= t tests
def hedges_J(df):
    """Exact small-sample correction J(df) = Gamma(df/2)/(sqrt(df/2) Gamma((df-1)/2))."""
    return math.exp(sp.gammaln(df / 2) - sp.gammaln((df - 1) / 2)) / math.sqrt(df / 2)


def ttest_block(res, est_diff):
    ci = res.confidence_interval(0.95)
    return {"t": res.statistic, "df": res.df, "mean_difference": est_diff,
            "ci95_difference": [ci.low, ci.high]}


def gen_ttests():
    cases = []
    # one-sample
    for name, x, mu in [("sleep1_vs_0_classic", SLEEP_1, 0.0), ("n3", [5.1, 4.9, 5.6], 5.0),
                        ("normal_n30", r(RNG.normal(10.3, 2.0, 30), 3), 10.0),
                        ("large_t", [100.01, 100.02, 100.015, 100.012, 100.018], 100.0)]:
        a = np.asarray(x)
        exp = ttest_block(st.ttest_1samp(a, mu), a.mean() - mu)
        for alt in ("two-sided", "less", "greater"):
            exp["p_" + alt.replace("-", "_")] = st.ttest_1samp(a, mu, alternative=alt).pvalue
        d = (a.mean() - mu) / a.std(ddof=1)
        exp.update({"cohens_d": d, "hedges_g": d * hedges_J(a.size - 1),
                    "hedges_g_approx": d * (1 - 3 / (4 * (a.size - 1) - 1))})
        cases.append(case("one_sample/" + name, {"x": list(map(float, x)), "mu": mu}, exp, TOL_P))
    # two-sample Student & Welch
    two = [("sleep_classic", SLEEP_1, SLEEP_2), ("plantgrowth_trt2_vs_ctrl_classic", PG_TRT2, PG_CTRL),
           ("unequal_n_unequal_var", r(RNG.normal(5, 1, 6), 3), r(RNG.normal(6.5, 3, 15), 3)),
           ("n2_n3", [1.0, 2.0], [3.0, 4.5, 5.0]),
           ("extreme_separation", [1.0, 1.1, 0.9, 1.05], [100.0, 100.2, 99.8, 100.1])]
    for name, x, y in two:
        a, b = np.asarray(x), np.asarray(y)
        n1, n2 = a.size, b.size
        sp2 = ((n1 - 1) * a.var(ddof=1) + (n2 - 1) * b.var(ddof=1)) / (n1 + n2 - 2)
        diff = a.mean() - b.mean()
        d = diff / math.sqrt(sp2)
        for kind, eq in (("student", True), ("welch", False)):
            res = st.ttest_ind(a, b, equal_var=eq)
            exp = ttest_block(res, diff)
            for alt in ("two-sided", "less", "greater"):
                exp["p_" + alt.replace("-", "_")] = st.ttest_ind(a, b, equal_var=eq, alternative=alt).pvalue
            exp["cohens_d_pooled"] = d
            exp["hedges_g_pooled"] = d * hedges_J(n1 + n2 - 2)
            exp["cohens_d_av"] = diff / math.sqrt((a.var(ddof=1) + b.var(ddof=1)) / 2)
            if kind == "student":
                exp["pooled_sd"] = math.sqrt(sp2)
            else:
                exp["se_difference"] = math.sqrt(a.var(ddof=1) / n1 + b.var(ddof=1) / n2)
            cases.append(case(kind + "/" + name, {"x": list(map(float, x)), "y": list(map(float, y))}, exp, TOL_P))
    # paired and ratio paired
    paired = [("sleep_classic", SLEEP_2, SLEEP_1), ("hollander_wolfe_classic", HW_X, HW_Y),
              ("n3", [10.0, 12.0, 15.0], [9.0, 11.5, 13.0])]
    base = r(RNG.lognormal(2, 0.5, 12), 3)
    paired.append(("lognormal_n12", r(np.asarray(base) * RNG.lognormal(0.15, 0.1, 12), 3), base))
    for name, x, y in paired:
        a, b = np.asarray(x), np.asarray(y)
        dd = a - b
        res = st.ttest_rel(a, b)
        exp = ttest_block(res, dd.mean())
        for alt in ("two-sided", "less", "greater"):
            exp["p_" + alt.replace("-", "_")] = st.ttest_rel(a, b, alternative=alt).pvalue
        dz = dd.mean() / dd.std(ddof=1)
        exp.update({"sd_differences": dd.std(ddof=1), "cohens_dz": dz, "hedges_gz": dz * hedges_J(dd.size - 1)})
        cases.append(case("paired/" + name, {"x": list(map(float, x)), "y": list(map(float, y))}, exp, TOL_P))
        if np.all(a > 0) and np.all(b > 0):
            la, lb = np.log10(a), np.log10(b)
            rr = st.ttest_rel(la, lb)
            ci = rr.confidence_interval(0.95)
            ex = {"t": rr.statistic, "df": rr.df, "p_two_sided": rr.pvalue,
                  "mean_log10_ratio": (la - lb).mean(), "ci95_mean_log10_ratio": [ci.low, ci.high],
                  "geometric_mean_ratio": 10 ** (la - lb).mean(), "ci95_ratio": [10 ** ci.low, 10 ** ci.high]}
            cases.append(case("ratio_paired/" + name, {"x": list(map(float, x)), "y": list(map(float, y))}, ex, TOL_P))
    write("t_tests", "scipy.stats.ttest_1samp / ttest_ind(equal_var=True|False) / ttest_rel; result.confidence_interval(0.95)",
          cases, notes=[
              "Difference convention: mean(x) - mean(y); paired differences are x - y.",
              "p_less / p_greater are scipy's one-sided alternatives (H1: mean(x)-mean(y) < 0 / > 0).",
              "cohens_d_pooled = diff/pooled SD (pooled with n-1 weights); hedges_g = d * J(df), J exact gamma form; hedges_g_approx uses J ~ 1-3/(4df-1).",
              "cohens_d_av = diff/sqrt((s1^2+s2^2)/2), reported for Welch users.",
              "Welch df is the Welch-Satterthwaite df (non-integer).",
              "ratio_paired: paired t test on log10(x)-log10(y); ratio = 10^mean, CI back-transformed. t and p do not depend on the log base.",
          ])


# ================================================================= Mann-Whitney
def hl_two_sample(x, y):
    return float(np.median(np.subtract.outer(np.asarray(x, float), np.asarray(y, float)).ravel()))


def gen_mann_whitney():
    cases = []
    sets = [("hollander_wolfe_classic", HW2_X, HW2_Y, True),
            ("n3_n4_no_ties", [1.1, 2.3, 4.5], [3.3, 5.6, 6.1, 7.2], True),
            ("n7_n9_no_ties", r(RNG.normal(0, 1, 7), 4), r(RNG.normal(0.8, 1, 9), 4), True),
            ("complete_separation_n5_n5", [1, 2, 3, 4, 5], [6, 7, 8, 9, 10], True),
            ("ties_n8_n10", [1, 2, 2, 3, 3, 3, 4, 5], [2, 3, 4, 4, 5, 5, 6, 6, 7, 7], False),
            ("unequal_n_20_35_ties", [float(v) for v in RNG.integers(0, 12, 20)], [float(v) for v in RNG.integers(3, 15, 35)], False),
            ("plantgrowth_trt1_vs_ctrl_classic_ties", PG_TRT1, PG_CTRL, False)]
    for name, x, y, exact in sets:
        n1, n2 = len(x), len(y)
        exp = {}
        res = st.mannwhitneyu(x, y, method="asymptotic", use_continuity=True)
        exp["U1"] = res.statistic
        exp["U2"] = n1 * n2 - res.statistic
        exp["U_min"] = min(exp["U1"], exp["U2"])
        exp["rank_sum_x"] = st.rankdata(np.r_[x, y])[:n1].sum()
        for alt in ("two-sided", "less", "greater"):
            k = alt.replace("-", "_")
            exp["p_asymptotic_cc_" + k] = st.mannwhitneyu(x, y, method="asymptotic", use_continuity=True, alternative=alt).pvalue
            exp["p_asymptotic_nocc_" + k] = st.mannwhitneyu(x, y, method="asymptotic", use_continuity=False, alternative=alt).pvalue
            if exact:
                exp["p_exact_" + k] = st.mannwhitneyu(x, y, method="exact", alternative=alt).pvalue
        # z with tie correction (no continuity), for reference
        ranks = st.rankdata(np.r_[x, y])
        N = n1 + n2
        _, t = np.unique(ranks, return_counts=True)
        sigma = math.sqrt(n1 * n2 / 12 * ((N + 1) - (t ** 3 - t).sum() / (N * (N - 1))))
        exp["z_tie_corrected_nocc"] = (exp["U1"] - n1 * n2 / 2) / sigma
        exp["hodges_lehmann_shift_x_minus_y"] = hl_two_sample(x, y)
        cases.append(case(name, {"x": list(map(float, x)), "y": list(map(float, y)), "has_ties": not exact}, exp, TOL_P))
    write("mann_whitney", "scipy.stats.mannwhitneyu(method='exact'|'asymptotic', use_continuity=True|False)", cases, notes=[
        "U1 = U statistic for x (number of pairs x_i > y_j, ties count 1/2); U2 = n1*n2 - U1.",
        "Asymptotic p uses the tie-corrected variance n1 n2/12 [(N+1) - sum(t^3-t)/(N(N-1))]; 'cc' = continuity correction 0.5.",
        "Exact p only for tie-free cases (scipy exact ignores ties).",
        "hodges_lehmann_shift_x_minus_y = median of all n1*n2 differences x_i - y_j.",
    ])


# ================================================================= Wilcoxon signed rank
def walsh_hl(d):
    d = np.asarray(d, float)
    i, j = np.triu_indices(d.size)
    return float(np.median((d[i] + d[j]) / 2))


def gen_wilcoxon():
    cases = []
    sets = [("hollander_wolfe_classic", HW_X, HW_Y, True),
            ("sleep_classic_ties_and_zero", SLEEP_2, SLEEP_1, False),
            ("n5_no_ties", [1.2, 3.4, 2.2, 5.0, 0.3], [1.0, 1.0, 1.0, 1.0, 1.0], True),
            ("n12_no_ties", r(RNG.normal(0.5, 1, 12), 4), [0.0] * 12, True),
            ("with_zeros_and_ties", [1, 2, 3, 3, 4, 5, 5, 6, 7, 8, 9, 2, 3], [1, 1, 3, 2, 2, 7, 3, 2, 7, 5, 4, 2, 0], False),
            ("n30_ties", [float(v) for v in RNG.integers(-5, 9, 30)], [0.0] * 30, False)]
    for name, x, y, exact in sets:
        d = np.asarray(x, float) - np.asarray(y, float)
        nz = d[d != 0]
        rk = st.rankdata(np.abs(nz))
        exp = {"n_nonzero": int(nz.size), "n_zero": int((d == 0).sum()),
               "W_plus_wilcox": rk[nz > 0].sum(), "W_minus_wilcox": rk[nz < 0].sum()}
        rk_all = st.rankdata(np.abs(d))
        exp["W_plus_pratt"] = rk_all[d > 0].sum()
        exp["W_minus_pratt"] = rk_all[d < 0].sum()
        for zm in ("wilcox", "pratt"):
            for corr in (False, True):
                for alt in ("two-sided", "less", "greater"):
                    res = st.wilcoxon(d, zero_method=zm, correction=corr, alternative=alt, method="approx")
                    exp["p_approx_%s_%s_%s" % (zm, "cc" if corr else "nocc", alt.replace("-", "_"))] = res.pvalue
            res = st.wilcoxon(d, zero_method=zm, method="approx")
            exp["statistic_scipy_two_sided_" + zm] = res.statistic
            exp["z_" + zm] = res.zstatistic
        if exact:
            for alt in ("two-sided", "less", "greater"):
                exp["p_exact_" + alt.replace("-", "_")] = st.wilcoxon(d, method="exact", alternative=alt).pvalue
        exp["hodges_lehmann_median_difference"] = walsh_hl(d)
        exp["hodges_lehmann_nonzero_only"] = walsh_hl(nz)
        cases.append(case(name, {"x": list(map(float, x)), "y": list(map(float, y)), "differences": list(map(float, d))}, exp, TOL_P))
    write("wilcoxon_signed_rank", "scipy.stats.wilcoxon(d, zero_method='wilcox'|'pratt', correction, method='exact'|'approx')", cases, notes=[
        "Differences d = x - y.",
        "'wilcox' drops zero differences before ranking; 'pratt' ranks them and then drops their ranks from the statistic (Pratt 1959).",
        "statistic_scipy_two_sided = min(W+, W-) as scipy reports it; W_plus/W_minus given explicitly for both zero conventions.",
        "Normal approximation uses tie-corrected variance; 'cc' adds the 0.5 continuity correction.",
        "Exact p only for cases without zeros or ties.",
        "hodges_lehmann_median_difference = median of Walsh averages (d_i+d_j)/2, i<=j, zeros kept; *_nonzero_only drops zeros first.",
    ])


# ================================================================= one-way ANOVA
def welch_anova(groups):
    """Welch (1951) heteroscedastic one-way ANOVA."""
    n = np.array([len(g) for g in groups], float)
    m = np.array([np.mean(g) for g in groups])
    v = np.array([np.var(g, ddof=1) for g in groups])
    k = len(groups)
    w = n / v
    W = w.sum()
    mw = (w * m).sum() / W
    A = (w * (m - mw) ** 2).sum() / (k - 1)
    lam = ((1 - w / W) ** 2 / (n - 1)).sum()
    B = 1 + 2 * (k - 2) / (k ** 2 - 1) * lam
    F = A / B
    df2 = (k ** 2 - 1) / (3 * lam)
    return F, k - 1, df2, st.f.sf(F, k - 1, df2)


def brown_forsythe_means(groups):
    """Brown & Forsythe (1974) F* test of equal means under unequal variances."""
    n = np.array([len(g) for g in groups], float)
    m = np.array([np.mean(g) for g in groups])
    v = np.array([np.var(g, ddof=1) for g in groups])
    N = n.sum()
    gm = (n * m).sum() / N
    num = (n * (m - gm) ** 2).sum()
    den = ((1 - n / N) * v).sum()
    F = num / den
    c = (1 - n / N) * v / den
    df2 = 1 / ((c ** 2) / (n - 1)).sum()
    return F, len(groups) - 1, df2, st.f.sf(F, len(groups) - 1, df2)


def oneway_table(groups):
    allv = np.concatenate([np.asarray(g, float) for g in groups])
    N, k = allv.size, len(groups)
    gm = allv.mean()
    ssb = sum(len(g) * (np.mean(g) - gm) ** 2 for g in groups)
    ssw = sum(((np.asarray(g) - np.mean(g)) ** 2).sum() for g in groups)
    sst = ((allv - gm) ** 2).sum()
    dfb, dfw = k - 1, N - k
    msb, msw = ssb / dfb, ssw / dfw
    return {"ss_between": ssb, "ss_within": ssw, "ss_total": sst, "df_between": dfb, "df_within": dfw,
            "df_total": N - 1, "ms_between": msb, "ms_within": msw, "F": msb / msw, "p": st.f.sf(msb / msw, dfb, dfw),
            "r_squared_eta2": ssb / sst, "omega2": (ssb - dfb * msw) / (sst + msw),
            "group_means": [np.mean(g) for g in groups], "group_sds": [np.std(g, ddof=1) for g in groups],
            "group_n": [len(g) for g in groups]}


def anova_sets():
    return [("plantgrowth_classic", [PG_CTRL, PG_TRT1, PG_TRT2]),
            ("kruskal_example_classic", [KW_X, KW_Y, KW_Z]),
            ("unequal_n_unequal_var", [r(RNG.normal(10, 1, 5), 3), r(RNG.normal(11, 3, 12), 3), r(RNG.normal(13, 6, 8), 3), r(RNG.normal(10.5, 0.5, 20), 3)]),
            ("k2_small", [[1.0, 2.0, 3.0], [4.0, 5.0, 7.0]]),
            ("k6_balanced_n6", [r(RNG.normal(20 + 0.6 * i, 2, 6), 3) for i in range(6)])]


def gen_anova_oneway():
    cases = []
    for name, groups in anova_sets():
        exp = oneway_table(groups)
        Fs, ps = st.f_oneway(*groups)
        assert abs(Fs - exp["F"]) <= 1e-9 * exp["F"]
        F, d1, d2, p = welch_anova(groups)
        sw = anova_oneway(groups, use_var="unequal", welch_correction=True)
        assert abs(sw.statistic - F) < 1e-8 * F and abs(sw.pvalue - p) < 1e-8
        exp["welch"] = {"F": F, "df1": d1, "df2": d2, "p": p}
        F, d1, d2, p = brown_forsythe_means(groups)
        sb = anova_oneway(groups, use_var="bf")
        assert abs(sb.statistic - F) < 1e-8 * F and abs(sb.pvalue2 - p) < 1e-8, (sb, F, p)
        exp["brown_forsythe_means_Fstar"] = {"F": F, "df1": d1, "df2": d2, "p": p,
                                             "mehrotra_1997_df1": sb.df_num, "mehrotra_1997_p": sb.pvalue}
        lev = st.levene(*groups, center="median")
        exp["brown_forsythe_levene_median"] = {"W": lev.statistic, "p": lev.pvalue}
        lev2 = st.levene(*groups, center="mean")
        exp["levene_mean"] = {"W": lev2.statistic, "p": lev2.pvalue}
        b = st.bartlett(*groups)
        exp["bartlett"] = {"statistic": b.statistic, "p": b.pvalue}
        cases.append(case(name, {"groups": [list(map(float, g)) for g in groups]}, exp, TOL_P))
    write("anova_oneway", "scipy.stats.f_oneway; statsmodels.stats.oneway.anova_oneway(use_var='unequal'|'bf'); scipy.stats.levene/bartlett", cases, notes=[
        "omega2 = (SSB - dfB*MSW)/(SST + MSW); eta2 = SSB/SST.",
        "welch = Welch 1951 F with df2 = (k^2-1)/(3*sum((1-w_i/W)^2/(n_i-1))), w_i = n_i/s_i^2 (cross-checked with statsmodels).",
        "brown_forsythe_means_Fstar = Brown & Forsythe 1974 F* test of MEANS (what Prism calls Brown-Forsythe ANOVA), Satterthwaite df2; mehrotra_1997_* = statsmodels default modified numerator df (Mehrotra 1997).",
        "brown_forsythe_levene_median = Brown-Forsythe test of equal VARIANCES (Levene on |x - median|). Different test, same name.",
    ])


# ================================================================= two-way ANOVA
def gen_anova_twoway():
    cases = []
    # balanced 2 x 3 with n=4 per cell
    rows = []
    eff_a = {"a1": 0.0, "a2": 1.5}
    eff_b = {"b1": 0.0, "b2": 0.8, "b3": 2.0}
    for a in eff_a:
        for b in eff_b:
            inter = 1.0 if (a, b) == ("a2", "b3") else 0.0
            for v in RNG.normal(10 + eff_a[a] + eff_b[b] + inter, 1.0, 4):
                rows.append((a, b, float(round(v, 3))))
    balanced = rows
    # unbalanced 3 x 2 with cell sizes 2..6
    sizes = {("a1", "b1"): 3, ("a1", "b2"): 6, ("a2", "b1"): 5, ("a2", "b2"): 2, ("a3", "b1"): 4, ("a3", "b2"): 5}
    rows = []
    for (a, b), m in sizes.items():
        mu = 20 + {"a1": 0, "a2": 2, "a3": 3.5}[a] + {"b1": 0, "b2": 1.2}[b] + (1.5 if (a, b) == ("a3", "b2") else 0)
        for v in RNG.normal(mu, 1.5, m):
            rows.append((a, b, float(round(v, 3))))
    unbalanced = rows
    for name, data in (("balanced_2x3_n4", balanced), ("unbalanced_3x2", unbalanced)):
        df = pd.DataFrame(data, columns=["A", "B", "y"])
        exp = {}
        m_sum = smf.ols("y ~ C(A, Sum) * C(B, Sum)", data=df).fit()
        m_trt = smf.ols("y ~ C(A) * C(B)", data=df).fit()
        for typ in (1, 2, 3):
            tab = anova_lm(m_sum, typ=typ)
            out = {}
            for idx, key in zip(tab.index, tab.index):
                k2 = key.replace("C(A, Sum)", "A").replace("C(B, Sum)", "B")
                out[k2] = {"ss": tab.loc[idx, "sum_sq"], "df": tab.loc[idx, "df"],
                           "F": tab.loc[idx, "F"] if not pd.isna(tab.loc[idx, "F"]) else None,
                           "p": tab.loc[idx, "PR(>F)"] if not pd.isna(tab.loc[idx, "PR(>F)"]) else None}
            exp["type%d" % typ] = out
        # type III with treatment coding is NOT the classical type III: record to show the trap
        t3 = anova_lm(m_trt, typ=3)
        exp["type3_treatment_coding_WRONG_for_reference"] = {
            k.replace("C(A)", "A").replace("C(B)", "B"): t3.loc[k, "sum_sq"] for k in t3.index}
        exp["residual_df"] = m_sum.df_resid
        exp["mse"] = m_sum.mse_resid
        exp["cell_means"] = {"%s,%s" % k: float(v) for k, v in df.groupby(["A", "B"])["y"].mean().items()}
        cases.append(case(name, {"A": list(df.A), "B": list(df.B), "y": list(df.y)}, exp, TOL_P))
    write("anova_twoway", "statsmodels ols('y ~ C(A, Sum) * C(B, Sum)') + anova_lm(typ=1|2|3)", cases, notes=[
        "All three types use sum-to-zero (deviation) contrasts C(., Sum). Type III is only meaningful with such contrasts; "
        "type3_treatment_coding_WRONG_for_reference shows what dummy (treatment) coding gives, as a regression trap.",
        "Type I is sequential in the order A, B, A:B. Balanced data: types I, II, III coincide.",
        "Prism reports type III for unbalanced two-way ANOVA (see research/04-algorithms.md).",
        "Factor levels sort lexicographically (a1<a2<a3, b1<b2<b3).",
    ])


# ================================================================= repeated measures
def helmert_orthonormal(k):
    """(k-1) x k orthonormal contrasts, rows orthogonal to the ones vector."""
    C = np.zeros((k - 1, k))
    for i in range(1, k):
        C[i - 1, :i] = 1.0
        C[i - 1, i] = -i
        C[i - 1] /= math.sqrt(i * (i + 1))
    return C


def sphericity(Y):
    n, k = Y.shape
    S = np.cov(Y, rowvar=False, ddof=1)
    C = helmert_orthonormal(k)
    Sc = C @ S @ C.T
    p = k - 1
    gg = np.trace(Sc) ** 2 / (p * np.trace(Sc @ Sc))
    # Box / Greenhouse-Geisser classical form from the double-centred covariance matrix
    sbar = S.mean()
    sdiag = np.trace(S) / k
    rowm = S.mean(axis=1)
    gg2 = k ** 2 * (sdiag - sbar) ** 2 / ((k - 1) * ((S ** 2).sum() - 2 * k * (rowm ** 2).sum() + k ** 2 * sbar ** 2))
    assert abs(gg - gg2) < 1e-12, (gg, gg2)
    # Huynh-Feldt with Lecoutre (1991) correction: ((N-g+1) p gg - 2)/(p (N-g - p gg));
    # with one group (g=1, N=n) it coincides with the original HF 1976 form.
    hf = (n * p * gg - 2) / (p * (n - 1 - p * gg))
    W = np.linalg.det(Sc) / (np.trace(Sc) / p) ** p
    f = 1 - (2 * p ** 2 + p + 2) / (6 * p * (n - 1))
    chi2 = -(n - 1) * f * math.log(W)
    dfm = p * (p + 1) / 2 - 1
    return {"gg_epsilon": gg, "gg_epsilon_box_formula": gg2, "hf_epsilon_uncapped": hf,
            "hf_epsilon": min(1.0, hf),
            "lower_bound_epsilon": 1 / p,
            "mauchly_W": W, "mauchly_chi2": chi2, "mauchly_df": dfm, "mauchly_p": st.chi2.sf(chi2, dfm) if dfm > 0 else None}


def gen_anova_rm():
    cases = []
    sets = []
    subj = RNG.normal(0, 2, 8)
    Y = np.array([[round(50 + s + e, 2) for e in (0.0, 1.0, 2.5, 2.0) + RNG.normal(0, 1, 4)] for s in subj])
    sets.append(("n8_k4_spherical_ish", Y))
    subj = RNG.normal(0, 3, 10)
    Y = np.array([[round(10 + s + j * 0.8 + RNG.normal(0, 0.5 + 1.2 * j), 2) for j in range(5)] for s in subj])
    sets.append(("n10_k5_nonspherical", Y))
    sets.append(("n6_k3_textbook_like", np.array([[45, 50, 55], [42, 42, 45], [36, 41, 43], [39, 35, 40], [51, 55, 59], [44, 49, 56]], float)))
    for name, Y in sets:
        n, k = Y.shape
        df = pd.DataFrame([(i, "t%d" % j, Y[i, j]) for i in range(n) for j in range(k)], columns=["subject", "cond", "y"])
        res = AnovaRM(df, "y", "subject", within=["cond"]).fit()
        row = res.anova_table.iloc[0]
        F, d1, d2 = row["F Value"], row["Num DF"], row["Den DF"]
        # SS partition
        gm = Y.mean()
        ss_tr = n * ((Y.mean(axis=0) - gm) ** 2).sum()
        ss_subj = k * ((Y.mean(axis=1) - gm) ** 2).sum()
        ss_tot = ((Y - gm) ** 2).sum()
        ss_err = ss_tot - ss_tr - ss_subj
        exp = {"F": F, "df1": d1, "df2": d2, "p": row["Pr > F"],
               "ss_treatment": ss_tr, "ss_subjects": ss_subj, "ss_error": ss_err, "ss_total": ss_tot,
               "ms_treatment": ss_tr / (k - 1), "ms_error": ss_err / ((n - 1) * (k - 1))}
        sph = sphericity(Y)
        exp.update(sph)
        for key, eps in (("gg", sph["gg_epsilon"]), ("hf", sph["hf_epsilon"])):
            exp["p_" + key] = st.f.sf(F, eps * d1, eps * d2)
            exp["df1_" + key], exp["df2_" + key] = eps * d1, eps * d2
        fr = st.friedmanchisquare(*Y.T)
        exp["friedman_chi2"], exp["friedman_p"] = fr.statistic, fr.pvalue
        cases.append(case(name, {"Y_subjects_by_conditions": Y.tolist()}, exp, TOL_P))
    write("anova_rm", "statsmodels.stats.anova.AnovaRM; epsilons and Mauchly computed here from the sample covariance matrix", cases, notes=[
        "S = k x k sample covariance (n-1) of the conditions; C = (k-1) x k orthonormal Helmert contrasts; Sc = C S C'.",
        "gg_epsilon = tr(Sc)^2 / ((k-1) tr(Sc^2)); equals the Box (1954) double-centred formula k^2 (mean diag S - mean S)^2 / ((k-1)(sum s_ij^2 - 2k sum rowmean_i^2 + k^2 mean^2)) (asserted).",
        "hf_epsilon = min(1, (n(k-1)gg - 2)/((k-1)(n-1-(k-1)gg))). General form with g groups and N subjects (Lecoutre 1991): ((N-g+1)(k-1)gg - 2)/((k-1)(N-g-(k-1)gg)); the original Huynh-Feldt 1976 numerator used N instead of N-g+1, identical when g=1.",
        "Mauchly W = det(Sc)/(tr(Sc)/(k-1))^(k-1); chi2 = -(n-1)(1 - (2p^2+p+2)/(6p(n-1))) ln W, p=k-1, df = p(p+1)/2 - 1.",
        "p_gg / p_hf: F referred to F(eps*(k-1), eps*(n-1)(k-1)).",
    ])


# ================================================================= Kruskal-Wallis, Friedman, Dunn
def dunn(groups):
    allv = np.concatenate([np.asarray(g, float) for g in groups])
    N = allv.size
    ranks = st.rankdata(allv)
    _, t = np.unique(allv, return_counts=True)
    tie = (t ** 3 - t).sum() / (12 * (N - 1))
    idx = np.cumsum([0] + [len(g) for g in groups])
    rbar = [ranks[idx[i]:idx[i + 1]].mean() for i in range(len(groups))]
    pairs, z, p = [], [], []
    for i, j in itertools.combinations(range(len(groups)), 2):
        se = math.sqrt((N * (N + 1) / 12 - tie) * (1 / len(groups[i]) + 1 / len(groups[j])))
        zz = (rbar[i] - rbar[j]) / se
        pairs.append([i, j]); z.append(zz); p.append(2 * st.norm.sf(abs(zz)))
    return {"mean_ranks": rbar, "pairs": pairs, "z": z, "p_raw": p,
            "p_bonferroni": multipletests(p, method="bonferroni")[1], "p_holm": multipletests(p, method="holm")[1],
            "p_sidak": multipletests(p, method="sidak")[1]}


def friedman_dunn(Y):
    n, k = Y.shape
    R = np.apply_along_axis(st.rankdata, 1, Y)
    rbar = R.mean(axis=0)
    se = math.sqrt(k * (k + 1) / (6 * n))
    pairs, z, p = [], [], []
    for i, j in itertools.combinations(range(k), 2):
        zz = (rbar[i] - rbar[j]) / se
        pairs.append([i, j]); z.append(zz); p.append(2 * st.norm.sf(abs(zz)))
    return {"rank_sums": R.sum(axis=0), "mean_ranks": rbar, "pairs": pairs, "z": z, "p_raw": p,
            "p_bonferroni": multipletests(p, method="bonferroni")[1], "p_holm": multipletests(p, method="holm")[1]}


def gen_nonparam_ksample():
    cases = []
    sets = [("kruskal_example_classic", [KW_X, KW_Y, KW_Z]),
            ("plantgrowth_classic", [PG_CTRL, PG_TRT1, PG_TRT2]),
            ("ties_heavy_k4", [[float(v) for v in RNG.integers(0, 6, m)] for m in (6, 9, 7, 12)]),
            ("unequal_n_k3", [r(RNG.exponential(1, 4), 3), r(RNG.exponential(2, 11), 3), r(RNG.exponential(3.5, 7), 3)])]
    for name, groups in sets:
        kw = st.kruskal(*groups)
        allv = np.concatenate(groups)
        N = allv.size
        ranks = st.rankdata(allv)
        idx = np.cumsum([0] + [len(g) for g in groups])
        H0 = 12 / (N * (N + 1)) * sum(ranks[idx[i]:idx[i + 1]].sum() ** 2 / len(groups[i]) for i in range(len(groups))) - 3 * (N + 1)
        _, t = np.unique(allv, return_counts=True)
        C = 1 - (t ** 3 - t).sum() / (N ** 3 - N)
        exp = {"H": kw.statistic, "H_uncorrected": H0, "tie_correction_factor": C, "df": len(groups) - 1, "p": kw.pvalue,
               "dunn": dunn(groups)}
        cases.append(case("kruskal_wallis/" + name, {"groups": [list(map(float, g)) for g in groups]}, exp, TOL_P))
    fsets = [("n6_k3_textbook_like", np.array([[45, 50, 55], [42, 42, 45], [36, 41, 43], [39, 35, 40], [51, 55, 59], [44, 49, 56]], float)),
             ("n12_k4_random", np.round(RNG.normal(0, 1, (12, 4)) + np.array([0, 0.3, 0.9, 0.4]), 3)),
             ("n8_k3_ties_within_rows", np.array([[1, 1, 2], [2, 3, 3], [1, 2, 3], [4, 4, 4], [2, 1, 3], [5, 6, 6], [1, 3, 2], [2, 2, 5]], float))]
    for name, Y in fsets:
        fr = st.friedmanchisquare(*Y.T)
        exp = {"chi2": fr.statistic, "df": Y.shape[1] - 1, "p": fr.pvalue, "dunn": friedman_dunn(Y)}
        cases.append(case("friedman/" + name, {"Y_subjects_by_conditions": Y.tolist()}, exp, TOL_P))
    write("kruskal_friedman_dunn", "scipy.stats.kruskal / friedmanchisquare; Dunn's test implemented here; statsmodels multipletests for adjustment", cases, notes=[
        "scikit-posthocs is not installed; Dunn (1964) is implemented directly.",
        "KW Dunn: z_ij = (Rbar_i - Rbar_j)/sqrt((N(N+1)/12 - sum(t^3-t)/(12(N-1))) (1/n_i + 1/n_j)), ranks over the pooled sample, p = 2(1-Phi(|z|)).",
        "Friedman Dunn: ranks within each subject; z_ij = (Rbar_i - Rbar_j)/sqrt(k(k+1)/(6n)) (no tie correction), as Prism documents.",
        "H uses the tie correction H = H0 / (1 - sum(t^3-t)/(N^3-N)); Friedman chi2 is scipy's tie-corrected statistic.",
        "Pairs are listed as [i, j] with i<j in group order; adjustments are over all k(k-1)/2 pairs.",
    ])


# ================================================================= multiple comparisons
_GLX, _GLW = np.polynomial.legendre.leggauss(400)
_ZN, _ZW = 9.5 * _GLX, 9.5 * _GLW          # fixed 400-point Gauss-Legendre on z in [-9.5, 9.5]
_PHIZ = np.exp(-0.5 * _ZN ** 2) / math.sqrt(2 * math.pi) * _ZW


def chi_over_sqrt_nu_pdf(u, nu):
    """density of u = sqrt(chi2_nu / nu)."""
    return np.exp(math.log(2) + (nu / 2) * math.log(nu / 2) - sp.gammaln(nu / 2) + (nu - 1) * np.log(u) - nu * u * u / 2)


def outer_quad(inner, nu):
    from scipy.integrate import quad
    if np.isinf(nu):
        return inner(1.0)
    # the density of u concentrates near 1 with sd ~ 1/sqrt(2 nu)
    sd = 1 / math.sqrt(2 * nu)
    pts = sorted({max(1e-9, 1 - 8 * sd), 1.0, 1 + 8 * sd})
    f = lambda u: inner(u) * chi_over_sqrt_nu_pdf(u, nu)
    total = 0.0
    edges = [0.0] + pts + [np.inf]
    for lo, hi in zip(edges[:-1], edges[1:]):
        total += quad(f, lo, hi, epsabs=1e-15, epsrel=1e-13, limit=400)[0]
    return total


def dunnett_cdf(c, lam, nu, two_sided=True):
    """P(max_i |T_i| <= c) (or max T_i <= c) for Dunnett's many-to-one statistics
    with product correlation rho_ij = lam_i lam_j: inner integral over the shared
    normal z by 400-point Gauss-Legendre, outer integral over u = s/sigma adaptively."""
    lam = np.asarray(lam)[:, None]
    s = np.sqrt(1 - lam ** 2)

    def inner(u):
        if two_sided:
            v = sp.ndtr((c * u - lam * _ZN) / s) - sp.ndtr((-c * u - lam * _ZN) / s)
        else:
            v = sp.ndtr((c * u - lam * _ZN) / s)
        return float((_PHIZ * np.prod(v, axis=0)).sum())

    return outer_quad(inner, nu)


def ptukey_quad(q, k, nu):
    """Studentized range cdf by the standard double integral (independent check)."""
    def inner(u):
        v = (sp.ndtr(_ZN) - sp.ndtr(_ZN - q * u)) ** (k - 1)
        return float(k * (_PHIZ * v).sum())
    return outer_quad(inner, nu)


def gen_multiple_comparisons():
    cases = []
    for name, groups in [("plantgrowth_classic", [PG_CTRL, PG_TRT1, PG_TRT2]),
                         ("unequal_n_k4", [r(RNG.normal(10, 2, 6), 3), r(RNG.normal(12, 2, 9), 3), r(RNG.normal(11, 2, 5), 3), r(RNG.normal(14, 2, 8), 3)])]:
        k = len(groups)
        tab = oneway_table(groups)
        msw, dfw = tab["ms_within"], tab["df_within"]
        qcrit = st.studentized_range.ppf(0.95, k, dfw)
        out = {"q_crit_0.95": qcrit, "ms_within": msw, "df_within": dfw, "pairs": [], "mean_diff": [], "q": [], "p_adj": [], "ci95": []}
        for i, j in itertools.combinations(range(k), 2):
            d = np.mean(groups[j]) - np.mean(groups[i])
            se = math.sqrt(msw / 2 * (1 / len(groups[i]) + 1 / len(groups[j])))
            out["pairs"].append([i, j]); out["mean_diff"].append(d); out["q"].append(abs(d) / se)
            out["p_adj"].append(st.studentized_range.sf(abs(d) / se, k, dfw))
            out["ci95"].append([d - qcrit * se, d + qcrit * se])
        th = st.tukey_hsd(*groups)
        for (i, j), p in zip(out["pairs"], out["p_adj"]):
            assert abs(th.pvalue[i, j] - p) < 1e-9
        vals = np.concatenate(groups)
        labs = np.concatenate([[str(i)] * len(g) for i, g in enumerate(groups)])
        sm_res = pairwise_tukeyhsd(vals, labs)
        out["statsmodels_p_adj"] = list(map(float, sm_res.pvalues))
        cases.append(case("tukey_kramer/" + name, {"groups": [list(map(float, g)) for g in groups]}, out,
                          {"rel": 1e-7, "abs": 1e-9, "fields": {"statsmodels_p_adj": {"rel": 0, "abs": 2e-3}}}))
        # Dunnett vs group 0
        ctrl, others = groups[0], groups[1:]
        sd = math.sqrt(msw)
        n0 = len(ctrl)
        lam = [math.sqrt(len(g) / (len(g) + n0)) for g in others]
        tstats = [(np.mean(g) - np.mean(ctrl)) / (sd * math.sqrt(1 / len(g) + 1 / n0)) for g in others]
        p2 = [1 - dunnett_cdf(abs(t), lam, dfw, True) for t in tstats]
        pg = [1 - dunnett_cdf(t, lam, dfw, False) for t in tstats]
        crit = opt.brentq(lambda c: dunnett_cdf(c, lam, dfw, True) - 0.95, 1.5, 6, xtol=1e-14)
        sres = st.dunnett(*others, control=ctrl, rng=np.random.default_rng(SEED))
        ci = sres.confidence_interval(0.95)
        dexp = {"t": tstats, "df": dfw, "lambda": lam, "p_two_sided": p2, "p_greater": pg, "critical_value_two_sided_0.95": crit,
                "mean_diff": [np.mean(g) - np.mean(ctrl) for g in others],
                "ci95": [[np.mean(g) - np.mean(ctrl) - crit * sd * math.sqrt(1 / len(g) + 1 / n0),
                          np.mean(g) - np.mean(ctrl) + crit * sd * math.sqrt(1 / len(g) + 1 / n0)] for g in others],
                "scipy_dunnett_p": list(map(float, sres.pvalue)), "scipy_dunnett_ci95": [list(map(float, ci.low)), list(map(float, ci.high))]}
        assert max(abs(a - b) for a, b in zip(p2, sres.pvalue)) < 2e-3, (p2, sres.pvalue)
        cases.append(case("dunnett/" + name, {"control": list(map(float, ctrl)), "treatments": [list(map(float, g)) for g in others]}, dexp,
                          {"rel": 1e-7, "abs": 1e-9, "fields": {"scipy_dunnett_p": {"rel": 0, "abs": 2e-3}, "scipy_dunnett_ci95": {"rel": 1e-3, "abs": 1e-3}}}))
    # p-value adjustment lists
    plists = [("textbook10", [0.010, 0.013, 0.014, 0.190, 0.350, 0.500, 0.630, 0.670, 0.750, 0.810]),
              ("with_ties_and_one", [0.04, 0.04, 0.001, 1.0, 0.2, 0.03]),
              ("tiny_p", [1e-12, 3e-9, 0.0004, 0.02, 0.049]),
              ("random20", sorted(RNG.uniform(0, 0.2, 20).round(6).tolist()))]
    for name, p in plists:
        exp = {}
        for m in ("bonferroni", "sidak", "holm", "holm-sidak", "fdr_bh", "fdr_by", "hommel"):
            exp[m.replace("-", "_")] = multipletests(p, method=m)[1]
        cases.append(case("p_adjust/" + name, {"p": p}, exp, TOL_EXACT))
    write("multiple_comparisons", "Tukey-Kramer from scipy.stats.studentized_range (checked vs scipy.stats.tukey_hsd); Dunnett by nested quadrature (checked vs scipy.stats.dunnett); statsmodels multipletests",
          cases, notes=[
              "Tukey: mean_diff = mean_j - mean_i for pair [i,j]; q = |diff|/sqrt(MSW/2 (1/n_i+1/n_j)); p_adj = 1 - Ptukey(q; k, df_within). This is Tukey-Kramer (unequal n).",
              "statsmodels_p_adj is statsmodels' qsturng interpolation, kept only as a loose cross-check (abs 2e-3).",
              "Dunnett: T_i = (mean_i - mean_0)/(s sqrt(1/n_i + 1/n_0)), s^2 = pooled MSW over ALL groups; corr rho_ij = lam_i lam_j, lam_i = sqrt(n_i/(n_i+n_0)).",
              "Dunnett p = 1 - P(max|T| <= |t_i|) by nested quadrature over z ~ N(0,1) and u = s/sigma ~ chi_nu/sqrt(nu) (1e-12 rel requested). scipy.stats.dunnett uses randomized QMC (Genz-Bretz), so its p is reproducible only to ~1e-3.",
              "p_adjust: statsmodels.stats.multitest.multipletests adjusted p-values (fdr_bh = Benjamini-Hochberg, fdr_by = Benjamini-Yekutieli).",
          ])


def gen_studentized_range():
    ks = list(range(2, 11))
    dfs = [5, 10, 20, 60, 1000]
    probs = [0.90, 0.95, 0.99]
    qgrid = [0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0]
    ppf, cdf = [], []
    for k in ks:
        for df in dfs:
            ppf.append({"k": k, "df": df, "p": [st.studentized_range.ppf(p, k, df) for p in probs]})
            cdf.append({"k": k, "df": df, "cdf": [st.studentized_range.cdf(q, k, df) for q in qgrid],
                        "sf": [st.studentized_range.sf(q, k, df) for q in qgrid]})
    for q, k, nu in ((3.0, 3, 10), (4.5, 6, 20), (2.0, 10, 5)):
        a, b = ptukey_quad(q, k, nu), st.studentized_range.cdf(q, k, nu)
        assert abs(a - b) < 1e-8, (q, k, nu, a, b)
    cases = [case("ppf_table", {"k": ks, "df": dfs, "probabilities": probs}, {"rows": ppf}, {"rel": 1e-7, "abs": 1e-9}),
             case("cdf_table", {"k": ks, "df": dfs, "q": qgrid}, {"rows": cdf}, {"rel": 1e-7, "abs": 1e-11})]
    write("studentized_range", "scipy.stats.studentized_range cdf/sf/ppf (spot-checked here against a nested-quadrature integral)", cases, notes=[
        "df=1000 stands in for infinity.",
        "Ptukey(q;k,nu) = int_0^inf f_u(u) int k phi(z)[Phi(z)-Phi(z-qu)]^(k-1) dz du with u = s/sigma ~ chi_nu/sqrt(nu).",
    ])


# ================================================================= contingency tables
def fisher_rxc(table):
    """Exact two-sided Fisher test for an r x c table by full enumeration of all
    tables with the observed margins. P(table) = prod(r_i!) prod(c_j!) / (N! prod n_ij!).
    p = sum of P over tables with P <= P_obs * (1 + 1e-7) (R fisher.test convention)."""
    T = np.asarray(table, int)
    rs, cs = T.sum(axis=1), T.sum(axis=0)
    N = int(T.sum())
    const = sum(sp.gammaln(v + 1) for v in rs) + sum(sp.gammaln(v + 1) for v in cs) - sp.gammaln(N + 1)

    def logp(cells):
        return const - sum(sp.gammaln(v + 1) for v in cells)

    lp_obs = logp(T.ravel())
    R, C = T.shape
    probs = []

    def fill(row, colrem, acc):
        if row == R - 1:
            probs.append(logp(acc + list(colrem)))
            return
        # all compositions of rs[row] into C cells bounded by colrem
        def comp(j, left, cur):
            if j == C - 1:
                if left <= colrem[j]:
                    yield cur + [left]
                return
            for v in range(min(left, colrem[j]) + 1):
                yield from comp(j + 1, left - v, cur + [v])
        for rowvals in comp(0, int(rs[row]), []):
            fill(row + 1, [colrem[j] - rowvals[j] for j in range(C)], acc + rowvals)

    fill(0, [int(v) for v in cs], [])
    lps = np.array(probs)
    total = np.exp(lps).sum()
    pval = np.exp(lps[lps <= lp_obs + math.log1p(1e-7)]).sum()
    return {"p_two_sided": min(1.0, pval), "p_observed_table": math.exp(lp_obs), "n_tables": len(probs), "sum_prob_check": total}


def cochran_armitage(successes, totals, scores):
    x, n, t = (np.asarray(v, float) for v in (successes, totals, scores))
    N = n.sum()
    pbar = x.sum() / N
    T = (t * (x - n * pbar)).sum()
    V = pbar * (1 - pbar) * ((n * t ** 2).sum() - (n * t).sum() ** 2 / N)
    z = T / math.sqrt(V)
    return {"z": z, "chi2": z * z, "p_two_sided": 2 * st.norm.sf(abs(z)), "z_with_N_over_N_minus_1": z * math.sqrt((N - 1) / N)}


def gen_contingency():
    cases = []
    # chi-square independence
    for name, tab in [("r_chisq_example_classic", [[762, 327, 468], [484, 239, 477]]),
                      ("tea_tasting_2x2_classic", [[3, 1], [1, 3]]),
                      ("small_2x2", [[12, 5], [7, 15]]),
                      ("3x3", [[10, 6, 4], [5, 12, 8], [2, 7, 15]])]:
        exp = {}
        for corr in (False, True):
            c = st.chi2_contingency(tab, correction=corr)
            k = "yates" if corr else "pearson"
            exp[k] = {"chi2": c.statistic, "df": c.dof, "p": c.pvalue}
        exp["expected"] = st.chi2_contingency(tab).expected_freq
        g = st.chi2_contingency(tab, correction=False, lambda_="log-likelihood")
        exp["g_test"] = {"G": g.statistic, "p": g.pvalue}
        cases.append(case("chi2_independence/" + name, {"table": tab}, exp, TOL_P,
                          note="Yates correction applies only to 2x2 (scipy applies it when dof==1)."))
    # goodness of fit
    for name, obs, ex in [("mendel_peas_classic", [315, 108, 101, 32], [9 / 16, 3 / 16, 3 / 16, 1 / 16]),
                          ("uniform_die", [8, 12, 9, 15, 6, 10], None)]:
        n = sum(obs)
        f_exp = [p * n for p in ex] if ex else None
        c = st.chisquare(obs, f_exp=f_exp)
        cases.append(case("chi2_gof/" + name, {"observed": obs, "expected_proportions": ex}, {"chi2": c.statistic, "df": len(obs) - 1, "p": c.pvalue}, TOL_P))
    # Fisher 2x2 + odds ratios + relative risk
    for name, tab in [("tea_tasting_classic", [[3, 1], [1, 3]]),
                      ("moderate", [[12, 5], [7, 15]]),
                      ("zero_cell", [[0, 10], [6, 8]]),
                      ("larger", [[45, 55], [25, 75]]),
                      ("extreme_p", [[30, 1], [2, 40]])]:
        (a, b), (c, d) = tab
        exp = {}
        for alt in ("two-sided", "less", "greater"):
            exp["p_" + alt.replace("-", "_")] = st.fisher_exact(tab, alternative=alt).pvalue
        orc = st.contingency.odds_ratio(tab, kind="conditional")
        ci = orc.confidence_interval(0.95)
        exp["odds_ratio_conditional_mle"] = orc.statistic
        exp["odds_ratio_conditional_ci95"] = [ci.low, ci.high]
        ors = st.contingency.odds_ratio(tab, kind="sample")
        exp["odds_ratio_sample"] = ors.statistic
        if min(a, b, c, d) > 0:
            lor = math.log(a * d / (b * c))
            se = math.sqrt(1 / a + 1 / b + 1 / c + 1 / d)
            exp["odds_ratio_woolf_ci95"] = [math.exp(lor - Z975 * se), math.exp(lor + Z975 * se)]
            sci = ors.confidence_interval(0.95)
            exp["scipy_sample_or_ci95"] = [sci.low, sci.high]
            t22 = Table2x2(np.asarray(tab))
            assert abs(t22.oddsratio_confint()[0] - exp["odds_ratio_woolf_ci95"][0]) < 1e-10
            rr = (a / (a + b)) / (c / (c + d))
            serr = math.sqrt(1 / a - 1 / (a + b) + 1 / c - 1 / (c + d))
            exp["relative_risk"] = rr
            exp["relative_risk_katz_ci95"] = [rr * math.exp(-Z975 * serr), rr * math.exp(Z975 * serr)]
            rrs = st.contingency.relative_risk(a, a + b, c, c + d)
            assert abs(rrs.relative_risk - rr) < 1e-12
            rci = rrs.confidence_interval(0.95)
            assert abs(rci.low - exp["relative_risk_katz_ci95"][0]) < 1e-10
        exp["risk_difference"] = a / (a + b) - c / (c + d)
        cases.append(case("fisher_2x2/" + name, {"table": tab}, exp, TOL_P))
    # Fisher r x c by enumeration
    for name, tab in [("2x3", [[3, 1, 4], [1, 5, 2]]), ("3x3", [[2, 0, 3], [1, 4, 0], [0, 2, 5]]),
                      ("2x4", [[1, 2, 6, 0], [4, 3, 1, 5]]), ("3x2_from_2x3_transpose", [[3, 1], [1, 5], [4, 2]]),
                      ("2x2_matches_scipy", [[12, 5], [7, 15]])]:
        exp = fisher_rxc(tab)
        if np.shape(tab) == (2, 2):
            assert abs(exp["p_two_sided"] - st.fisher_exact(tab).pvalue) < 1e-12
        cases.append(case("fisher_rxc/" + name, {"table": tab}, exp, TOL_P))
    # McNemar
    for name, tab in [("r_mcnemar_example_classic", [[794, 86], [150, 570]]), ("small", [[10, 3], [9, 12]]), ("tiny_discordant", [[5, 1], [4, 6]])]:
        exp = {}
        for corr in (False, True):
            m = mcnemar(tab, exact=False, correction=corr)
            exp["cc" if corr else "nocc"] = {"chi2": m.statistic, "p": m.pvalue}
        m = mcnemar(tab, exact=True)
        exp["exact_binomial"] = {"statistic_min_discordant": m.statistic, "p": m.pvalue}
        cases.append(case("mcnemar/" + name, {"table": tab}, exp, TOL_P, note="table[0][1] and table[1][0] are the discordant pairs."))
    # Cochran-Armitage
    for name, x, n, t in [("dose_4_levels", [2, 5, 9, 14], [20, 20, 20, 20], [0, 1, 2, 3]),
                          ("unequal_n_scores", [15, 8, 4, 1], [40, 30, 35, 25], [1, 2, 4, 8])]:
        cases.append(case("cochran_armitage/" + name, {"successes": x, "totals": n, "scores": t}, cochran_armitage(x, n, t), TOL_P))
    write("contingency", "scipy.stats chi2_contingency/chisquare/fisher_exact/contingency.odds_ratio/relative_risk; statsmodels mcnemar/Table2x2; r x c Fisher and Cochran-Armitage implemented here",
          cases, notes=[
              "Fisher 2x2 one-sided: 'less' = H1 OR<1, 'greater' = H1 OR>1, table [[a,b],[c,d]], OR = ad/bc.",
              "odds_ratio_conditional_mle and its CI: conditional MLE of the noncentral hypergeometric (R fisher.test estimate/CI).",
              "odds_ratio_woolf_ci95 = exp(ln OR +/- 1.959964 sqrt(1/a+1/b+1/c+1/d)); relative_risk_katz_ci95 = RR exp(+/- z sqrt(1/a - 1/(a+b) + 1/c - 1/(c+d))); RR = (a/(a+b))/(c/(c+d)).",
              "fisher_rxc: exact enumeration of every table with the observed margins (n_tables given); two-sided p sums tables with P <= P_obs(1+1e-7) as R does.",
              "Cochran-Armitage z = sum t_i(x_i - n_i pbar)/sqrt(pbar(1-pbar)[sum n_i t_i^2 - (sum n_i t_i)^2/N]); chi2 = z^2 equals R prop.trend.test. z_with_N_over_N_minus_1 is the variant some texts use.",
          ])


# ================================================================= correlation
def spearman_exact(x, y):
    """Two-sided exact permutation p for Spearman rho by enumerating all n! orderings of y."""
    rx, ry = st.rankdata(x), st.rankdata(y)
    n = len(x)
    rho_obs = np.corrcoef(rx, ry)[0, 1]
    perms = np.array(list(itertools.permutations(ry)))
    rxc = rx - rx.mean()
    pc = perms - perms.mean(axis=1, keepdims=True)
    rho = (pc @ rxc) / (np.sqrt((pc ** 2).sum(axis=1)) * math.sqrt((rxc ** 2).sum()))
    eps = 1e-12
    return {"rho": rho_obs, "p_exact_two_sided": float(np.mean(np.abs(rho) >= abs(rho_obs) - eps)),
            "p_exact_greater": float(np.mean(rho >= rho_obs - eps)), "p_exact_less": float(np.mean(rho <= rho_obs + eps)),
            "n_permutations": len(perms)}


def gen_correlation():
    cases = []
    sets = [("anscombe1_classic", ANS_X, ANS_Y1),
            ("n5", [1.0, 2.0, 3.0, 4.0, 5.0], [2.1, 3.9, 6.2, 7.8, 9.9]),
            ("n7_with_ties", [1, 2, 2, 3, 4, 4, 5], [3, 1, 4, 4, 5, 9, 2]),
            ("n8_no_ties", r(RNG.normal(0, 1, 8), 3), None),
            ("n40_moderate", r(RNG.normal(0, 1, 40), 3), None),
            ("negative_strong_n12", list(range(1, 13)), [30.5, 29, 27.4, 25.1, 26, 22.3, 20, 19.4, 17.2, 15.5, 16.1, 12])]
    out = []
    for name, x, y in sets:
        if y is None:
            y = r(0.6 * np.asarray(x) + RNG.normal(0, 0.8, len(x)), 3)
        out.append((name, x, y))
    for name, x, y in out:
        x, y = list(map(float, x)), list(map(float, y))
        n = len(x)
        pr = st.pearsonr(x, y)
        ci = pr.confidence_interval(0.95)
        exp = {"pearson_r": pr.statistic, "pearson_p_two_sided": pr.pvalue, "pearson_ci95_fisher_z": [ci.low, ci.high],
               "pearson_p_greater": st.pearsonr(x, y, alternative="greater").pvalue,
               "r_squared": pr.statistic ** 2}
        sr = st.spearmanr(x, y)
        exp["spearman_rho"] = sr.statistic
        exp["spearman_p_t_approx_two_sided"] = sr.pvalue
        if n <= 8:
            ex = spearman_exact(x, y)
            exp.update({"spearman_" + k: v for k, v in ex.items() if k != "rho"})
        kt = st.kendalltau(x, y, variant="b")
        exp["kendall_tau_b"] = kt.statistic
        exp["kendall_p_two_sided_scipy_auto"] = kt.pvalue
        exp["kendall_p_asymptotic"] = st.kendalltau(x, y, method="asymptotic").pvalue
        if len(set(x)) == n and len(set(y)) == n:
            exp["kendall_p_exact"] = st.kendalltau(x, y, method="exact").pvalue
        exp["kendall_tau_c"] = st.kendalltau(x, y, variant="c").statistic
        cases.append(case(name, {"x": x, "y": y}, exp, TOL_P))
    write("correlation", "scipy.stats.pearsonr (+confidence_interval, Fisher z), spearmanr, kendalltau(variant='b'); exact Spearman by full enumeration here",
          cases, notes=[
              "Pearson CI: tanh(atanh(r) +/- 1.959964/sqrt(n-3)).",
              "Spearman p_t_approx: t = rho sqrt((n-2)/(1-rho^2)) on n-2 df (scipy). Exact p (n<=8): fraction of all n! permutations of y-ranks with |rho| >= |rho_obs| (ties kept as midranks).",
              "Kendall tau-b; p_exact only without ties (Kendall's recurrence); p_asymptotic uses the tie-corrected variance.",
          ])


# ================================================================= linear regression
def runs_test(resid_sorted_by_x):
    s = [v > 0 for v in resid_sorted_by_x if v != 0]
    n1, n2 = sum(s), len(s) - sum(s)
    runs = 1 + sum(1 for a, b in zip(s, s[1:]) if a != b)
    N = n1 + n2
    tot = math.comb(N, n1)

    def pmf(rr):
        if rr % 2 == 0:
            k = rr // 2
            return 2 * math.comb(n1 - 1, k - 1) * math.comb(n2 - 1, k - 1) / tot
        k = (rr - 1) // 2
        return (math.comb(n1 - 1, k) * math.comb(n2 - 1, k - 1) + math.comb(n1 - 1, k - 1) * math.comb(n2 - 1, k)) / tot

    p_le = sum(pmf(rr) for rr in range(2, runs + 1))
    mu = 2 * n1 * n2 / N + 1
    var = 2 * n1 * n2 * (2 * n1 * n2 - N) / (N * N * (N - 1))
    return {"runs": runs, "n_positive": n1, "n_negative": n2, "p_exact_too_few_runs": p_le,
            "expected_runs": mu, "z": (runs - mu) / math.sqrt(var)}


def deming(x, y, lam):
    x, y = np.asarray(x, float), np.asarray(y, float)
    mx, my = x.mean(), y.mean()
    sxx = ((x - mx) ** 2).sum()
    syy = ((y - my) ** 2).sum()
    sxy = ((x - mx) * (y - my)).sum()
    b = (syy - lam * sxx + math.sqrt((syy - lam * sxx) ** 2 + 4 * lam * sxy ** 2)) / (2 * sxy)
    return b, my - b * mx


def gen_linear_regression():
    cases = []
    sets = [("anscombe1_classic", ANS_X, ANS_Y1),
            ("n4_tiny", [1.0, 2.0, 3.0, 4.0], [2.0, 4.1, 5.9, 8.3]),
            ("n20_noisy", r(np.linspace(0, 10, 20), 4), None),
            ("negative_slope_n12_replicates", [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6], [9.8, 10.4, 8.7, 9.1, 7.2, 8.0, 6.6, 6.1, 5.0, 5.6, 3.9, 4.2])]
    for name, x, y in sets:
        if y is None:
            y = r(1.5 + 0.8 * np.asarray(x) + RNG.normal(0, 1.2, len(x)), 4)
        x, y = list(map(float, x)), list(map(float, y))
        n = len(x)
        X = sm.add_constant(np.asarray(x))
        m = sm.OLS(np.asarray(y), X).fit()
        lr = st.linregress(x, y)
        assert abs(lr.slope - m.params[1]) < 1e-10 * max(1, abs(lr.slope))
        ci = m.conf_int(0.05)
        exp = {"slope": m.params[1], "intercept": m.params[0], "se_slope": m.bse[1], "se_intercept": m.bse[0],
               "t_slope": m.tvalues[1], "p_slope": m.pvalues[1], "t_intercept": m.tvalues[0], "p_intercept": m.pvalues[0],
               "ci95_slope": list(ci[1]), "ci95_intercept": list(ci[0]), "r": lr.rvalue, "r_squared": m.rsquared,
               "adj_r_squared": m.rsquared_adj, "sy_x": math.sqrt(m.mse_resid), "ss_residual": m.ssr, "df_residual": m.df_resid,
               "F": m.fvalue, "p_F": m.f_pvalue, "x_intercept": -m.params[0] / m.params[1]}
        x0 = [min(x), float(np.mean(x)), max(x), max(x) + (max(x) - min(x)) * 0.5]
        pr = m.get_prediction(sm.add_constant(np.asarray(x0), has_constant="add")).summary_frame(alpha=0.05)
        exp["bands"] = {"x": x0, "fit": list(pr["mean"]), "ci95_mean_lower": list(pr["mean_ci_lower"]), "ci95_mean_upper": list(pr["mean_ci_upper"]),
                        "pi95_lower": list(pr["obs_ci_lower"]), "pi95_upper": list(pr["obs_ci_upper"])}
        order = np.argsort(x, kind="stable")
        exp["runs_test"] = runs_test(np.asarray(m.resid)[order])
        dm = {}
        for lam in (1.0, 2.0):
            b, a = deming(x, y, lam)
            jk = np.array([deming(np.delete(x, i), np.delete(y, i), lam) for i in range(n)])
            se = np.sqrt((n - 1) / n * ((jk - jk.mean(axis=0)) ** 2).sum(axis=0))
            dm["lambda_%g" % lam] = {"slope": b, "intercept": a, "se_slope_jackknife": se[0], "se_intercept_jackknife": se[1]}
        exp["deming"] = dm
        cases.append(case("simple/" + name, {"x": x, "y": y}, exp, {"rel": 1e-9, "abs": 1e-12}))
    # multiple regression on 3 predictors
    n = 25
    X3 = np.round(np.column_stack([RNG.normal(10, 2, n), RNG.uniform(0, 5, n), RNG.normal(0, 1, n)]), 3)
    y3 = np.round(2.0 + 0.5 * X3[:, 0] - 1.2 * X3[:, 1] + 0.3 * X3[:, 2] + RNG.normal(0, 0.7, n), 3)
    m = sm.OLS(y3, sm.add_constant(X3)).fit()
    ci = m.conf_int(0.05)
    exp = {"params": m.params, "se": m.bse, "t": m.tvalues, "p": m.pvalues, "ci95": ci.tolist(), "r_squared": m.rsquared,
           "adj_r_squared": m.rsquared_adj, "F": m.fvalue, "p_F": m.f_pvalue, "sy_x": math.sqrt(m.mse_resid), "ss_residual": m.ssr,
           "df_residual": m.df_resid, "vif": [1 / (1 - sm.OLS(X3[:, j], sm.add_constant(np.delete(X3, j, axis=1))).fit().rsquared) for j in range(3)]}
    cases.append(case("multiple/three_predictors_n25", {"X": X3.tolist(), "y": y3.tolist()}, exp, {"rel": 1e-9, "abs": 1e-12},
                      note="params order: intercept, x1, x2, x3"))
    write("linear_regression", "statsmodels OLS (+get_prediction) cross-checked with scipy.stats.linregress; runs test and Deming implemented here", cases, notes=[
        "bands: ci95_mean = confidence band of the regression line; pi95 = prediction band for a new observation; evaluated at min x, mean x, max x and an extrapolated point.",
        "runs_test: residuals ordered by x (stable sort, ties keep input order), zero residuals dropped; p_exact_too_few_runs = P(R <= runs) from the Swed-Eisenhart exact distribution (one-sided, as Prism reports); z = normal approximation without continuity.",
        "deming: lambda = var(error in y)/var(error in x); slope = (Syy - lam Sxx + sqrt((Syy - lam Sxx)^2 + 4 lam Sxy^2))/(2 Sxy); SEs by leave-one-out jackknife sqrt((n-1)/n sum (b_(i) - mean b_(.))^2). Prism's Deming SE method may differ; treat SE as approximate.",
        "vif_j = 1/(1 - R^2 of x_j on the other predictors).",
    ])


# ================================================================= nonlinear regression
def aicc(ss, n, nparam):
    K = nparam + 1
    return n * math.log(ss / n) + 2 * K + 2 * K * (K + 1) / (n - K - 1)


def nl_fit(f, x, y, p0, sigma=None):
    x, y = np.asarray(x, float), np.asarray(y, float)
    popt, pcov = opt.curve_fit(f, x, y, p0=p0, sigma=sigma, absolute_sigma=False, method="lm",
                               ftol=1e-15, xtol=1e-15, gtol=1e-15, maxfev=200000)
    # polish and confirm convergence from a perturbed start
    popt2, _ = opt.curve_fit(f, x, y, p0=np.asarray(popt) * 1.05, sigma=sigma, method="lm", ftol=1e-15, xtol=1e-15, gtol=1e-15, maxfev=200000)
    assert np.allclose(popt, popt2, rtol=1e-6, atol=1e-10), (popt, popt2)
    w = 1.0 if sigma is None else 1 / np.asarray(sigma) ** 2
    resid = y - f(x, *popt)
    ss = float((w * resid ** 2).sum())
    n, p = len(x), len(popt)
    se = np.sqrt(np.diag(pcov))
    tc = st.t.ppf(0.975, n - p)
    out = {"params": popt, "se": se, "ci95": [[a - tc * b, a + tc * b] for a, b in zip(popt, se)],
           "ss": ss, "df": n - p, "sy_x": math.sqrt(ss / (n - p)), "aicc": aicc(ss, n, p)}
    if sigma is None:
        out["r_squared"] = 1 - ss / float(((y - y.mean()) ** 2).sum())
    out["cov_scaled"] = pcov
    return out


def f_4pl(x, bottom, top, logec50, hill):
    return bottom + (top - bottom) / (1 + 10 ** ((logec50 - x) * hill))


def f_3pl(x, bottom, top, logec50):
    return f_4pl(x, bottom, top, logec50, 1.0)


def f_decay1(x, y0, plateau, k):
    return (y0 - plateau) * np.exp(-k * x) + plateau


def f_decay2(x, plateau, span_fast, k_fast, span_slow, k_slow):
    return plateau + span_fast * np.exp(-k_fast * x) + span_slow * np.exp(-k_slow * x)


def f_mm(x, vmax, km):
    return vmax * x / (km + x)


def f_hill(x, bmax, kd, h):
    return bmax * x ** h / (kd ** h + x ** h)


def f_gauss(x, amp, mean, sd):
    return amp * np.exp(-0.5 * ((x - mean) / sd) ** 2)


def gen_nonlinear():
    cases = []
    tol = {"rel": 1e-6, "abs": 1e-9}
    logx = np.repeat(np.round(np.linspace(-9, -4, 11), 3), 2)
    y4 = np.round(f_4pl(logx, 5, 95, -6.5, 0.8) + RNG.normal(0, 3, logx.size), 3)
    xd = np.round(np.linspace(0, 10, 16), 3)
    yd1 = np.round(f_decay1(xd, 100, 12, 0.45) + RNG.normal(0, 2.5, xd.size), 3)
    yd2 = np.round(f_decay2(xd, 5, 60, 1.8, 35, 0.18) + RNG.normal(0, 1.2, xd.size), 3)
    xm = np.array([0.5, 1, 2, 3, 5, 7.5, 10, 15, 20, 30, 40])
    ym = np.round(f_mm(xm, 120, 6) * (1 + RNG.normal(0, 0.04, xm.size)), 3)
    xh = np.array([0.1, 0.25, 0.5, 1, 2, 4, 8, 16, 32, 64])
    yh = np.round(f_hill(xh, 1500, 3.5, 1.6) + RNG.normal(0, 40, xh.size), 2)
    xg = np.round(np.linspace(-4, 8, 25), 3)
    yg = np.round(f_gauss(xg, 50, 2.2, 1.4) + RNG.normal(0, 1.5, xg.size), 3)
    specs = [("4pl_dose_response", "bottom + (top-bottom)/(1+10^((logec50-x)*hill))", ["bottom", "top", "logec50", "hill"], f_4pl, logx, y4, [0, 100, -6, 1]),
             ("one_phase_decay", "(y0-plateau)*exp(-k*x) + plateau", ["y0", "plateau", "k"], f_decay1, xd, yd1, [90, 10, 0.3]),
             ("two_phase_decay", "plateau + span_fast*exp(-k_fast*x) + span_slow*exp(-k_slow*x)", ["plateau", "span_fast", "k_fast", "span_slow", "k_slow"], f_decay2, xd, yd2, [3, 50, 1.0, 40, 0.1]),
             ("michaelis_menten", "vmax*x/(km+x)", ["vmax", "km"], f_mm, xm, ym, [100, 5]),
             ("hill", "bmax*x^h/(kd^h+x^h)", ["bmax", "kd", "h"], f_hill, xh, yh, [1000, 2, 1]),
             ("gaussian", "amp*exp(-0.5*((x-mean)/sd)^2)", ["amp", "mean", "sd"], f_gauss, xg, yg, [40, 2, 1])]
    fits = {}
    for name, eq, pn, f, x, y, p0 in specs:
        res = nl_fit(f, x, y, p0)
        fits[name] = res
        cases.append(case("fit/" + name, {"equation": eq, "param_names": pn, "x": x.tolist(), "y": y.tolist(), "p0": p0}, res, tol))
    # nested comparisons
    def compare(simple, complex_, n):
        F = ((simple["ss"] - complex_["ss"]) / (simple["df"] - complex_["df"])) / (complex_["ss"] / complex_["df"])
        d = complex_["aicc"] - simple["aicc"]
        return {"F": F, "df_num": simple["df"] - complex_["df"], "df_den": complex_["df"],
                "p": st.f.sf(F, simple["df"] - complex_["df"], complex_["df"]),
                "aicc_simple": simple["aicc"], "aicc_complex": complex_["aicc"],
                "delta_aicc_simple_minus_complex": simple["aicc"] - complex_["aicc"],
                "prob_complex_correct": math.exp(-0.5 * d) / (1 + math.exp(-0.5 * d))}
    f3 = nl_fit(f_3pl, logx, y4, [0, 100, -6])
    cases.append(case("compare/3pl_vs_4pl", {"x": logx.tolist(), "y": y4.tolist(), "simple": "4PL with hill fixed at 1", "complex": "4PL",
                                              "p0_simple": [0, 100, -6], "p0_complex": [0, 100, -6, 1]},
                      {"simple_fit": f3, "complex_fit": fits["4pl_dose_response"], **compare(f3, fits["4pl_dose_response"], logx.size)}, tol))
    cases.append(case("compare/one_vs_two_phase_decay", {"x": xd.tolist(), "y": yd2.tolist(), "p0_simple": [90, 5, 0.5], "p0_complex": [3, 50, 1.0, 40, 0.1]},
                      {"simple_fit": (s1 := nl_fit(f_decay1, xd, yd2, [90, 5, 0.5])), "complex_fit": fits["two_phase_decay"],
                       **compare(s1, fits["two_phase_decay"], xd.size)}, tol))
    # weighted 1/Y^2 (weights from the data) on Michaelis-Menten with proportional error
    wres = nl_fit(f_mm, xm, ym, [100, 5], sigma=np.abs(ym))
    cases.append(case("weighted/michaelis_menten_1_over_y2_data", {"x": xm.tolist(), "y": ym.tolist(), "p0": [100, 5], "weights": "1/Y_data^2"}, wres, tol,
                      note="minimizes sum((y-f)/y)^2; ss is the weighted SS; se from the covariance scaled by weighted SS/(n-p)."))
    # weighted by the curve: minimize sum((y-f)/f)^2
    ls = opt.least_squares(lambda p: (ym - f_mm(xm, *p)) / f_mm(xm, *p), [100, 5], method="lm", ftol=1e-15, xtol=1e-15, gtol=1e-15, max_nfev=100000)
    cases.append(case("weighted/michaelis_menten_1_over_y2_curve", {"x": xm.tolist(), "y": ym.tolist(), "p0": [100, 5], "weights": "1/Y_curve^2"},
                      {"params": ls.x, "ss_weighted": float((ls.fun ** 2).sum())}, tol,
                      note="minimizes sum(((y-f)/f)^2) directly (weights from the fitted curve)."))
    # robust fits with planted outliers
    yo = y4.copy()
    yo[[3, 15]] += np.array([45.0, -40.0])
    for loss, fs in (("cauchy", 3.0), ("soft_l1", 3.0)):
        rl = opt.least_squares(lambda p: yo - f_4pl(logx, *p), [0, 100, -6, 1], loss=loss, f_scale=fs, method="trf",
                               ftol=1e-15, xtol=1e-15, gtol=1e-15, max_nfev=100000)
        cases.append(case("robust/4pl_%s" % loss, {"x": logx.tolist(), "y": yo.tolist(), "p0": [0, 100, -6, 1], "loss": loss, "f_scale": fs,
                                                   "outlier_indices_planted": [3, 15]},
                          {"params": rl.x, "objective_sum_rho": float(2 * rl.cost)}, {"rel": 1e-5, "abs": 1e-8},
                          note="scipy least_squares minimizes 0.5*sum(f_scale^2 rho((r/f_scale)^2)); cauchy rho(z)=ln(1+z) (Lorentzian, FIXED scale); soft_l1 rho(z)=2(sqrt(1+z)-1). objective_sum_rho = 2*cost."))
    write("nonlinear_regression", "scipy.optimize.curve_fit(method='lm', absolute_sigma=False) at tight tolerances; least_squares for curve-weighted and robust fits",
          cases, notes=[
              "se = sqrt(diag(cov)), cov = (J'WJ)^-1 * SS/(n-p) (asymptotic); ci95 = param +/- t(0.975, n-p) se.",
              "aicc = N ln(SS/N) + 2K + 2K(K+1)/(N-K-1) with K = number of fitted parameters + 1 (Prism convention).",
              "compare: F = ((SS_s - SS_c)/(df_s - df_c))/(SS_c/df_c); prob_complex_correct = exp(-0.5 D)/(1+exp(-0.5 D)), D = AICc_complex - AICc_simple.",
              "r_squared = 1 - SS/SS_total (unweighted fits only).",
              "Parameter tolerance 1e-6 rel: MINPACK uses forward-difference Jacobians; analytic Jacobians reach the same minimum to about that level.",
          ])


# ================================================================= normality
def ad_pvalue(A2, n):
    a = A2 * (1 + 0.75 / n + 2.25 / n ** 2)
    if a >= 0.6:
        p = math.exp(1.2937 - 5.709 * a + 0.0186 * a * a)
    elif a >= 0.34:
        p = math.exp(0.9177 - 4.279 * a - 1.38 * a * a)
    elif a >= 0.2:
        p = 1 - math.exp(-8.318 + 42.796 * a - 59.938 * a * a)
    else:
        p = 1 - math.exp(-13.436 + 101.14 * a - 223.73 * a * a)
    return a, p


def gen_normality():
    cases = []
    sets = [("sleep_drug2_classic", SLEEP_2), ("normal_n20", r(RNG.normal(5, 2, 20), 4)), ("normal_n50", r(RNG.normal(0, 1, 50), 4)),
            ("lognormal_n30", r(RNG.lognormal(0, 0.9, 30), 4)), ("uniform_n100", r(RNG.uniform(0, 1, 100), 5)),
            ("n8_minimum_for_k2", [2.1, 3.4, 1.9, 5.6, 2.8, 3.1, 4.0, 2.5]), ("n3_shapiro_min", [1.0, 2.0, 4.0]),
            ("ties_rounded_n25", [float(v) for v in np.round(RNG.normal(10, 1.5, 25))])]
    for name, x in sets:
        a = np.asarray(x, float)
        n = a.size
        exp = {}
        sw = st.shapiro(a)
        exp["shapiro_W"], exp["shapiro_p"] = sw.statistic, sw.pvalue
        if n >= 8:
            k2 = st.normaltest(a)
            exp["dagostino_K2"], exp["dagostino_p"] = k2.statistic, k2.pvalue
            exp["skewtest_z"] = st.skewtest(a).statistic
            exp["kurtosistest_z"] = st.kurtosistest(a).statistic
        if n >= 4:
            ad = st.anderson(a, dist="norm")
            astar, pad = ad_pvalue(ad.statistic, n)
            exp.update({"anderson_A2": ad.statistic, "anderson_A2_star": astar, "anderson_p_dagostino_stephens": pad,
                        "anderson_scipy_critical_values": ad.critical_values, "anderson_scipy_significance_levels_pct": ad.significance_level})
            ks, pl = lilliefors(a, dist="norm", pvalmethod="table")
            _, pa = lilliefors(a, dist="norm", pvalmethod="approx")
            exp.update({"lilliefors_D": ks, "lilliefors_p_table": pl, "lilliefors_p_approx_dallal_wilkinson": pa})
        mu, sd = float(np.round(a.mean())), float(max(1.0, np.round(a.std(ddof=1))))
        kt = st.kstest(a, st.norm(mu, sd).cdf, method="exact")
        exp.update({"ks_specified_mu": mu, "ks_specified_sd": sd, "ks_D": kt.statistic, "ks_p_exact": kt.pvalue,
                    "ks_p_asymptotic": st.kstest(a, st.norm(mu, sd).cdf, method="asymp").pvalue})
        tol = {"rel": 1e-8, "abs": 1e-10, "fields": {"lilliefors_p_table": {"rel": 1e-6, "abs": 1e-6}, "lilliefors_p_approx_dallal_wilkinson": {"rel": 1e-6, "abs": 1e-6},
                                                       "shapiro_p": {"rel": 1e-6, "abs": 1e-7}, "shapiro_W": {"rel": 1e-7, "abs": 1e-8}}}
        cases.append(case(name, {"x": list(map(float, x))}, exp, tol))
    write("normality", "scipy.stats shapiro / normaltest / anderson / kstest(method='exact'); statsmodels lilliefors", cases, notes=[
        "Shapiro-Wilk: scipy wraps Royston's AS R94 (single precision Fortran polynomial coefficients), hence the looser tolerance.",
        "anderson_p_dagostino_stephens: A*^2 = A^2(1+0.75/n+2.25/n^2) then D'Agostino & Stephens (1986) Table 4.9 piecewise exponential (see research/04-algorithms.md).",
        "Lilliefors p_table = statsmodels table interpolation; p_approx = Dallal-Wilkinson (1986) analytic approximation (valid for p < 0.1).",
        "ks_*: one-sample KS against N(ks_specified_mu, ks_specified_sd) given a priori (NOT estimated).",
    ])


# ================================================================= outliers: Grubbs and ROUT
def grubbs_iterative(x, alpha=0.05):
    vals = list(map(float, x))
    idx = list(range(len(vals)))
    steps = []
    while len(vals) >= 3:
        a = np.asarray(vals)
        n = a.size
        m, s = a.mean(), a.std(ddof=1)
        dev = np.abs(a - m)
        k = int(np.argmax(dev))
        G = dev[k] / s
        tcrit = st.t.ppf(1 - alpha / (2 * n), n - 2)
        Gcrit = (n - 1) / math.sqrt(n) * math.sqrt(tcrit ** 2 / (n - 2 + tcrit ** 2))
        den = (n - 1) ** 2 - n * G * G
        tobs = math.sqrt(n * (n - 2) * G * G / den) if den > 0 else float("inf")
        p = min(1.0, 2 * n * st.t.sf(tobs, n - 2))
        steps.append({"n": n, "mean": m, "sd": s, "suspect_index": idx[k], "suspect_value": vals[k], "G": G, "G_critical": Gcrit, "p_approx": p,
                      "significant": bool(G > Gcrit)})
        if G <= Gcrit:
            break
        del vals[k]
        del idx[k]
    return {"steps": steps, "outlier_indices": [s_["suspect_index"] for s_ in steps if s_["significant"]]}


def rout_constant(x, Q=0.01):
    """ROUT (Motulsky & Brown 2006) for 'identify outliers in a column' (model Y = M, K = 1).
    1. Robust fit: minimize sum ln(1 + (r_i/RSDR)^2) over M, RSDR recomputed from the residuals
       at each iteration, starting from the median, until M changes by < 1e-13 relative.
    2. RSDR = P68 * N/(N-K), P68 = 68.27th percentile of |residuals| (numpy linear interpolation).
    3. Rank |r| ascending (i = 1..N). For i from int(0.7N)+1 ... N (farthest 30%), alpha_i = Q(N-(i-1))/N,
       t_i = |r_i|/RSDR, p_i = two-tailed t probability on N-K df. The first i (closest to the centre) with
       p_i < alpha_i marks point i and all farther points as outliers.
    """
    a = np.asarray(x, float)
    N, K = a.size, 1
    M = float(np.median(a))
    hist = []
    for _ in range(500):
        rsdr = np.percentile(np.abs(a - M), 68.27) * N / (N - K)
        res = opt.minimize_scalar(lambda m: np.log1p(((a - m) / rsdr) ** 2).sum(), bracket=(M - rsdr, M, M + rsdr), tol=1e-14)
        newM = float(res.x)
        hist.append(newM)
        if abs(newM - M) <= 1e-13 * max(1.0, abs(M)):
            M = newM
            break
        M = newM
    r_ = a - M
    p68 = float(np.percentile(np.abs(r_), 68.27))
    rsdr = p68 * N / (N - K)
    mad = float(np.median(np.abs(a - np.median(a))))
    order = np.argsort(np.abs(r_), kind="stable")
    flagged = []
    first = None
    for i in range(int(0.7 * N) + 1, N + 1):
        j = order[i - 1]
        t = abs(r_[j]) / rsdr
        p = 2 * st.t.sf(t, N - K)
        alpha_i = Q * (N - (i - 1)) / N
        if p < alpha_i:
            first = i
            break
    if first is not None:
        flagged = sorted(int(order[i - 1]) for i in range(first, N + 1))
    kept = np.delete(a, flagged)
    return {"robust_center": M, "p68_abs_residual": p68, "rsdr": rsdr, "rsdr_alt_1p4826_mad": 1.4826 * mad * N / (N - K),
            "outlier_indices": flagged, "mean_after_removal": kept.mean(), "sd_after_removal": kept.std(ddof=1), "Q": Q}


def gen_outliers():
    cases = []
    base = r(RNG.normal(50, 4, 20), 3)
    sets = [("one_planted_n20", base[:19] + [85.0]),
            ("two_planted_masking_n20", base[:18] + [80.0, 82.0]),
            ("clean_n15", r(RNG.normal(10, 1, 15), 3)),
            ("low_and_high_n25", r(RNG.normal(100, 5, 23), 3) + [60.0, 141.0]),
            ("textbook_small_n8", [2.1, 2.4, 2.2, 2.3, 2.5, 2.2, 2.4, 5.9])]
    for name, x in sets:
        exp = {"grubbs": grubbs_iterative(x), "rout_Q1pct": rout_constant(x, 0.01), "rout_Q0p1pct": rout_constant(x, 0.001),
               "rout_Q5pct": rout_constant(x, 0.05)}
        cases.append(case(name, {"x": list(map(float, x))}, exp, {"rel": 1e-8, "abs": 1e-10, "fields": {"robust_center": {"rel": 1e-9, "abs": 1e-9}}}))
    write("outliers", "Grubbs (two-sided, iterative) and ROUT (Motulsky & Brown 2006, fit-a-constant case) implemented here", cases, notes=[
        "Grubbs: G = max|x - mean|/sd (n-1); G_crit = (n-1)/sqrt(n) sqrt(t^2/(n-2+t^2)), t = t(1-alpha/(2n), n-2); p_approx = min(1, 2n P(T_{n-2} > t_obs)), t_obs = sqrt(n(n-2)G^2/((n-1)^2-nG^2)); alpha = 0.05; iterate removing the suspect while significant. Indices refer to the input order.",
        "ROUT steps documented in rout_constant() docstring and research/04-algorithms.md section 16. The robust centre is the minimizer of the Lorentzian merit with adaptive RSDR; flagged indices come from the P68-based RSDR; rsdr_alt_1p4826_mad is recorded for comparison only.",
        "Q is a fraction (0.01 = Prism's default Q = 1%).",
    ])


# ================================================================= survival
def km_table(T, E):
    T, E = np.asarray(T, float), np.asarray(E, int)
    kmf = KaplanMeierFitter().fit(T, E)
    times = np.unique(T[E == 1])
    S, cum, rows = 1.0, 0.0, []
    for t in times:
        n = int((T >= t).sum())
        d = int(((T == t) & (E == 1)).sum())
        c = int(((T == t) & (E == 0)).sum())
        S *= 1 - d / n
        cum += d / (n * (n - d)) if n > d else float("inf")
        var = S * S * cum
        if 0 < S < 1 and np.isfinite(cum):
            th = math.log(-math.log(S))
            se_th = math.sqrt(cum) / abs(math.log(S))
            lo, hi = S ** math.exp(Z975 * se_th), S ** math.exp(-Z975 * se_th)
        else:
            lo = hi = None
        lin = [max(0.0, S - Z975 * math.sqrt(var)), min(1.0, S + Z975 * math.sqrt(var))] if np.isfinite(var) else [None, None]
        rows.append({"time": t, "n_at_risk": n, "events": d, "censored_at_time": c, "survival": S,
                     "greenwood_var": var if np.isfinite(var) else None, "se": math.sqrt(var) if np.isfinite(var) else None,
                     "ci95_loglog": [lo, hi], "ci95_linear_clipped": lin})
    # cross-check with lifelines
    sf = kmf.survival_function_.iloc[:, 0]
    ci = kmf.confidence_interval_
    for row in rows:
        assert abs(sf.loc[row["time"]] - row["survival"]) < 1e-12
        if row["ci95_loglog"][0] is not None:
            assert abs(ci.loc[row["time"]].iloc[0] - row["ci95_loglog"][0]) < 1e-9, (ci.loc[row["time"]], row)
    return rows, kmf.median_survival_time_


def logrank_parts(T1, E1, T2, E2):
    T = np.r_[T1, T2].astype(float)
    E = np.r_[E1, E2].astype(int)
    G = np.r_[np.ones(len(T1)), np.zeros(len(T2))]
    O1 = E1v = V = 0.0
    O2 = E2v = 0.0
    for t in np.unique(T[E == 1]):
        at = T >= t
        n, n1 = at.sum(), (at & (G == 1)).sum()
        d = ((T == t) & (E == 1)).sum()
        d1 = ((T == t) & (E == 1) & (G == 1)).sum()
        e1 = d * n1 / n
        O1 += d1; E1v += e1
        O2 += d - d1; E2v += d - e1
        if n > 1:
            V += d * (n1 / n) * (1 - n1 / n) * (n - d) / (n - 1)
    hr_lr = (O1 / E1v) / (O2 / E2v)
    se_lr = math.sqrt(1 / E1v + 1 / E2v)
    L = (O1 - E1v) / V
    return {"O1": O1, "E1": E1v, "O2": O2, "E2": E2v, "V": V, "chi2_mantel_haenszel": (O1 - E1v) ** 2 / V,
            "hr_logrank_OE": hr_lr, "hr_logrank_OE_ci95": [hr_lr * math.exp(-Z975 * se_lr), hr_lr * math.exp(Z975 * se_lr)],
            "hr_mantel_haenszel": math.exp(L), "hr_mantel_haenszel_ci95": [math.exp(L - Z975 / math.sqrt(V)), math.exp(L + Z975 / math.sqrt(V))]}


def gen_survival():
    cases = []
    def synth(n, scale, cens_scale):
        t = RNG.exponential(scale, n)
        c = RNG.exponential(cens_scale, n)
        T = np.ceil(np.minimum(t, c))      # integer days create ties
        E = (t <= c).astype(int)
        return T.tolist(), E.tolist()
    sets = [("gehan_leukemia_classic", GEHAN_6MP_T, GEHAN_6MP_E, GEHAN_PL_T, GEHAN_PL_E)]
    t1, e1 = synth(30, 20, 40)
    t2, e2 = synth(25, 12, 40)
    sets.append(("synthetic_ties_censoring_30_25", t1, e1, t2, e2))
    sets.append(("small_censor_at_end", [2, 3, 3, 5, 8, 9], [1, 1, 0, 1, 1, 0], [1, 1, 2, 4, 4, 6, 7], [1, 1, 1, 1, 0, 1, 1]))
    for name, T1, E1, T2, E2 in sets:
        exp = {}
        for g, (T, E) in (("group1", (T1, E1)), ("group2", (T2, E2))):
            rows, med = km_table(T, E)
            exp["km_" + g] = rows
            exp["median_" + g] = med
        for w, key in ((None, "logrank"), ("wilcoxon", "gehan_breslow_wilcoxon"), ("tarone-ware", "tarone_ware")):
            kw = {"weightings": w} if w else {}
            lr = logrank_test(T1, T2, event_observed_A=E1, event_observed_B=E2, **kw)
            exp[key] = {"chi2": lr.test_statistic, "p": lr.p_value}
        parts = logrank_parts(T1, E1, T2, E2)
        assert abs(parts["chi2_mantel_haenszel"] - exp["logrank"]["chi2"]) < 1e-9
        exp["logrank_components"] = parts
        df = pd.DataFrame({"T": list(T1) + list(T2), "E": list(E1) + list(E2), "group1": [1] * len(T1) + [0] * len(T2)})
        cph = CoxPHFitter().fit(df, "T", "E")
        s = cph.summary.loc["group1"]
        exp["cox_efron"] = {"coef": s["coef"], "se": s["se(coef)"], "hr": s["exp(coef)"], "hr_ci95": [s["exp(coef) lower 95%"], s["exp(coef) upper 95%"]],
                            "p_wald": s["p"], "log_likelihood": cph.log_likelihood_}
        cases.append(case(name, {"group1": {"time": list(map(float, T1)), "event": list(map(int, E1))},
                                 "group2": {"time": list(map(float, T2)), "event": list(map(int, E2))}}, exp,
                          {"rel": 1e-9, "abs": 1e-12, "fields": {"cox_efron": {"rel": 1e-6, "abs": 1e-8}}}))
    write("survival", "lifelines KaplanMeierFitter / logrank_test(weightings=None|'wilcoxon'|'tarone-ware') / CoxPHFitter; Greenwood, CIs, O/E and Mantel-Haenszel HR computed here",
          cases, notes=[
              "event = 1 for an observed event, 0 for censored. KM rows only at distinct event times; censored times step nothing.",
              "greenwood_var = S^2 sum d/(n(n-d)); ci95_loglog = S^exp(+/- 1.959964 sqrt(sum d/(n(n-d)))/|ln S|) (lifelines 'exponential Greenwood' = Prism 'asymmetrical'; asserted against lifelines); ci95_linear_clipped = S +/- 1.959964 se clipped to [0,1]. null where S is 0 or 1.",
              "median = smallest t with S(t) <= 0.5 (lifelines); 'Infinity' if never reached.",
              "logrank chi2 is the Mantel-Haenszel form (O1-E1)^2/V with hypergeometric V (asserted equal to lifelines).",
              "hr_logrank_OE = (O1/E1)/(O2/E2), CI exp(ln HR +/- z sqrt(1/E1+1/E2)); hr_mantel_haenszel = exp((O1-E1)/V), CI exp((O1-E1)/V +/- z/sqrt(V)). Group 1 relative to group 2.",
              "cox_efron: lifelines CoxPH with Efron tie handling (R coxph default is also Efron), covariate group1 = 1 for group 1.",
          ])


# ================================================================= ROC
def delong(pos, neg):
    """DeLong (1988) variance via the Sun & Xu (2014) midrank formulation."""
    pos, neg = np.asarray(pos, float), np.asarray(neg, float)
    m, n = pos.size, neg.size
    allr = st.rankdata(np.r_[pos, neg])
    rx, ry = st.rankdata(pos), st.rankdata(neg)
    auc = (allr[:m].sum() - m * (m + 1) / 2) / (m * n)
    v10 = (allr[:m] - rx) / n
    v01 = 1 - (allr[m:] - ry) / m
    var = v10.var(ddof=1) / m + v01.var(ddof=1) / n
    return auc, math.sqrt(var), v10, v01


def wilson(k, n, z=Z975):
    if n == 0:
        return [None, None]
    p = k / n
    den = 1 + z * z / n
    c = (p + z * z / (2 * n)) / den
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return [c - h, c + h]


def gen_roc():
    cases = []
    sets = []
    pos = np.round(RNG.normal(1.2, 1, 25), 1)
    neg = np.round(RNG.normal(0, 1, 30), 1)
    sets.append(("binormal_rounded_ties_25_30", pos, neg))
    sets.append(("small_no_ties_5_6", np.array([0.9, 0.8, 0.55, 0.7, 0.35]), np.array([0.1, 0.4, 0.3, 0.6, 0.2, 0.5])))
    sets.append(("hanley_mcneil_style_ratings", np.array([5] * 33 + [4] * 11 + [3] * 2 + [2] * 3 + [1] * 2, float),
                 np.array([5] * 3 + [4] * 2 + [3] * 6 + [2] * 11 + [1] * 33, float)))
    for name, pos, neg in sets:
        y = np.r_[np.ones(pos.size), np.zeros(neg.size)]
        s = np.r_[pos, neg]
        fpr, tpr, thr = roc_curve(y, s, drop_intermediate=False)
        auc = roc_auc_score(y, s)
        a2, se, v10, v01 = delong(pos, neg)
        assert abs(a2 - auc) < 1e-12
        A, m, n = auc, pos.size, neg.size
        q1, q2 = A / (2 - A), 2 * A * A / (1 + A)
        se_hm = math.sqrt((A * (1 - A) + (m - 1) * (q1 - A * A) + (n - 1) * (q2 - A * A)) / (m * n))
        sens = tpr
        spec = 1 - fpr
        J = sens + spec - 1
        jbest = int(np.argmax(J))
        tp = np.round(tpr * m).astype(int)
        tn = np.round(spec * n).astype(int)
        exp = {"auc": auc, "delong_se": se, "delong_ci95": [max(0.0, auc - Z975 * se), min(1.0, auc + Z975 * se)],
               "hanley_mcneil_se": se_hm, "hanley_mcneil_ci95": [max(0.0, auc - Z975 * se_hm), min(1.0, auc + Z975 * se_hm)],
               "p_auc_vs_0p5_delong_z": 2 * st.norm.sf(abs(auc - 0.5) / se),
               "thresholds": thr, "tpr": tpr, "fpr": fpr, "sensitivity": sens, "specificity": spec,
               "sensitivity_wilson_ci95": [wilson(int(k), m) for k in tp], "specificity_wilson_ci95": [wilson(int(k), n) for k in tn],
               "youden_J": J, "youden_best_index": jbest, "youden_best_threshold": thr[jbest], "youden_best_J": J[jbest],
               "delong_v10": v10, "delong_v01": v01}
        cases.append(case(name, {"scores_positive": pos.tolist(), "scores_negative": neg.tolist()}, exp, {"rel": 1e-10, "abs": 1e-12}))
    write("roc", "sklearn.metrics roc_curve(drop_intermediate=False) / roc_auc_score; DeLong (Sun & Xu 2014), Hanley-McNeil 1982 SE, Wilson score CIs implemented here",
          cases, notes=[
              "Classification rule: positive if score >= threshold. thresholds descend; the first is 'Infinity' (sklearn's sentinel: nothing called positive).",
              "DeLong: V10_i = (R_pooled(x_i) - R_pos(x_i))/n_neg, V01_j = 1 - (R_pooled(y_j) - R_neg(y_j))/n_pos, Var = var(V10)/n_pos + var(V01)/n_neg (n-1 denominators). CI = AUC +/- 1.959964 SE clipped to [0,1].",
              "Hanley-McNeil: Q1 = A/(2-A), Q2 = 2A^2/(1+A), SE^2 = (A(1-A) + (n_pos-1)(Q1-A^2) + (n_neg-1)(Q2-A^2))/(n_pos n_neg).",
              "Wilson CIs are the plain Wilson score interval (Prism uses a Wilson/Brown hybrid; may differ at the extremes).",
              "youden_best_index: first index (in the descending threshold order) attaining max J.",
          ])


# ================================================================= Bland-Altman
def gen_bland_altman():
    cases = []
    a_ = r(RNG.normal(100, 20, 30), 2)
    b_ = r(np.asarray(a_) * 1.04 - 2 + RNG.normal(0, 4, 30), 2)
    for name, A, B in (("pefr_wright_vs_mini_classic", BA_WRIGHT, BA_MINI), ("proportional_bias_n30", a_, b_), ("n5", [10.0, 12.0, 15.0, 9.0, 11.0], [10.5, 11.0, 15.5, 9.6, 10.4])):
        A, B = np.asarray(A, float), np.asarray(B, float)
        d = A - B
        mean = (A + B) / 2
        n = d.size
        bias, sd = d.mean(), d.std(ddof=1)
        tc = st.t.ppf(0.975, n - 1)
        lo, hi = bias - 1.96 * sd, bias + 1.96 * sd
        se_lim = sd * math.sqrt(1 / n + 1.96 ** 2 / (2 * (n - 1)))
        lr = st.linregress(mean, d)
        exp = {"bias": bias, "sd_diff": sd, "loa_lower": lo, "loa_upper": hi,
               "bias_ci95": [bias - tc * sd / math.sqrt(n), bias + tc * sd / math.sqrt(n)],
               "loa_se": se_lim, "loa_lower_ci95": [lo - tc * se_lim, lo + tc * se_lim], "loa_upper_ci95": [hi - tc * se_lim, hi + tc * se_lim],
               "loa_lower_exact_z": bias - Z975 * sd, "loa_upper_exact_z": bias + Z975 * sd,
               "proportional_bias_slope": lr.slope, "proportional_bias_intercept": lr.intercept, "proportional_bias_p": lr.pvalue,
               "proportional_bias_slope_se": lr.stderr,
               "differences": d, "averages": mean}
        cases.append(case(name, {"a": A.tolist(), "b": B.tolist()}, exp, {"rel": 1e-12, "abs": 1e-12, "fields": {"proportional_bias_p": TOL_P}}))
    write("bland_altman", "formulas of Bland & Altman 1986 (Lancet) and 1999 (Stat Methods Med Res) computed with numpy/scipy", cases, notes=[
        "d = a - b; bias = mean(d); sd with n-1; LoA = bias +/- 1.96 sd (the constant 1.96, as Prism does); *_exact_z variants use 1.959964...",
        "loa_se = sd sqrt(1/n + 1.96^2/(2(n-1))); CI of each limit = limit +/- t(0.975, n-1) loa_se; bias CI = bias +/- t sd/sqrt(n).",
        "proportional bias: OLS of d on (a+b)/2.",
    ])


# ================================================================= PCA
def gen_pca():
    cases = []
    n = 15
    Z = RNG.normal(0, 1, (n, 2))
    X = np.column_stack([Z[:, 0] * 3 + 10, Z[:, 0] * 2 + Z[:, 1] + 5, Z[:, 1] * 4 - 2 + RNG.normal(0, 0.5, n), RNG.normal(0, 1, n), Z[:, 0] - Z[:, 1] + RNG.normal(0, 0.3, n)])
    X = np.round(X, 3)
    for name, M in (("five_vars_n15", X), ("three_vars_n8", np.array([[2.5, 2.4, 1.2], [0.5, 0.7, 0.3], [2.2, 2.9, 1.1], [1.9, 2.2, 0.8], [3.1, 3.0, 1.6], [2.3, 2.7, 1.0], [2.0, 1.6, 0.9], [1.0, 1.1, 0.4]]))):
        Zs = (M - M.mean(axis=0)) / M.std(axis=0, ddof=1)
        p = PCA().fit(Zs)
        comps = p.components_.copy()
        scores = p.transform(Zs)
        for k in range(comps.shape[0]):
            j = int(np.argmax(np.abs(comps[k])))
            if comps[k, j] < 0:
                comps[k] *= -1
                scores[:, k] *= -1
        ev = p.explained_variance_
        R = np.corrcoef(M, rowvar=False)
        assert np.allclose(np.sort(np.linalg.eigvalsh(R))[::-1], ev, rtol=1e-10)
        exp = {"eigenvalues": ev, "explained_variance_ratio": p.explained_variance_ratio_, "cumulative_ratio": np.cumsum(p.explained_variance_ratio_),
               "eigenvectors": comps, "loadings_scaled": comps * np.sqrt(ev)[:, None], "scores": scores, "column_means": M.mean(axis=0),
               "column_sds": M.std(axis=0, ddof=1)}
        cases.append(case(name, {"X": M.tolist(), "standardize": "z-score with n-1 SD"}, exp, {"rel": 1e-9, "abs": 1e-11}))
    write("pca", "sklearn.decomposition.PCA on the column-standardized matrix (n-1 SD); eigenvalues checked against eig of the correlation matrix", cases, notes=[
        "eigenvectors: rows are components (sklearn components_ layout: component x variable). Sign convention: each row flipped so that its largest-|.| entry is positive (first such entry on ties); scores flipped accordingly.",
        "eigenvalues = explained_variance_ (n-1 denominator) = eigenvalues of the correlation matrix.",
        "loadings_scaled[k] = eigenvectors[k] * sqrt(eigenvalue_k) = correlations between variables and PC k.",
    ])


# ================================================================= distribution functions
mp = mpmath.mp
mp.dps = 40
PGRID = [1e-12, 1e-10, 1e-8, 1e-6, 1e-4, 1e-3, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95, 0.975, 0.99, 0.999]
SMALL_Q = [1e-12, 1e-8, 1e-4, 0.025]


def mpf(x):
    return mpmath.mpf(float(x))


def mp_root(fn, target, guess):
    """Solve fn(q) = target at 40 digits starting from a double-precision guess."""
    g = mpf(guess)
    h = abs(g) * mpf("1e-6") + mpf("1e-12")
    return mpmath.findroot(lambda q: fn(q) - target, (g - h, g + h), solver="secant", tol=mpf("1e-35"), maxsteps=200)


def mp_t_cdf(x, df):
    x, df = mpf(x) if not isinstance(x, mpmath.mpf) else x, mpf(df)
    tail = mpmath.betainc(df / 2, mpf(0.5), 0, df / (df + x * x), regularized=True) / 2
    return tail if x < 0 else 1 - tail


def mp_t_sf(x, df):
    return mp_t_cdf(-x if isinstance(x, mpmath.mpf) else -mpf(x), df)


def mp_f_cdf(x, a, b):
    x, a, b = (v if isinstance(v, mpmath.mpf) else mpf(v) for v in (x, a, b))
    return mpmath.betainc(a / 2, b / 2, 0, a * x / (a * x + b), regularized=True)


def mp_f_sf(x, a, b):
    x, a, b = (v if isinstance(v, mpmath.mpf) else mpf(v) for v in (x, a, b))
    return mpmath.betainc(b / 2, a / 2, 0, b / (b + a * x), regularized=True)


def mp_chi2_cdf(x, k):
    x = x if isinstance(x, mpmath.mpf) else mpf(x)
    return mpmath.gammainc(mpf(k) / 2, 0, x / 2, regularized=True)


def mp_chi2_sf(x, k):
    x = x if isinstance(x, mpmath.mpf) else mpf(x)
    return mpmath.gammainc(mpf(k) / 2, x / 2, mpmath.inf, regularized=True)


def mp_nct_cdf(x, df, nc):
    """P(T <= x), T = (Z + nc)/sqrt(V/df): integral over u = sqrt(V/df) of Phi(x u - nc) f(u) du."""
    x, df, nc = mpf(x), mpf(df), mpf(nc)
    lc = mpmath.log(2) + (df / 2) * mpmath.log(df / 2) - mpmath.loggamma(df / 2)
    f = lambda u: mpmath.ncdf(x * u - nc) * mpmath.exp(lc + (df - 1) * mpmath.log(u) - df * u * u / 2)
    sd = 1 / mpmath.sqrt(2 * df)
    pts = [0, max(mpf("1e-30"), 1 - 10 * sd), 1, 1 + 10 * sd, mpmath.inf]
    return mpmath.quad(f, pts)


class Tracker:
    def __init__(self):
        self.worst = {}

    def rec(self, table, ref, got):
        ref = float(ref)
        got = float(got)
        if ref == 0:
            e = abs(got)
        else:
            e = abs(got - ref) / abs(ref)
        self.worst[table] = max(self.worst.get(table, 0.0), e)
        return ref


def gen_distributions():
    T = Tracker()
    cases = []
    # normal
    xs = [-38, -20, -10, -5, -3, -1.959963984540054, -1, -0.5, 0, 0.5, 1, 2, 3, 5, 8]
    rows = [{"x": x, "cdf": T.rec("normal", mpmath.ncdf(mpf(x)), sp.ndtr(x)), "sf": T.rec("normal", mpmath.ncdf(-mpf(x)), st.norm.sf(x)),
             "pdf": T.rec("normal", mpmath.npdf(mpf(x)), st.norm.pdf(x)), "logcdf": T.rec("normal", mpmath.log(mpmath.ncdf(mpf(x))), st.norm.logcdf(x))} for x in xs]
    q = [{"p": p, "ppf": T.rec("normal_ppf", mp_root(mpmath.ncdf, mpf(p), st.norm.ppf(p)), st.norm.ppf(p))} for p in PGRID]
    qi = [{"q": p, "isf": T.rec("normal_ppf", -mp_root(mpmath.ncdf, mpf(p), st.norm.ppf(p)), st.norm.isf(p))} for p in SMALL_Q]
    cases.append(case("normal", {"x": xs, "p": PGRID, "q_upper": SMALL_Q}, {"values": rows, "ppf": q, "isf": qi}, {"rel": 1e-13, "abs": 0.0}))
    # Student t
    dfs = [1, 2, 3, 5, 10, 17.776473516178488, 30, 100, 1000, 0.5]
    xs = [-1e3, -40, -10, -4, -2, -1, -0.1, 0, 0.5, 1.5, 2.5, 6, 30]
    rows, qs = [], []
    for df in dfs:
        rows.append({"df": df, "x": xs, "cdf": [T.rec("t", mp_t_cdf(x, df), st.t.cdf(x, df)) for x in xs],
                     "sf": [T.rec("t", mp_t_sf(x, df), st.t.sf(x, df)) for x in xs]})
        pp = [p for p in PGRID if not (df < 1 and p < 1e-8)]
        qs.append({"df": df, "p": pp, "ppf": [T.rec("t_ppf", mp_root(lambda v: mp_t_cdf(v, df), mpf(p), st.t.ppf(p, df)), st.t.ppf(p, df)) for p in pp],
                   "q_upper": SMALL_Q, "isf": [T.rec("t_ppf", mp_root(lambda v: mp_t_sf(v, df), mpf(p), st.t.isf(p, df)), st.t.isf(p, df)) for p in SMALL_Q]})
    cases.append(case("student_t", {"df": dfs}, {"cdf_sf": rows, "quantiles": qs}, {"rel": 1e-12, "abs": 0.0}))
    # F
    pairs = [(1, 1), (2, 10), (5, 3), (10, 20), (3, 1000), (1.5, 7.3), (50, 50), (4, 0.5)]
    xs = [1e-4, 0.01, 0.1, 0.5, 1, 2, 5, 10, 50, 1000]
    rows, qs = [], []
    for a, b in pairs:
        rows.append({"df1": a, "df2": b, "x": xs, "cdf": [T.rec("F", mp_f_cdf(x, a, b), st.f.cdf(x, a, b)) for x in xs],
                     "sf": [T.rec("F", mp_f_sf(x, a, b), st.f.sf(x, a, b)) for x in xs]})
        pp = [1e-6, 0.01, 0.05, 0.5, 0.9, 0.95, 0.99]
        qs.append({"df1": a, "df2": b, "p": pp, "ppf": [T.rec("F_ppf", mp_root(lambda v: mp_f_cdf(v, a, b), mpf(p), st.f.ppf(p, a, b)), st.f.ppf(p, a, b)) for p in pp],
                   "q_upper": SMALL_Q, "isf": [T.rec("F_ppf", mp_root(lambda v: mp_f_sf(v, a, b), mpf(p), st.f.isf(p, a, b)), st.f.isf(p, a, b)) for p in SMALL_Q]})
    cases.append(case("f", {"pairs": pairs}, {"cdf_sf": rows, "quantiles": qs}, {"rel": 1e-12, "abs": 0.0}))
    # chi-square
    ks = [0.5, 1, 2, 3, 5, 10, 50, 100, 1000]
    rows, qs = [], []
    for k in ks:
        xs = [k * f for f in (1e-6, 0.01, 0.2, 0.5, 1, 1.5, 3, 10)] + [k + 60 * math.sqrt(2 * k)]
        rows.append({"df": k, "x": xs, "cdf": [T.rec("chi2", mp_chi2_cdf(x, k), st.chi2.cdf(x, k)) for x in xs],
                     "sf": [T.rec("chi2", mp_chi2_sf(x, k), st.chi2.sf(x, k)) for x in xs]})
        qs.append({"df": k, "p": PGRID, "ppf": [T.rec("chi2_ppf", mp_root(lambda v: mp_chi2_cdf(v, k), mpf(p), st.chi2.ppf(p, k)), st.chi2.ppf(p, k)) for p in PGRID],
                   "q_upper": SMALL_Q, "isf": [T.rec("chi2_ppf", mp_root(lambda v: mp_chi2_sf(v, k), mpf(p), st.chi2.isf(p, k)), st.chi2.isf(p, k)) for p in SMALL_Q]})
    cases.append(case("chi2", {"df": ks}, {"cdf_sf": rows, "quantiles": qs}, {"rel": 1e-12, "abs": 0.0}))
    # regularized incomplete beta I_x(a,b) and its complement
    bpairs = [(0.5, 1000), (1000, 0.5), (0.5, 0.5), (2, 3), (50, 50), (0.1, 10), (5000, 5000), (1e-3, 1e-3), (1, 1e5), (30, 0.01)]
    xs = [1e-10, 1e-6, 1e-4, 1e-3, 0.002, 0.01, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99, 0.999, 0.999999]
    rows = []
    for a, b in bpairs:
        rows.append({"a": a, "b": b, "x": xs,
                     "I": [T.rec("betainc", mpmath.betainc(mpf(a), mpf(b), 0, mpf(x), regularized=True), sp.betainc(a, b, x)) for x in xs],
                     "I_complement": [T.rec("betainc", mpmath.betainc(mpf(a), mpf(b), mpf(x), 1, regularized=True), sp.betaincc(a, b, x)) for x in xs]})
    binv = []
    for a, b in bpairs[:7]:
        pp = [1e-10, 1e-4, 0.05, 0.5, 0.95]
        binv.append({"a": a, "b": b, "p": pp, "x": [T.rec("betaincinv", mp_root(lambda v: mpmath.betainc(mpf(a), mpf(b), 0, v, regularized=True), mpf(p), sp.betaincinv(a, b, p)), sp.betaincinv(a, b, p)) for p in pp]})
    cases.append(case("incomplete_beta", {"pairs": bpairs}, {"rows": rows, "inverse": binv}, {"rel": 1e-12, "abs": 1e-300}))
    # regularized incomplete gamma P(a,x), Q(a,x)
    avals = [1e-3, 0.5, 1, 5, 30, 100, 1000, 1e5]
    rows = []
    for a in avals:
        xs = [1e-8, a * 1e-3, a * 0.5, a * 0.9, a, a * 1.1, a * 2, a * 10 + 10]
        rows.append({"a": a, "x": xs, "P": [T.rec("gammainc", mpmath.gammainc(mpf(a), 0, mpf(x), regularized=True), sp.gammainc(a, x)) for x in xs],
                     "Q": [T.rec("gammainc", mpmath.gammainc(mpf(a), mpf(x), mpmath.inf, regularized=True), sp.gammaincc(a, x)) for x in xs]})
    ginv = []
    for a in avals[1:7]:
        pp = [1e-10, 1e-4, 0.05, 0.5, 0.95, 0.999999]
        ginv.append({"a": a, "p": pp, "x": [T.rec("gammaincinv", mp_root(lambda v: mpmath.gammainc(mpf(a), 0, v, regularized=True), mpf(p), sp.gammaincinv(a, p)), sp.gammaincinv(a, p)) for p in pp]})
    lg = [{"x": x, "lgamma": T.rec("lgamma", mpmath.loggamma(mpf(x)), sp.gammaln(x)), "digamma": T.rec("digamma", mpmath.digamma(mpf(x)), sp.digamma(x))}
          for x in (1e-8, 0.5, 1, 1.5, 2, 2.5, 10, 100.5, 1e5, 1e10)]
    cases.append(case("incomplete_gamma", {"a": avals}, {"rows": rows, "inverse": ginv, "lgamma_digamma": lg}, {"rel": 1e-12, "abs": 1e-300}))
    # binomial
    rows = []
    for n, p in ((10, 0.5), (20, 0.1), (100, 0.37), (1000, 0.002), (5, 0.999)):
        ks_ = sorted({0, 1, n // 4, n // 2, int(n * p), n - 1, n})
        def pmf(k):
            return mpmath.binomial(n, k) * mpf(p) ** k * (1 - mpf(p)) ** (n - k)
        cdf = lambda k: mpmath.fsum(pmf(j) for j in range(0, k + 1))
        sf = lambda k: mpmath.fsum(pmf(j) for j in range(k + 1, n + 1))
        rows.append({"n": n, "p": p, "k": ks_, "pmf": [T.rec("binom", pmf(k), st.binom.pmf(k, n, p)) for k in ks_],
                     "cdf": [T.rec("binom", cdf(k), st.binom.cdf(k, n, p)) for k in ks_],
                     "sf": [T.rec("binom", sf(k), st.binom.sf(k, n, p)) if k < n else 0.0 for k in ks_]})
    cases.append(case("binomial", {}, {"rows": rows}, {"rel": 1e-12, "abs": 1e-300}))
    # hypergeometric (population M, K successes in population, N draws) -- exact rationals
    from fractions import Fraction
    rows = []
    for M, K, N in ((20, 7, 12), (50, 5, 10), (1000, 30, 100), (8, 4, 4)):
        lo, hi = max(0, N - (M - K)), min(K, N)
        pmf = {k: Fraction(math.comb(K, k) * math.comb(M - K, N - k), math.comb(M, N)) for k in range(lo, hi + 1)}
        ks_ = list(range(lo, hi + 1)) if hi - lo <= 12 else sorted({lo, lo + 1, 2, 3, 5, 8, hi - 1, hi})
        rows.append({"M_population": M, "K_successes": K, "N_draws": N, "k": ks_,
                     "pmf": [T.rec("hypergeom", float(pmf[k]), st.hypergeom.pmf(k, M, K, N)) for k in ks_],
                     "cdf": [T.rec("hypergeom", float(sum(pmf[j] for j in pmf if j <= k)), st.hypergeom.cdf(k, M, K, N)) for k in ks_],
                     "sf": [T.rec("hypergeom", float(sum(pmf[j] for j in pmf if j > k)), st.hypergeom.sf(k, M, K, N)) if k < hi else 0.0 for k in ks_]})
    cases.append(case("hypergeometric", {}, {"rows": rows}, {"rel": 1e-13, "abs": 1e-300},
                      note="scipy convention hypergeom(M, n, N): M population size, n = K successes, N draws."))
    # noncentral t
    rows = []
    for df, nc in ((3, 0.0), (10, 1.0), (10, -1.5), (30, 2.5), (5, 8.0), (50, 0.5)):
        xs = [-3, -1, 0, 1, 2, 4, 8, 15]
        ppv = [0.025, 0.5, 0.975]
        rows.append({"df": df, "nc": nc, "x": xs, "cdf": [T.rec("nct", mp_nct_cdf(x, df, nc), st.nct.cdf(x, df, nc)) for x in xs],
                     "p": ppv, "ppf": [T.rec("nct_ppf", mp_root(lambda v: mp_nct_cdf(v, df, nc), mpf(p), st.nct.ppf(p, df, nc)), st.nct.ppf(p, df, nc)) for p in ppv]})
    cases.append(case("noncentral_t", {}, {"rows": rows}, {"rel": 1e-10, "abs": 1e-15},
                      note="Reference by 40-digit quadrature of int_0^inf Phi(x u - nc) f(u) du, u = sqrt(chi2_df/df). Note the mpmath quadrature grid; values agree with scipy (Boost) to the scipy_max_rel_err shown."))
    notes = ["Expected values are 40-digit mpmath references rounded to double, not scipy output. scipy's worst relative error per table is recorded in scipy_max_rel_err (for orientation only).",
             "t cdf via 0.5*I_{df/(df+x^2)}(df/2, 1/2); F via I_{d1 x/(d1 x + d2)}(d1/2, d2/2); chi2 via regularized gamma P(k/2, x/2); sf computed directly (never 1-cdf).",
             "ppf/isf found by 40-digit secant root of cdf/sf. p down to 1e-12 is covered by isf on the small upper-tail grid.",
             "Studentized range lives in studentized_range.json."]
    write("distributions", "mpmath 40-digit references (betainc, gammainc, ncdf, loggamma, quad), cross-checked against scipy.special/scipy.stats",
          cases + [case("scipy_max_rel_err", {}, {k: v for k, v in T.worst.items()}, {"rel": 1.0, "abs": 1.0}, note="informational, not a test")], notes=notes)
