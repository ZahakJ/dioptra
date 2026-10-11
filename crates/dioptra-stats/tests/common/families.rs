//! One function per fixture family: runs every case and returns the filled [`Checker`].
//! Shared by the integration tests (which call `.finish()`) and by
//! `examples/correctness_report.rs` (which tabulates the accuracy achieved).

use super::*;
use dioptra_stats::descriptive::{self as d, QuantileMethod, Summary, SummaryOptions};
use dioptra_stats::dist::*;
use dioptra_stats::mannwhitney::{mann_whitney, MannWhitneyOptions, Method};
use dioptra_stats::normality;
use dioptra_stats::outliers::{grubbs, rout_column};
use dioptra_stats::special;
use dioptra_stats::ttest::{self, TTestOptions};
use dioptra_stats::wilcoxon::{paired as wilcoxon_paired, WilcoxonOptions, ZeroMethod};

fn f(row: &Value, k: &str) -> Vec<f64> {
    nums(&row[k])
}

// ------------------------------------------------------------------ special functions

pub fn incomplete_beta() -> Checker {
    let cases = cases("distributions");
    let c = case(&cases, "incomplete_beta");
    let t = tol(c, "I");
    let mut ck = Checker::new("incomplete beta + inverse");
    for row in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let (a, b) = (num(&row["a"]).unwrap(), num(&row["b"]).unwrap());
        let (ie, ce) = (f(row, "I"), f(row, "I_complement"));
        for (k, &x) in f(row, "x").iter().enumerate() {
            let (i, ic) = special::betainc_pair(a, b, x);
            let ctx = format!("I_x(a={a}, b={b}) at x={x}");
            ck.check(&ctx, "I", ie[k], i, t);
            ck.check(&ctx, "I_complement", ce[k], ic, t);
        }
    }
    for row in c["expected"]["inverse"].as_array().unwrap() {
        let (a, b) = (num(&row["a"]).unwrap(), num(&row["b"]).unwrap());
        for (p, x) in f(row, "p").into_iter().zip(f(row, "x")) {
            ck.check(
                &format!("betainc_inv(a={a}, b={b}, p={p})"),
                "x",
                x,
                special::betainc_inv(a, b, p),
                t,
            );
        }
    }
    ck
}

pub fn incomplete_gamma() -> Checker {
    let cases = cases("distributions");
    let c = case(&cases, "incomplete_gamma");
    let t = tol(c, "P");
    let mut ck = Checker::new("incomplete gamma + inverse, lgamma, digamma");
    for row in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let a = num(&row["a"]).unwrap();
        let (pe, qe) = (f(row, "P"), f(row, "Q"));
        for (k, &x) in f(row, "x").iter().enumerate() {
            let (p, q) = special::gammainc_pair(a, x);
            let ctx = format!("P/Q(a={a}) at x={x}");
            ck.check(&ctx, "P", pe[k], p, t);
            ck.check(&ctx, "Q", qe[k], q, t);
        }
    }
    for row in c["expected"]["inverse"].as_array().unwrap() {
        let a = num(&row["a"]).unwrap();
        for (p, x) in f(row, "p").into_iter().zip(f(row, "x")) {
            ck.check(
                &format!("gammainc_inv(a={a}, p={p})"),
                "x",
                x,
                special::gammainc_inv(a, p),
                t,
            );
        }
    }
    for row in c["expected"]["lgamma_digamma"].as_array().unwrap() {
        let x = num(&row["x"]).unwrap();
        ck.check(
            &format!("x={x}"),
            "lgamma",
            num(&row["lgamma"]).unwrap(),
            special::ln_gamma(x),
            t,
        );
        ck.check(
            &format!("x={x}"),
            "digamma",
            num(&row["digamma"]).unwrap(),
            special::digamma(x),
            t,
        );
    }
    ck
}

// ------------------------------------------------------------------ distributions

pub fn normal() -> Checker {
    let cases = cases("distributions");
    let c = case(&cases, "normal");
    let t = tol(c, "values");
    let n = Normal::standard();
    let mut ck = Checker::new("normal");
    ck.cases = 1;
    for v in c["expected"]["values"].as_array().unwrap() {
        let x = num(&v["x"]).unwrap();
        let ctx = format!("x={x}");
        ck.check(&ctx, "cdf", num(&v["cdf"]).unwrap(), n.cdf(x), t);
        ck.check(&ctx, "sf", num(&v["sf"]).unwrap(), n.sf(x), t);
        ck.check(&ctx, "pdf", num(&v["pdf"]).unwrap(), n.pdf(x), t);
        ck.check(&ctx, "logcdf", num(&v["logcdf"]).unwrap(), n.ln_cdf(x), t);
    }
    for v in c["expected"]["ppf"].as_array().unwrap() {
        let p = num(&v["p"]).unwrap();
        ck.check(
            &format!("p={p}"),
            "ppf",
            num(&v["ppf"]).unwrap(),
            n.ppf(p),
            t,
        );
    }
    for v in c["expected"]["isf"].as_array().unwrap() {
        let q = num(&v["q"]).unwrap();
        ck.check(
            &format!("q={q}"),
            "isf",
            num(&v["isf"]).unwrap(),
            n.isf(q),
            t,
        );
    }
    ck
}

fn cdf_sf_quantiles<D: Continuous>(
    ck: &mut Checker,
    label: &str,
    d: &D,
    cdf_row: &Value,
    q_row: &Value,
    t: Tol,
) {
    let (ce, se) = (f(cdf_row, "cdf"), f(cdf_row, "sf"));
    for (i, &x) in f(cdf_row, "x").iter().enumerate() {
        let ctx = format!("{label} x={x}");
        ck.check(&ctx, "cdf", ce[i], d.cdf(x), t);
        ck.check(&ctx, "sf", se[i], d.sf(x), t);
    }
    for (p, e) in f(q_row, "p").into_iter().zip(f(q_row, "ppf")) {
        ck.check(&format!("{label} p={p}"), "ppf", e, d.ppf(p), t);
    }
    for (q, e) in f(q_row, "q_upper").into_iter().zip(f(q_row, "isf")) {
        ck.check(&format!("{label} q={q}"), "isf", e, d.isf(q), t);
    }
}

/// t, F and chi-square tables (cdf, sf, ppf, isf).
pub fn t_f_chi2() -> Checker {
    let cases = cases("distributions");
    let mut ck = Checker::new("t, F, chi-square");
    for name in ["student_t", "f", "chi2"] {
        let c = case(&cases, name);
        let t = tol(c, "cdf_sf");
        let rows = c["expected"]["cdf_sf"].as_array().unwrap();
        let qrows = c["expected"]["quantiles"].as_array().unwrap();
        for (r, q) in rows.iter().zip(qrows) {
            ck.cases += 1;
            match name {
                "student_t" => {
                    let df = num(&r["df"]).unwrap();
                    cdf_sf_quantiles(
                        &mut ck,
                        &format!("t(df={df})"),
                        &StudentT::new(df).unwrap(),
                        r,
                        q,
                        t,
                    )
                }
                "f" => {
                    let (d1, d2) = (num(&r["df1"]).unwrap(), num(&r["df2"]).unwrap());
                    cdf_sf_quantiles(
                        &mut ck,
                        &format!("F({d1},{d2})"),
                        &FisherF::new(d1, d2).unwrap(),
                        r,
                        q,
                        t,
                    )
                }
                _ => {
                    let df = num(&r["df"]).unwrap();
                    cdf_sf_quantiles(
                        &mut ck,
                        &format!("chi2(df={df})"),
                        &ChiSquared::new(df).unwrap(),
                        r,
                        q,
                        t,
                    )
                }
            }
        }
    }
    ck
}

pub fn binomial_hypergeometric() -> Checker {
    let cases = cases("distributions");
    let mut ck = Checker::new("binomial, hypergeometric");
    let c = case(&cases, "binomial");
    let t = tol(c, "rows");
    for r in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let (n, p) = (num(&r["n"]).unwrap() as u64, num(&r["p"]).unwrap());
        let b = Binomial::new(n, p).unwrap();
        let (pm, cd, sf) = (f(r, "pmf"), f(r, "cdf"), f(r, "sf"));
        for (i, &k) in f(r, "k").iter().enumerate() {
            let ctx = format!("Binom(n={n}, p={p}) k={k}");
            ck.check(&ctx, "pmf", pm[i], b.pmf(k as i64), t);
            ck.check(&ctx, "cdf", cd[i], b.cdf(k as i64), t);
            ck.check(&ctx, "sf", sf[i], b.sf(k as i64), t);
        }
    }
    let c = case(&cases, "hypergeometric");
    let t = tol(c, "rows");
    for r in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let m = num(&r["M_population"]).unwrap() as u64;
        let kk = num(&r["K_successes"]).unwrap() as u64;
        let n = num(&r["N_draws"]).unwrap() as u64;
        let h = Hypergeometric::new(m, kk, n).unwrap();
        let (pm, cd, sf) = (f(r, "pmf"), f(r, "cdf"), f(r, "sf"));
        for (i, &k) in f(r, "k").iter().enumerate() {
            let ctx = format!("Hypergeom(M={m}, K={kk}, N={n}) k={k}");
            ck.check(&ctx, "pmf", pm[i], h.pmf(k as i64), t);
            ck.check(&ctx, "cdf", cd[i], h.cdf(k as i64), t);
            ck.check(&ctx, "sf", sf[i], h.sf(k as i64), t);
        }
    }
    ck
}

pub fn noncentral_t() -> Checker {
    let cases = cases("distributions");
    let c = case(&cases, "noncentral_t");
    let t = tol(c, "rows");
    let mut ck = Checker::new("noncentral t");
    for r in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let (df, nc) = (num(&r["df"]).unwrap(), num(&r["nc"]).unwrap());
        let d = NoncentralT::new(df, nc).unwrap();
        for (x, e) in f(r, "x").into_iter().zip(f(r, "cdf")) {
            ck.check(
                &format!("nct(df={df}, nc={nc}) x={x}"),
                "cdf",
                e,
                d.cdf(x),
                t,
            );
        }
        for (p, e) in f(r, "p").into_iter().zip(f(r, "ppf")) {
            ck.check(
                &format!("nct(df={df}, nc={nc}) p={p}"),
                "ppf",
                e,
                d.ppf(p),
                t,
            );
        }
    }
    ck
}

pub fn studentized_range() -> Checker {
    let cases = cases("studentized_range");
    let mut ck = Checker::new("studentized range");
    let c = case(&cases, "cdf_table");
    let t = tol(c, "rows");
    let qs = nums(&c["input"]["q"]);
    for r in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let (k, df) = (num(&r["k"]).unwrap() as usize, num(&r["df"]).unwrap());
        let d = StudentizedRange::new(k, df).unwrap();
        let (ce, se) = (f(r, "cdf"), f(r, "sf"));
        for (i, &q) in qs.iter().enumerate() {
            let (cdf, sf) = d.cdf_sf(q);
            let ctx = format!("ptukey(q={q}, k={k}, df={df})");
            ck.check(&ctx, "cdf", ce[i], cdf, t);
            ck.check(&ctx, "sf", se[i], sf, t);
        }
    }
    let c = case(&cases, "ppf_table");
    let t = tol(c, "rows");
    let ps = nums(&c["input"]["probabilities"]);
    for r in c["expected"]["rows"].as_array().unwrap() {
        ck.cases += 1;
        let (k, df) = (num(&r["k"]).unwrap() as usize, num(&r["df"]).unwrap());
        let d = StudentizedRange::new(k, df).unwrap();
        for (p, e) in ps.iter().zip(f(r, "p")) {
            ck.check(
                &format!("qtukey(p={p}, k={k}, df={df})"),
                "ppf",
                e,
                d.ppf(*p),
                t,
            );
        }
    }
    ck
}

/// The Dunnett distribution part of multiple_comparisons.json (p-values and critical
/// value given the t statistics, lambdas and df recorded there).
pub fn dunnett() -> Checker {
    let cases = cases("multiple_comparisons");
    let mut ck = Checker::new("Dunnett distribution");
    for c in cases
        .iter()
        .filter(|c| c["name"].as_str().unwrap().starts_with("dunnett/"))
    {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let d = Dunnett::new(nums(&e["lambda"]), num(&e["df"]).unwrap()).unwrap();
        let (p2, pg) = (nums(&e["p_two_sided"]), nums(&e["p_greater"]));
        for (i, &t) in nums(&e["t"]).iter().enumerate() {
            ck.check(
                name,
                &format!("p_two_sided[{i}]"),
                p2[i],
                d.p_value(t, true),
                tol(c, "p_two_sided"),
            );
            ck.check(
                name,
                &format!("p_greater[{i}]"),
                pg[i],
                d.p_value(t, false),
                tol(c, "p_greater"),
            );
        }
        let key = "critical_value_two_sided_0.95";
        ck.check(
            name,
            key,
            num(&e[key]).unwrap(),
            d.quantile(0.95, true),
            tol(c, key),
        );
    }
    ck
}

// ------------------------------------------------------------------ descriptive and tests

pub fn descriptive() -> Checker {
    let mut ck = Checker::new("descriptive");
    for c in cases("descriptive") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let x = nums(&c["input"]["x"]);
        let e = &c["expected"];
        let t = |fl: &str| tol(&c, fl);
        let s = Summary::compute(&x, &SummaryOptions::default()).unwrap();
        let s7 = Summary::compute(
            &x,
            &SummaryOptions {
                quantile_method: QuantileMethod::Linear,
                ci_level: 0.95,
            },
        )
        .unwrap();
        let st = Summary::compute(
            &x,
            &SummaryOptions {
                quantile_method: QuantileMethod::TukeyHinges,
                ci_level: 0.95,
            },
        )
        .unwrap();
        ck.check(&name, "n", num(&e["n"]).unwrap(), s.n as f64, t("n"));
        for (fl, g) in [
            ("sum", s.sum),
            ("min", s.min),
            ("max", s.max),
            ("range", s.range),
            ("mean", s.mean),
            ("sd", s.sd.unwrap()),
            ("variance", s.variance.unwrap()),
            ("sem", s.sem.unwrap()),
            ("median", s.median),
            ("cv", s.cv.unwrap()),
            ("skewness_g1_biased", s.skewness_g1.unwrap()),
            ("kurtosis_excess_g2_biased", s.kurtosis_g2.unwrap()),
            ("sd_population", d::sd_population(&x).unwrap()),
            ("skewness_G1_adjusted", s.skewness.unwrap_or(f64::NAN)),
            (
                "kurtosis_excess_G2_adjusted",
                s.kurtosis.unwrap_or(f64::NAN),
            ),
        ] {
            ck.check_value(&name, fl, &e[fl], g, t(fl));
        }
        let ci = s.ci_mean.unwrap();
        ck.check_vec(
            &name,
            "ci95_mean",
            &nums(&e["ci95_mean"]),
            &[ci.0, ci.1],
            t("ci95_mean"),
        );
        let q = &e["quartiles"];
        ck.check_vec(
            &name,
            "quartiles.linear_type7",
            &nums(&q["linear_type7"]),
            &[s7.q1, s7.q3],
            t("quartiles"),
        );
        ck.check_vec(
            &name,
            "quartiles.weibull_type6",
            &nums(&q["weibull_type6"]),
            &[s.q1, s.q3],
            t("quartiles"),
        );
        ck.check_vec(
            &name,
            "quartiles.tukey_hinges",
            &nums(&q["tukey_hinges"]),
            &[st.q1, st.q3],
            t("quartiles"),
        );
        let p = &e["percentiles_10_90"];
        ck.check_vec(
            &name,
            "percentiles.linear_type7",
            &nums(&p["linear_type7"]),
            &[s7.p10.unwrap(), s7.p90.unwrap()],
            t("percentiles_10_90"),
        );
        ck.check_vec(
            &name,
            "percentiles.weibull_type6",
            &nums(&p["weibull_type6"]),
            &[s.p10.unwrap(), s.p90.unwrap()],
            t("percentiles_10_90"),
        );
        if e["geometric_mean"].is_null() {
            if s.geometric_mean.is_some() {
                ck.fail(format!("{name}: geometric mean should be undefined"));
            }
        } else {
            ck.check_value(
                &name,
                "geometric_mean",
                &e["geometric_mean"],
                s.geometric_mean.unwrap(),
                t("geometric_mean"),
            );
            ck.check_value(
                &name,
                "geometric_sd_factor",
                &e["geometric_sd_factor"],
                s.geometric_sd_factor.unwrap(),
                t("geometric_sd_factor"),
            );
            let g = s.ci_geometric_mean.unwrap();
            ck.check_vec(
                &name,
                "ci95_geometric_mean",
                &nums(&e["ci95_geometric_mean"]),
                &[g.0, g.1],
                t("ci95_geometric_mean"),
            );
            ck.check_value(
                &name,
                "harmonic_mean",
                &e["harmonic_mean"],
                s.harmonic_mean.unwrap(),
                t("harmonic_mean"),
            );
        }
    }
    ck
}

pub fn t_tests() -> Checker {
    let mut ck = Checker::new("t tests");
    let o = TTestOptions::default();
    for c in cases("t_tests") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let kind = name.split('/').next().unwrap().to_string();
        let (inp, e) = (&c["input"], &c["expected"]);
        let t = |fl: &str| tol(&c, fl);
        let x = nums(&inp["x"]);
        if kind == "ratio_paired" {
            let r = ttest::ratio_paired(&x, &nums(&inp["y"]), &o).unwrap();
            ck.check_value(&name, "t", &e["t"], r.log_test.t, t("t"));
            ck.check_value(&name, "df", &e["df"], r.log_test.df, t("df"));
            ck.check_value(
                &name,
                "p_two_sided",
                &e["p_two_sided"],
                r.log_test.p_two_sided,
                t("p_two_sided"),
            );
            ck.check_value(
                &name,
                "mean_log10_ratio",
                &e["mean_log10_ratio"],
                r.log_test.estimate,
                t("mean_log10_ratio"),
            );
            ck.check_vec(
                &name,
                "ci95_mean_log10_ratio",
                &nums(&e["ci95_mean_log10_ratio"]),
                &[r.log_test.ci.0, r.log_test.ci.1],
                t("ci95_mean_log10_ratio"),
            );
            ck.check_value(
                &name,
                "geometric_mean_ratio",
                &e["geometric_mean_ratio"],
                r.geometric_mean_ratio,
                t("geometric_mean_ratio"),
            );
            ck.check_vec(
                &name,
                "ci95_ratio",
                &nums(&e["ci95_ratio"]),
                &[r.ci_ratio.0, r.ci_ratio.1],
                t("ci95_ratio"),
            );
            continue;
        }
        let (r, ci_shift) = match kind.as_str() {
            // SciPy's one-sample CI is the CI of the mean; ours is the CI of mean - mu.
            "one_sample" => {
                let mu = num(&inp["mu"]).unwrap();
                (ttest::one_sample(&x, mu, &o).unwrap(), mu)
            }
            "student" => (ttest::unpaired(&x, &nums(&inp["y"]), &o).unwrap(), 0.0),
            "welch" => (ttest::welch(&x, &nums(&inp["y"]), &o).unwrap(), 0.0),
            "paired" => (ttest::paired(&x, &nums(&inp["y"]), &o).unwrap(), 0.0),
            other => {
                ck.fail(format!("unknown t-test kind {other}"));
                continue;
            }
        };
        let mut want = vec![
            ("t", r.t),
            ("df", r.df),
            ("mean_difference", r.estimate),
            ("p_two_sided", r.p_two_sided),
            ("p_less", r.p_less),
            ("p_greater", r.p_greater),
        ];
        match kind.as_str() {
            "one_sample" => want.extend([
                ("cohens_d", r.cohens_d),
                ("hedges_g", r.hedges_g),
                ("hedges_g_approx", r.hedges_g_approx),
            ]),
            "student" | "welch" => want.extend([
                ("cohens_d_pooled", r.cohens_d),
                ("hedges_g_pooled", r.hedges_g),
                ("cohens_d_av", r.cohens_d_av.unwrap_or(f64::NAN)),
                ("pooled_sd", r.sd),
                ("se_difference", r.se),
            ]),
            "paired" => want.extend([
                ("sd_differences", r.sd),
                ("cohens_dz", r.cohens_d),
                ("hedges_gz", r.hedges_g),
            ]),
            _ => {}
        }
        for (fl, g) in want {
            if !e[fl].is_null() {
                ck.check_value(&name, fl, &e[fl], g, t(fl));
            }
        }
        ck.check_vec(
            &name,
            "ci95_difference",
            &nums(&e["ci95_difference"]),
            &[r.ci.0 + ci_shift, r.ci.1 + ci_shift],
            t("ci95_difference"),
        );
    }
    ck
}

pub fn mann_whitney_family() -> Checker {
    let mut ck = Checker::new("Mann-Whitney");
    // Exact discrete p: tightened to 1e-12 as the tolerance policy asks.
    let t_exact = Tol::new(1e-12, 1e-15);
    for c in cases("mann_whitney") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let (x, y) = (nums(&c["input"]["x"]), nums(&c["input"]["y"]));
        let e = &c["expected"];
        let t = |fl: &str| tol(&c, fl);
        let base = mann_whitney(
            &x,
            &y,
            &MannWhitneyOptions {
                method: Method::Asymptotic,
                ..Default::default()
            },
        )
        .unwrap();
        for (fl, g) in [
            ("U1", base.u1),
            ("U2", base.u2),
            ("U_min", base.u),
            ("rank_sum_x", base.rank_sum_x),
            ("z_tie_corrected_nocc", base.z),
            ("hodges_lehmann_shift_x_minus_y", base.hodges_lehmann),
        ] {
            ck.check_value(&name, fl, &e[fl], g, t(fl));
        }
        for cc in [true, false] {
            let r = mann_whitney(
                &x,
                &y,
                &MannWhitneyOptions {
                    method: Method::Asymptotic,
                    continuity: cc,
                    ..Default::default()
                },
            )
            .unwrap();
            let tag = if cc { "cc" } else { "nocc" };
            for (alt, g) in [
                ("two_sided", r.p_two_sided),
                ("less", r.p_less),
                ("greater", r.p_greater),
            ] {
                let fl = format!("p_asymptotic_{tag}_{alt}");
                ck.check_value(&name, &fl, &e[fl.as_str()], g, t(&fl));
            }
        }
        let auto = mann_whitney(&x, &y, &MannWhitneyOptions::default()).unwrap();
        if !e["p_exact_two_sided"].is_null() {
            let r = mann_whitney(
                &x,
                &y,
                &MannWhitneyOptions {
                    method: Method::Exact,
                    ..Default::default()
                },
            )
            .unwrap();
            if !auto.exact {
                ck.fail(format!(
                    "{name}: Auto should choose the exact method without ties"
                ));
            }
            for (alt, g) in [
                ("two_sided", r.p_two_sided),
                ("less", r.p_less),
                ("greater", r.p_greater),
            ] {
                let fl = format!("p_exact_{alt}");
                ck.check_value(&name, &fl, &e[fl.as_str()], g, t_exact);
            }
        } else {
            if auto.exact || !auto.method.contains("ties") {
                ck.fail(format!(
                    "{name}: ties must switch to the asymptotic method and say so"
                ));
            }
            if mann_whitney(
                &x,
                &y,
                &MannWhitneyOptions {
                    method: Method::Exact,
                    ..Default::default()
                },
            )
            .is_ok()
            {
                ck.fail(format!("{name}: forced exact with ties must be an error"));
            }
        }
    }
    ck
}

pub fn wilcoxon_family() -> Checker {
    let mut ck = Checker::new("Wilcoxon signed-rank");
    for c in cases("wilcoxon_signed_rank") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let (x, y) = (nums(&c["input"]["x"]), nums(&c["input"]["y"]));
        let e = &c["expected"];
        let t = |fl: &str| tol(&c, fl);
        for (zm, tag) in [
            (ZeroMethod::Wilcoxon, "wilcox"),
            (ZeroMethod::Pratt, "pratt"),
        ] {
            for cc in [false, true] {
                let o = WilcoxonOptions {
                    method: Method::Asymptotic,
                    zero_method: zm,
                    continuity: cc,
                    ..Default::default()
                };
                let r = wilcoxon_paired(&x, &y, &o).unwrap();
                let cct = if cc { "cc" } else { "nocc" };
                for (alt, g) in [
                    ("two_sided", r.p_two_sided),
                    ("less", r.p_less),
                    ("greater", r.p_greater),
                ] {
                    let fl = format!("p_approx_{tag}_{cct}_{alt}");
                    ck.check_value(&name, &fl, &e[fl.as_str()], g, t(&fl));
                }
                if !cc {
                    for (fl, g) in [
                        (format!("W_plus_{tag}"), r.w_plus),
                        (format!("W_minus_{tag}"), r.w_minus),
                        (
                            format!("statistic_scipy_two_sided_{tag}"),
                            r.w_plus.min(r.w_minus),
                        ),
                        // SciPy's two-sided z is computed from min(W+, W-), so it is -|z|.
                        (format!("z_{tag}"), -r.z.abs()),
                        ("n_zero".to_string(), r.n_zero as f64),
                        (
                            "hodges_lehmann_median_difference".to_string(),
                            r.hodges_lehmann,
                        ),
                        (
                            "hodges_lehmann_nonzero_only".to_string(),
                            r.hodges_lehmann_nonzero,
                        ),
                    ] {
                        ck.check_value(&name, &fl, &e[fl.as_str()], g, t(&fl));
                    }
                }
            }
        }
        if !e["p_exact_two_sided"].is_null() {
            let r = wilcoxon_paired(&x, &y, &WilcoxonOptions::default()).unwrap();
            if !r.exact {
                ck.fail(format!("{name}: Auto should be exact"));
            }
            let te = Tol::new(1e-12, 1e-15);
            for (alt, g) in [
                ("two_sided", r.p_two_sided),
                ("less", r.p_less),
                ("greater", r.p_greater),
            ] {
                let fl = format!("p_exact_{alt}");
                ck.check_value(&name, &fl, &e[fl.as_str()], g, te);
            }
            match r.ci {
                Some((lo, hi)) => {
                    if !(lo <= r.hodges_lehmann
                        && r.hodges_lehmann <= hi
                        && r.ci_achieved_level.unwrap_or(0.0) >= 0.95)
                    {
                        ck.fail(format!(
                            "{name}: CI does not bracket the estimate at >= 95%"
                        ));
                    }
                }
                // n = 5 cannot reach 95% coverage (smallest two-sided p is 1/16).
                None => {
                    if r.n > 5 {
                        ck.fail(format!("{name}: CI missing"));
                    }
                }
            }
        }
    }
    ck
}

pub fn normality_family() -> Checker {
    let mut ck = Checker::new("normality");
    for c in cases("normality") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let x = nums(&c["input"]["x"]);
        let e = &c["expected"];
        let t = |fl: &str| tol(&c, fl);
        let sw = normality::shapiro_wilk(&x).unwrap();
        ck.check_value(&name, "shapiro_W", &e["shapiro_W"], sw.w, t("shapiro_W"));
        ck.check_value(&name, "shapiro_p", &e["shapiro_p"], sw.p, t("shapiro_p"));
        if !e["dagostino_K2"].is_null() {
            let k = normality::dagostino_pearson(&x).unwrap();
            for (fl, g) in [
                ("dagostino_K2", k.k2),
                ("dagostino_p", k.p),
                ("skewtest_z", k.z_skewness),
                ("kurtosistest_z", k.z_kurtosis),
            ] {
                ck.check_value(&name, fl, &e[fl], g, t(fl));
            }
        }
        if !e["anderson_A2"].is_null() {
            let a = normality::anderson_darling(&x).unwrap();
            for (fl, g) in [
                ("anderson_A2", a.a2),
                ("anderson_A2_star", a.a2_star),
                ("anderson_p_dagostino_stephens", a.p),
            ] {
                ck.check_value(&name, fl, &e[fl], g, t(fl));
            }
            ck.check_vec(
                &name,
                "anderson_scipy_critical_values",
                &nums(&e["anderson_scipy_critical_values"]),
                &a.critical_values,
                t("anderson_scipy_critical_values"),
            );
            ck.check_vec(
                &name,
                "anderson_scipy_significance_levels_pct",
                &nums(&e["anderson_scipy_significance_levels_pct"]),
                &a.significance_levels,
                t("anderson_scipy_significance_levels_pct"),
            );
            let l = normality::lilliefors(&x).unwrap();
            for (fl, g) in [
                ("lilliefors_D", l.d),
                ("lilliefors_p_table", l.p_table),
                ("lilliefors_p_approx_dallal_wilkinson", l.p_dallal_wilkinson),
            ] {
                ck.check_value(&name, fl, &e[fl], g, t(fl));
            }
        }
        let (mu, sd) = (
            num(&e["ks_specified_mu"]).unwrap(),
            num(&e["ks_specified_sd"]).unwrap(),
        );
        let k = normality::kolmogorov_smirnov_normal(&x, mu, sd).unwrap();
        for (fl, g) in [
            ("ks_D", k.d),
            ("ks_p_exact", k.p_exact),
            ("ks_p_asymptotic", k.p_asymptotic),
        ] {
            ck.check_value(&name, fl, &e[fl], g, t(fl));
        }
    }
    ck
}

pub fn outliers_family() -> Checker {
    let mut ck = Checker::new("outliers (Grubbs, ROUT)");
    for c in cases("outliers") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let x = nums(&c["input"]["x"]);
        let e = &c["expected"];
        let t = |fl: &str| tol(&c, fl);
        let g = grubbs(&x, 0.05).unwrap();
        let steps = e["grubbs"]["steps"].as_array().unwrap();
        if steps.len() != g.steps.len() {
            ck.fail(format!(
                "{name}: Grubbs took {} steps, expected {}",
                g.steps.len(),
                steps.len()
            ));
        }
        for (i, (es, gs)) in steps.iter().zip(&g.steps).enumerate() {
            let ctx = format!("{name} grubbs step {i}");
            for (fl, v) in [
                ("n", gs.n as f64),
                ("mean", gs.mean),
                ("sd", gs.sd),
                ("suspect_index", gs.suspect_index as f64),
                ("suspect_value", gs.suspect_value),
                ("G", gs.g),
                ("G_critical", gs.g_critical),
                ("p_approx", gs.p),
                ("significant", if gs.significant { 1.0 } else { 0.0 }),
            ] {
                ck.check_value(&ctx, fl, &es[fl], v, t("grubbs"));
            }
        }
        let idx = |v: &Value| -> Vec<usize> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect()
        };
        if idx(&e["grubbs"]["outlier_indices"]) != g.outlier_indices {
            ck.fail(format!(
                "{name}: Grubbs outliers {:?}, expected {:?}",
                g.outlier_indices,
                idx(&e["grubbs"]["outlier_indices"])
            ));
        }
        for key in ["rout_Q1pct", "rout_Q0p1pct", "rout_Q5pct"] {
            let ex = &e[key];
            let r = rout_column(&x, num(&ex["Q"]).unwrap()).unwrap();
            let ctx = format!("{name} {key}");
            ck.check_value(
                &ctx,
                "robust_center",
                &ex["robust_center"],
                r.robust_center,
                t("robust_center"),
            );
            for (fl, v) in [
                ("p68_abs_residual", r.p68_abs_residual),
                ("rsdr", r.rsdr),
                ("rsdr_alt_1p4826_mad", r.rsdr_alt_mad),
                ("mean_after_removal", r.mean_after_removal),
                ("sd_after_removal", r.sd_after_removal),
            ] {
                ck.check_value(&ctx, fl, &ex[fl], v, t(key));
            }
            if idx(&ex["outlier_indices"]) != r.outlier_indices {
                ck.fail(format!(
                    "{ctx}: ROUT outliers {:?}, expected {:?}",
                    r.outlier_indices,
                    idx(&ex["outlier_indices"])
                ));
            }
        }
    }
    ck
}

// ------------------------------------------------------------------ NIST univariate

/// (name, mean, SD, r1) computed in exact rational arithmetic on the data as parsed into
/// doubles (Python `fractions`, 40-digit decimal square root).
pub const NIST_EXACT_ON_DOUBLES: [(&str, f64, f64, f64); 9] = [
    (
        "PiDigits",
        4.5348,
        2.867339060288708,
        -0.0035509928723797216,
    ),
    (
        "Lottery",
        518.9587155963303,
        291.6997274709691,
        -0.12094862296739287,
    ),
    ("Lew", -177.435, 277.3321680443161, -0.30730480060567944),
    ("Mavro", 2.001856, 0.0004291234540030854, 0.9379891834382589),
    (
        "Michelso",
        299.8524,
        0.07901054781905066,
        0.5351996686212636,
    ),
    ("NumAcc1", 10000002.0, 1.0, -0.5),
    ("NumAcc2", 1.2, 0.09999999999999998, -0.999),
    (
        "NumAcc3",
        1000000.2,
        0.1000000000349246,
        -0.9989999999994185,
    ),
    (
        "NumAcc4",
        10000000.2,
        0.10000000055879354,
        -0.9989999999906961,
    ),
];

/// LRE (McCullough 1998), capped at 15 digits.
pub fn lre(got: f64, cert: f64) -> f64 {
    let err = (got - cert).abs();
    let v = if err == 0.0 {
        15.0
    } else if cert == 0.0 {
        -err.log10()
    } else {
        -(err / cert.abs()).log10()
    };
    v.min(15.0)
}

/// One NIST univariate dataset scored: LREs vs the certified values and vs the exact
/// statistics of the double-rounded inputs.
pub struct NistRow {
    pub name: String,
    pub difficulty: String,
    pub certified: [f64; 3],
    pub exact_on_doubles: [f64; 3],
}

pub fn nist_univariate() -> Vec<NistRow> {
    NIST_EXACT_ON_DOUBLES
        .iter()
        .map(|&(name, em, es, er)| {
            let j = load(&format!("nist/{name}.json"));
            let y = nums(&j["data"]["y"]);
            let c = &j["certified"];
            let m = d::mean(&y).unwrap();
            let s = d::sd(&y).unwrap();
            let r = d::lag1_autocorrelation(&y).unwrap();
            NistRow {
                name: name.to_string(),
                difficulty: j["difficulty"].as_str().unwrap().to_string(),
                certified: [
                    lre(m, num(&c["mean"]["value"]).unwrap()),
                    lre(s, num(&c["sd_n_minus_1"]["value"]).unwrap()),
                    lre(r, num(&c["lag1_autocorrelation"]["value"]).unwrap()),
                ],
                exact_on_doubles: [lre(m, em), lre(s, es), lre(r, er)],
            }
        })
        .collect()
}

/// Every family, in report order.
pub fn all_families() -> Vec<Checker> {
    vec![
        incomplete_beta(),
        incomplete_gamma(),
        normal(),
        t_f_chi2(),
        binomial_hypergeometric(),
        noncentral_t(),
        studentized_range(),
        dunnett(),
        descriptive(),
        t_tests(),
        mann_whitney_family(),
        wilcoxon_family(),
        normality_family(),
        outliers_family(),
    ]
}
