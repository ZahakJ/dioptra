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
        anova_oneway_family(),
        anova_twoway_family(),
        anova_rm_family(),
        kruskal_friedman_dunn_family(),
        multiple_comparisons_family(),
        multiple_comparisons_sanity(),
        contingency_family(),
        correlation_family(),
        linear_regression_family(),
        nonlinear_family(),
        survival_family(),
        roc_family(),
        bland_altman_family(),
        pca_family(),
    ]
}

// ------------------------------------------------------------------ ANOVA and post hoc

use dioptra_stats::anova::{self, SsType, TwoWayOptions};
use dioptra_stats::posthoc::{self, Adjust};
use dioptra_stats::ttest::Alternative;

fn groups_of(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(nums).collect()
}

fn refs(g: &[Vec<f64>]) -> Vec<&[f64]> {
    g.iter().map(|v| v.as_slice()).collect()
}

fn matrix_of(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(nums).collect()
}

/// Check `e[key]` (number) against `got` if the key exists.
fn chk(ck: &mut Checker, c: &Value, e: &Value, key: &str, got: f64) {
    if e.get(key).is_none() {
        ck.fail(format!("{} :: missing expected key {key}", c["name"]));
        return;
    }
    let t = tol(c, key);
    ck.check_value(c["name"].as_str().unwrap(), key, &e[key], got, t);
}

fn chk_vec(ck: &mut Checker, c: &Value, e: &Value, key: &str, got: &[f64]) {
    let t = tol(c, key);
    ck.check_vec(c["name"].as_str().unwrap(), key, &nums(&e[key]), got, t);
}

pub fn anova_oneway_family() -> Checker {
    let mut ck = Checker::new("one-way ANOVA (+ Welch, Brown-Forsythe, Bartlett, Levene)");
    for c in cases("anova_oneway") {
        ck.cases += 1;
        let g = groups_of(&c["input"]["groups"]);
        let r = anova::one_way_anova(&refs(&g)).unwrap();
        let e = &c["expected"];
        for (k, v) in [
            ("ss_between", r.ss_between),
            ("ss_within", r.ss_within),
            ("ss_total", r.ss_total),
            ("df_between", r.df_between),
            ("df_within", r.df_within),
            ("df_total", r.df_total),
            ("ms_between", r.ms_between),
            ("ms_within", r.ms_within),
            ("F", r.f),
            ("p", r.p),
            ("r_squared_eta2", r.r_squared),
            ("omega2", r.omega_squared),
        ] {
            chk(&mut ck, &c, e, k, v);
        }
        let means: Vec<f64> = r.groups.iter().map(|s| s.mean).collect();
        let sds: Vec<f64> = r.groups.iter().map(|s| s.sd).collect();
        let ns: Vec<f64> = r.groups.iter().map(|s| s.n as f64).collect();
        chk_vec(&mut ck, &c, e, "group_means", &means);
        chk_vec(&mut ck, &c, e, "group_sds", &sds);
        chk_vec(&mut ck, &c, e, "group_n", &ns);
        let name = c["name"].as_str().unwrap().to_string();
        let w = r.welch.as_ref().unwrap();
        let ew = &e["welch"];
        let t = tol(&c, "welch");
        ck.check_value(&name, "welch.F", &ew["F"], w.f, t);
        ck.check_value(&name, "welch.df1", &ew["df1"], w.df1, t);
        ck.check_value(&name, "welch.df2", &ew["df2"], w.df2, t);
        ck.check_value(&name, "welch.p", &ew["p"], w.p, t);
        let b = r.brown_forsythe.as_ref().unwrap();
        let eb = &e["brown_forsythe_means_Fstar"];
        let t = tol(&c, "brown_forsythe_means_Fstar");
        ck.check_value(&name, "bf.F", &eb["F"], b.f, t);
        ck.check_value(&name, "bf.df1", &eb["df1"], b.df1, t);
        ck.check_value(&name, "bf.df2", &eb["df2"], b.df2, t);
        ck.check_value(&name, "bf.p", &eb["p"], b.p, t);
        ck.check_value(
            &name,
            "bf.mehrotra_df1",
            &eb["mehrotra_1997_df1"],
            b.mehrotra_df1,
            t,
        );
        ck.check_value(
            &name,
            "bf.mehrotra_p",
            &eb["mehrotra_1997_p"],
            b.mehrotra_p,
            t,
        );
        for (key, got) in [
            (
                "brown_forsythe_levene_median",
                r.brown_forsythe_variance.as_ref().unwrap(),
            ),
            ("levene_mean", r.levene_mean.as_ref().unwrap()),
        ] {
            let t = tol(&c, key);
            ck.check_value(&name, &format!("{key}.W"), &e[key]["W"], got.statistic, t);
            ck.check_value(&name, &format!("{key}.p"), &e[key]["p"], got.p, t);
        }
        let bt = r.bartlett.as_ref().unwrap();
        let t = tol(&c, "bartlett");
        ck.check_value(
            &name,
            "bartlett.statistic",
            &e["bartlett"]["statistic"],
            bt.statistic,
            t,
        );
        ck.check_value(&name, "bartlett.p", &e["bartlett"]["p"], bt.p, t);
    }
    ck
}

/// Long-format factor labels -> level indices in lexicographic order.
fn levels(v: &Value) -> (Vec<usize>, Vec<String>) {
    let labels: Vec<String> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    let mut uniq = labels.clone();
    uniq.sort();
    uniq.dedup();
    (
        labels
            .iter()
            .map(|l| uniq.iter().position(|u| u == l).unwrap())
            .collect(),
        uniq,
    )
}

pub fn anova_twoway_family() -> Checker {
    let mut ck = Checker::new("two-way ANOVA (types I, II, III)");
    for c in cases("anova_twoway") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let (a, la) = levels(&c["input"]["A"]);
        let (b, lb) = levels(&c["input"]["B"]);
        let y = nums(&c["input"]["y"]);
        let cells = anova::cells_from_long(&a, &b, &y).unwrap();
        let e = &c["expected"];
        for (key, ty) in [
            ("type1", SsType::I),
            ("type2", SsType::II),
            ("type3", SsType::III),
        ] {
            let r = anova::two_way_anova(
                &cells,
                TwoWayOptions {
                    ss_type: ty,
                    interaction: true,
                },
            )
            .unwrap();
            let t = tol(&c, key);
            let mut rows: Vec<&anova::AnovaTerm> = r.terms.iter().collect();
            rows.push(&r.residual);
            if let Some(i) = &r.intercept {
                rows.push(i);
            }
            let exp = e[key].as_object().unwrap();
            if exp.len() != rows.len() {
                ck.fail(format!(
                    "{name} :: {key}: {} rows vs {}",
                    exp.len(),
                    rows.len()
                ));
            }
            for row in rows {
                let er = &e[key][&row.name];
                let ctx = format!("{name} {key} {}", row.name);
                ck.check_value(&ctx, "ss", &er["ss"], row.ss, t);
                ck.check_value(&ctx, "df", &er["df"], row.df, t);
                ck.check_value(&ctx, "F", &er["F"], row.f, t);
                ck.check_value(&ctx, "p", &er["p"], row.p, t);
            }
            chk(&mut ck, &c, e, "residual_df", r.residual.df);
            chk(&mut ck, &c, e, "mse", r.residual.ms);
            for (i, ai) in la.iter().enumerate() {
                for (j, bj) in lb.iter().enumerate() {
                    let k = format!("{ai},{bj}");
                    ck.check_value(
                        &name,
                        &format!("cell_means[{k}]"),
                        &e["cell_means"][&k],
                        r.cell_means[i][j],
                        tol(&c, "cell_means"),
                    );
                }
            }
            // The treatment-coded "type III" is a trap: our sum-to-zero type III must differ
            // from it whenever the design is unbalanced.
            if ty == SsType::III && name.contains("unbalanced") {
                let wrong = num(&e["type3_treatment_coding_WRONG_for_reference"]["A"]).unwrap();
                if (r.terms[0].ss - wrong).abs() < 1e-6 * wrong {
                    ck.fail(format!(
                        "{name}: type III A matches the treatment-coded trap"
                    ));
                }
            }
        }
    }
    ck
}

pub fn anova_rm_family() -> Checker {
    let mut ck = Checker::new("repeated-measures ANOVA, sphericity, Friedman");
    for c in cases("anova_rm") {
        ck.cases += 1;
        let m = matrix_of(&c["input"]["Y_subjects_by_conditions"]);
        let r = anova::rm_anova(&m).unwrap();
        let fr = anova::friedman(&m).unwrap();
        let e = &c["expected"];
        for (k, v) in [
            ("F", r.f),
            ("df1", r.df_treatment),
            ("df2", r.df_error),
            ("p", r.p),
            ("ss_treatment", r.ss_treatment),
            ("ss_subjects", r.ss_subjects),
            ("ss_error", r.ss_error),
            ("ss_total", r.ss_total),
            ("ms_treatment", r.ms_treatment),
            ("ms_error", r.ms_error),
            ("gg_epsilon", r.gg_epsilon),
            ("gg_epsilon_box_formula", r.gg_epsilon_box_formula),
            ("hf_epsilon_uncapped", r.hf_epsilon_uncapped),
            ("hf_epsilon", r.hf_epsilon),
            ("lower_bound_epsilon", r.lower_bound_epsilon),
            ("mauchly_W", r.mauchly_w),
            ("mauchly_chi2", r.mauchly_chi2),
            ("mauchly_df", r.mauchly_df),
            ("mauchly_p", r.mauchly_p),
            ("p_gg", r.p_gg),
            ("df1_gg", r.df1_gg),
            ("df2_gg", r.df2_gg),
            ("p_hf", r.p_hf),
            ("df1_hf", r.df1_hf),
            ("df2_hf", r.df2_hf),
            ("friedman_chi2", fr.chi2),
            ("friedman_p", fr.p),
        ] {
            chk(&mut ck, &c, e, k, v);
        }
    }
    ck
}

fn check_dunn(ck: &mut Checker, c: &Value, d: &Value, res: &[(&str, posthoc::PostHoc)]) {
    let name = c["name"].as_str().unwrap();
    let t = tol(c, "dunn");
    let first = &res[0].1;
    ck.check_vec(
        name,
        "dunn.mean_ranks",
        &nums(&d["mean_ranks"]),
        &first.centers,
        t,
    );
    let pairs: Vec<(usize, usize)> = d["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p[0].as_u64().unwrap() as usize,
                p[1].as_u64().unwrap() as usize,
            )
        })
        .collect();
    let got_pairs: Vec<(usize, usize)> = first.comparisons.iter().map(|c| (c.i, c.j)).collect();
    if pairs != got_pairs {
        ck.fail(format!("{name}: pairs {pairs:?} vs {got_pairs:?}"));
    }
    let z: Vec<f64> = first.comparisons.iter().map(|c| c.statistic).collect();
    ck.check_vec(name, "dunn.z", &nums(&d["z"]), &z, t);
    let raw: Vec<f64> = first.comparisons.iter().map(|c| c.p_raw).collect();
    ck.check_vec(name, "dunn.p_raw", &nums(&d["p_raw"]), &raw, t);
    for (key, r) in res {
        let adj: Vec<f64> = r.comparisons.iter().map(|c| c.p_adj).collect();
        ck.check_vec(name, &format!("dunn.{key}"), &nums(&d[*key]), &adj, t);
    }
}

pub fn kruskal_friedman_dunn_family() -> Checker {
    let mut ck = Checker::new("Kruskal-Wallis, Friedman, Dunn");
    for c in cases("kruskal_friedman_dunn") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let e = &c["expected"];
        if name.starts_with("kruskal_wallis/") {
            let g = groups_of(&c["input"]["groups"]);
            let r = anova::kruskal_wallis(&refs(&g)).unwrap();
            chk(&mut ck, &c, e, "H", r.h);
            chk(&mut ck, &c, e, "H_uncorrected", r.h_uncorrected);
            chk(&mut ck, &c, e, "tie_correction_factor", r.tie_correction);
            chk(&mut ck, &c, e, "df", r.df);
            chk(&mut ck, &c, e, "p", r.p);
            let run = |a| posthoc::dunn_kruskal(&refs(&g), None, a, 0.05).unwrap();
            check_dunn(
                &mut ck,
                &c,
                &e["dunn"],
                &[
                    ("p_bonferroni", run(Adjust::Bonferroni)),
                    ("p_holm", run(Adjust::Holm)),
                    ("p_sidak", run(Adjust::Sidak)),
                ],
            );
            let mr = &r.mean_ranks;
            ck.check_vec(
                &name,
                "kw.mean_ranks",
                &nums(&e["dunn"]["mean_ranks"]),
                mr,
                tol(&c, "dunn"),
            );
        } else {
            let m = matrix_of(&c["input"]["Y_subjects_by_conditions"]);
            let r = anova::friedman(&m).unwrap();
            chk(&mut ck, &c, e, "chi2", r.chi2);
            chk(&mut ck, &c, e, "df", r.df);
            chk(&mut ck, &c, e, "p", r.p);
            ck.check_vec(
                &name,
                "dunn.rank_sums",
                &nums(&e["dunn"]["rank_sums"]),
                &r.rank_sums,
                tol(&c, "dunn"),
            );
            let run = |a| posthoc::dunn_friedman(&m, None, a, 0.05).unwrap();
            check_dunn(
                &mut ck,
                &c,
                &e["dunn"],
                &[
                    ("p_bonferroni", run(Adjust::Bonferroni)),
                    ("p_holm", run(Adjust::Holm)),
                ],
            );
        }
    }
    ck
}

pub fn multiple_comparisons_family() -> Checker {
    multiple_comparisons_impl(false)
}

/// The loose library cross-checks stored next to the exact values: statsmodels' `qsturng`
/// Tukey p and SciPy's randomized-QMC Dunnett p and CI (abs 2e-3 / rel 1e-3).
pub fn multiple_comparisons_sanity() -> Checker {
    multiple_comparisons_impl(true)
}

fn multiple_comparisons_impl(sanity: bool) -> Checker {
    let mut ck = Checker::new(if sanity {
        "sanity: statsmodels qsturng, SciPy QMC Dunnett"
    } else {
        "Tukey-Kramer, Dunnett, p-value adjustment"
    });
    for c in cases("multiple_comparisons") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let e = &c["expected"];
        if name.starts_with("tukey_kramer/") {
            let g = groups_of(&c["input"]["groups"]);
            let r = posthoc::tukey_kramer(&refs(&g), 0.05).unwrap();
            chk(&mut ck, &c, e, "q_crit_0.95", r.critical_value.unwrap());
            chk(&mut ck, &c, e, "ms_within", r.ms_within);
            chk(&mut ck, &c, e, "df_within", r.df_within);
            // fixture: mean_diff = mean_j - mean_i; ours: mean_i - mean_j
            let md: Vec<f64> = r.comparisons.iter().map(|x| -x.estimate).collect();
            chk_vec(&mut ck, &c, e, "mean_diff", &md);
            let q: Vec<f64> = r.comparisons.iter().map(|x| x.statistic).collect();
            chk_vec(&mut ck, &c, e, "q", &q);
            let p: Vec<f64> = r.comparisons.iter().map(|x| x.p_adj).collect();
            if sanity {
                chk_vec(&mut ck, &c, e, "statsmodels_p_adj", &p);
                continue;
            }
            chk_vec(&mut ck, &c, e, "p_adj", &p);
            let t = tol(&c, "ci95");
            for (k, (cmp, ev)) in r
                .comparisons
                .iter()
                .zip(e["ci95"].as_array().unwrap())
                .enumerate()
            {
                let (lo, hi) = cmp.ci.unwrap();
                ck.check(
                    &name,
                    &format!("ci95[{k}].lo"),
                    num(&ev[0]).unwrap(),
                    -hi,
                    t,
                );
                ck.check(
                    &name,
                    &format!("ci95[{k}].hi"),
                    num(&ev[1]).unwrap(),
                    -lo,
                    t,
                );
            }
        } else if name.starts_with("dunnett/") {
            let mut g = vec![nums(&c["input"]["control"])];
            g.extend(groups_of(&c["input"]["treatments"]));
            let r2 = posthoc::dunnett(&refs(&g), 0, Alternative::TwoSided, 0.05).unwrap();
            let rg = posthoc::dunnett(&refs(&g), 0, Alternative::Greater, 0.05).unwrap();
            let p2: Vec<f64> = r2.comparisons.iter().map(|x| x.p_adj).collect();
            if sanity {
                chk_vec(&mut ck, &c, e, "scipy_dunnett_p", &p2);
                // SciPy's CI is stored as [lows, highs].
                let sc = &e["scipy_dunnett_ci95"];
                let lows: Vec<f64> = r2.comparisons.iter().map(|x| x.ci.unwrap().0).collect();
                let highs: Vec<f64> = r2.comparisons.iter().map(|x| x.ci.unwrap().1).collect();
                let t = tol(&c, "scipy_dunnett_ci95");
                ck.check_vec(&name, "scipy_dunnett_ci95.lows", &nums(&sc[0]), &lows, t);
                ck.check_vec(&name, "scipy_dunnett_ci95.highs", &nums(&sc[1]), &highs, t);
                continue;
            }
            let tv: Vec<f64> = r2.comparisons.iter().map(|x| x.statistic).collect();
            chk_vec(&mut ck, &c, e, "t", &tv);
            chk(&mut ck, &c, e, "df", r2.df_within);
            let lam: Vec<f64> = (1..g.len())
                .map(|i| {
                    let (ni, n0) = (g[i].len() as f64, g[0].len() as f64);
                    (ni / (ni + n0)).sqrt()
                })
                .collect();
            chk_vec(&mut ck, &c, e, "lambda", &lam);
            chk_vec(&mut ck, &c, e, "p_two_sided", &p2);
            let pg: Vec<f64> = rg.comparisons.iter().map(|x| x.p_adj).collect();
            chk_vec(&mut ck, &c, e, "p_greater", &pg);
            chk(
                &mut ck,
                &c,
                e,
                "critical_value_two_sided_0.95",
                r2.critical_value.unwrap(),
            );
            let md: Vec<f64> = r2.comparisons.iter().map(|x| x.estimate).collect();
            chk_vec(&mut ck, &c, e, "mean_diff", &md);
            let t = tol(&c, "ci95");
            for (k, (cmp, ev)) in r2
                .comparisons
                .iter()
                .zip(e["ci95"].as_array().unwrap())
                .enumerate()
            {
                let (lo, hi) = cmp.ci.unwrap();
                ck.check(&name, &format!("ci95[{k}].lo"), num(&ev[0]).unwrap(), lo, t);
                ck.check(&name, &format!("ci95[{k}].hi"), num(&ev[1]).unwrap(), hi, t);
            }
        } else {
            if sanity {
                continue;
            }
            let p = nums(&c["input"]["p"]);
            for (key, m) in [
                ("bonferroni", Adjust::Bonferroni),
                ("sidak", Adjust::Sidak),
                ("holm", Adjust::Holm),
                ("holm_sidak", Adjust::HolmSidak),
                ("fdr_bh", Adjust::BenjaminiHochberg),
                ("fdr_by", Adjust::BenjaminiYekutieli),
                ("hommel", Adjust::Hommel),
            ] {
                chk_vec(&mut ck, &c, e, key, &posthoc::p_adjust(&p, m));
            }
        }
    }
    ck
}

// ------------------------------------------------------------------ contingency

use dioptra_stats::contingency as ct;

fn table_of(v: &Value) -> Vec<Vec<u64>> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap())
                .collect()
        })
        .collect()
}

fn t2(t: &[Vec<u64>]) -> [[u64; 2]; 2] {
    [[t[0][0], t[0][1]], [t[1][0], t[1][1]]]
}

fn chk_pair(ck: &mut Checker, c: &Value, key: &str, got: Option<(f64, f64)>) {
    let e = &c["expected"];
    if e.get(key).is_none() {
        return;
    }
    let name = c["name"].as_str().unwrap();
    match got {
        Some((lo, hi)) => {
            ck.check_vec(name, key, &nums(&e[key]), &[lo, hi], tol(c, key));
        }
        None => ck.fail(format!("{name} :: {key}: not computed")),
    }
}

pub fn contingency_family() -> Checker {
    let mut ck = Checker::new("contingency (chi-square, Fisher, McNemar, trend)");
    for c in cases("contingency") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let e = &c["expected"];
        let inp = &c["input"];
        let kind = name.split('/').next().unwrap();
        match kind {
            "chi2_independence" => {
                let t = table_of(&inp["table"]);
                let r = ct::chi_square_independence(&t).unwrap();
                let tl = tol(&c, "pearson");
                ck.check_value(
                    &name,
                    "pearson.chi2",
                    &e["pearson"]["chi2"],
                    r.pearson.statistic,
                    tl,
                );
                ck.check_value(&name, "pearson.df", &e["pearson"]["df"], r.pearson.df, tl);
                ck.check_value(&name, "pearson.p", &e["pearson"]["p"], r.pearson.p, tl);
                let y = r.yates.clone().unwrap_or(r.pearson.clone());
                let tl = tol(&c, "yates");
                ck.check_value(&name, "yates.chi2", &e["yates"]["chi2"], y.statistic, tl);
                ck.check_value(&name, "yates.df", &e["yates"]["df"], y.df, tl);
                ck.check_value(&name, "yates.p", &e["yates"]["p"], y.p, tl);
                let tl = tol(&c, "g_test");
                ck.check_value(&name, "g_test.G", &e["g_test"]["G"], r.g_test.statistic, tl);
                ck.check_value(&name, "g_test.p", &e["g_test"]["p"], r.g_test.p, tl);
                let ex: Vec<f64> = r.expected.iter().flatten().copied().collect();
                let ee: Vec<f64> = e["expected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(nums)
                    .collect();
                ck.check_vec(&name, "expected", &ee, &ex, tol(&c, "expected"));
            }
            "chi2_gof" => {
                let o: Vec<u64> = inp["observed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                let pr = if inp["expected_proportions"].is_null() {
                    None
                } else {
                    Some(nums(&inp["expected_proportions"]))
                };
                let r = ct::chi_square_gof(&o, pr.as_deref(), 0).unwrap();
                chk(&mut ck, &c, e, "chi2", r.chi2);
                chk(&mut ck, &c, e, "df", r.df);
                chk(&mut ck, &c, e, "p", r.p);
            }
            "fisher_2x2" => {
                let t = t2(&table_of(&inp["table"]));
                let r = ct::fisher_exact_2x2(t, 0.95).unwrap();
                let m = ct::risk_measures(t, 0.95).unwrap();
                chk(&mut ck, &c, e, "p_two_sided", r.p_two_sided);
                chk(&mut ck, &c, e, "p_less", r.p_less);
                chk(&mut ck, &c, e, "p_greater", r.p_greater);
                chk(
                    &mut ck,
                    &c,
                    e,
                    "odds_ratio_conditional_mle",
                    r.odds_ratio_conditional_mle,
                );
                chk_pair(
                    &mut ck,
                    &c,
                    "odds_ratio_conditional_ci95",
                    Some(r.odds_ratio_conditional_ci),
                );
                chk(&mut ck, &c, e, "odds_ratio_sample", r.odds_ratio_sample);
                chk(&mut ck, &c, e, "risk_difference", m.risk_difference);
                chk_pair(&mut ck, &c, "odds_ratio_woolf_ci95", m.odds_ratio_woolf_ci);
                chk_pair(&mut ck, &c, "scipy_sample_or_ci95", m.odds_ratio_woolf_ci);
                if e.get("relative_risk").is_some() {
                    chk(&mut ck, &c, e, "relative_risk", m.relative_risk);
                }
                chk_pair(
                    &mut ck,
                    &c,
                    "relative_risk_katz_ci95",
                    m.relative_risk_katz_ci,
                );
            }
            "fisher_rxc" => {
                let t = table_of(&inp["table"]);
                let r = ct::fisher_exact_rxc(&t).unwrap();
                chk(&mut ck, &c, e, "p_two_sided", r.p_two_sided);
                chk(&mut ck, &c, e, "p_observed_table", r.p_observed_table);
                chk(&mut ck, &c, e, "n_tables", r.n_tables.unwrap_or(f64::NAN));
                chk(&mut ck, &c, e, "sum_prob_check", r.total_probability);
            }
            "mcnemar" => {
                let t = t2(&table_of(&inp["table"]));
                let r = ct::mcnemar(t).unwrap();
                let tl = tol(&c, "nocc");
                ck.check_value(&name, "nocc.chi2", &e["nocc"]["chi2"], r.chi2, tl);
                ck.check_value(&name, "nocc.p", &e["nocc"]["p"], r.p, tl);
                ck.check_value(&name, "cc.chi2", &e["cc"]["chi2"], r.chi2_corrected, tl);
                ck.check_value(&name, "cc.p", &e["cc"]["p"], r.p_corrected, tl);
                let ex = &e["exact_binomial"];
                ck.check_value(
                    &name,
                    "exact.min",
                    &ex["statistic_min_discordant"],
                    r.b.min(r.c) as f64,
                    tl,
                );
                ck.check_value(&name, "exact.p", &ex["p"], r.p_exact, tl);
            }
            "cochran_armitage" => {
                let s: Vec<u64> = inp["successes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                let n: Vec<u64> = inp["totals"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                let sc = nums(&inp["scores"]);
                let r = ct::cochran_armitage(&s, &n, Some(&sc)).unwrap();
                chk(&mut ck, &c, e, "z", r.z);
                chk(&mut ck, &c, e, "chi2", r.chi2);
                chk(&mut ck, &c, e, "p_two_sided", r.p_two_sided);
                chk(
                    &mut ck,
                    &c,
                    e,
                    "z_with_N_over_N_minus_1",
                    r.z_n_over_n_minus_1,
                );
            }
            other => ck.fail(format!("{name}: unknown kind {other}")),
        }
    }
    ck
}

// ------------------------------------------------------------------ correlation

use dioptra_stats::correlation as cor;

pub fn correlation_family() -> Checker {
    let mut ck = Checker::new("correlation (Pearson, Spearman, Kendall)");
    for c in cases("correlation") {
        ck.cases += 1;
        let e = &c["expected"];
        let (x, y) = (nums(&c["input"]["x"]), nums(&c["input"]["y"]));
        let p = cor::pearson(&x, &y, 0.95).unwrap();
        chk(&mut ck, &c, e, "pearson_r", p.estimate);
        chk(&mut ck, &c, e, "pearson_p_two_sided", p.p_two_sided);
        chk(&mut ck, &c, e, "pearson_p_greater", p.p_greater);
        chk(&mut ck, &c, e, "r_squared", p.r_squared);
        chk_pair(&mut ck, &c, "pearson_ci95_fisher_z", p.ci);
        let s = cor::spearman(&x, &y, 0.95).unwrap();
        chk(&mut ck, &c, e, "spearman_rho", s.rho);
        chk(
            &mut ck,
            &c,
            e,
            "spearman_p_t_approx_two_sided",
            s.p_t_two_sided,
        );
        for (key, got) in [
            ("spearman_p_exact_two_sided", s.p_exact_two_sided),
            ("spearman_p_exact_greater", s.p_exact_greater),
            ("spearman_p_exact_less", s.p_exact_less),
            ("spearman_n_permutations", s.n_permutations),
        ] {
            if e.get(key).is_some() {
                chk(&mut ck, &c, e, key, got.unwrap_or(f64::NAN));
            }
        }
        let k = cor::kendall(&x, &y).unwrap();
        chk(&mut ck, &c, e, "kendall_tau_b", k.tau_b);
        chk(&mut ck, &c, e, "kendall_tau_c", k.tau_c);
        chk(&mut ck, &c, e, "kendall_p_asymptotic", k.p_asymptotic);
        chk(&mut ck, &c, e, "kendall_p_two_sided_scipy_auto", k.p_auto);
        if e.get("kendall_p_exact").is_some() {
            chk(
                &mut ck,
                &c,
                e,
                "kendall_p_exact",
                k.p_exact.unwrap_or(f64::NAN),
            );
        } else if k.p_exact.is_some() {
            ck.fail(format!(
                "{}: exact Kendall p computed despite ties",
                c["name"]
            ));
        }
    }
    ck
}

// ------------------------------------------------------------------ linear regression

use dioptra_stats::regression as reg;

pub fn linear_regression_family() -> Checker {
    let mut ck = Checker::new("linear regression (simple, bands, runs, Deming, multiple)");
    for c in cases("linear_regression") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap().to_string();
        let e = &c["expected"];
        if name.starts_with("simple/") {
            let (x, y) = (nums(&c["input"]["x"]), nums(&c["input"]["y"]));
            let r = reg::simple_linear_regression(&x, &y, reg::SimpleRegressionOptions::default())
                .unwrap();
            for (k, v) in [
                ("slope", r.slope),
                ("intercept", r.intercept),
                ("se_slope", r.se_slope),
                ("se_intercept", r.se_intercept),
                ("t_slope", r.t_slope),
                ("p_slope", r.p_slope),
                ("t_intercept", r.t_intercept),
                ("p_intercept", r.p_intercept),
                ("r", r.r),
                ("r_squared", r.r_squared),
                ("adj_r_squared", r.adj_r_squared),
                ("sy_x", r.sy_x),
                ("ss_residual", r.ss_residual),
                ("df_residual", r.df_residual),
                ("F", r.f),
                ("p_F", r.p_f),
                ("x_intercept", r.x_intercept),
            ] {
                chk(&mut ck, &c, e, k, v);
            }
            chk_pair(&mut ck, &c, "ci95_slope", Some(r.ci_slope));
            chk_pair(&mut ck, &c, "ci95_intercept", Some(r.ci_intercept));
            let eb = &e["bands"];
            let b = r.bands(&nums(&eb["x"]));
            let t = tol(&c, "bands");
            for (k, v) in [
                ("fit", &b.fit),
                ("ci95_mean_lower", &b.ci_lower),
                ("ci95_mean_upper", &b.ci_upper),
                ("pi95_lower", &b.pi_lower),
                ("pi95_upper", &b.pi_upper),
            ] {
                ck.check_vec(&name, &format!("bands.{k}"), &nums(&eb[k]), v, t);
            }
            let er = &e["runs_test"];
            let rt = &r.runs_test;
            let t = tol(&c, "runs_test");
            // exact discrete p: tightened to 1e-12 relative (research/05 §3)
            let texact = Tol::new(1e-12, 1e-14);
            ck.check_value(&name, "runs.runs", &er["runs"], rt.runs as f64, t);
            ck.check_value(
                &name,
                "runs.n_positive",
                &er["n_positive"],
                rt.n_positive as f64,
                t,
            );
            ck.check_value(
                &name,
                "runs.n_negative",
                &er["n_negative"],
                rt.n_negative as f64,
                t,
            );
            ck.check_value(
                &name,
                "runs.p_exact",
                &er["p_exact_too_few_runs"],
                rt.p_exact_too_few_runs,
                texact,
            );
            ck.check_value(
                &name,
                "runs.expected_runs",
                &er["expected_runs"],
                rt.expected_runs,
                t,
            );
            ck.check_value(&name, "runs.z", &er["z"], rt.z, t);
            let t = tol(&c, "deming");
            for (key, lam) in [("lambda_1", 1.0), ("lambda_2", 2.0)] {
                let d = reg::deming_regression(&x, &y, lam, 0.95).unwrap();
                let ed = &e["deming"][key];
                ck.check_value(
                    &name,
                    &format!("deming.{key}.slope"),
                    &ed["slope"],
                    d.slope,
                    t,
                );
                ck.check_value(
                    &name,
                    &format!("deming.{key}.intercept"),
                    &ed["intercept"],
                    d.intercept,
                    t,
                );
                ck.check_value(
                    &name,
                    &format!("deming.{key}.se_slope"),
                    &ed["se_slope_jackknife"],
                    d.se_slope,
                    t,
                );
                ck.check_value(
                    &name,
                    &format!("deming.{key}.se_intercept"),
                    &ed["se_intercept_jackknife"],
                    d.se_intercept,
                    t,
                );
            }
        } else {
            let rows = matrix_of(&c["input"]["X"]);
            let y = nums(&c["input"]["y"]);
            let r = reg::multiple_regression(&rows, &y, reg::MultipleRegressionOptions::default())
                .unwrap();
            chk_vec(&mut ck, &c, e, "params", &r.coefficients);
            chk_vec(&mut ck, &c, e, "se", &r.se);
            chk_vec(&mut ck, &c, e, "t", &r.t);
            chk_vec(&mut ck, &c, e, "p", &r.p);
            let t = tol(&c, "ci95");
            for (k, (ci, ev)) in r.ci.iter().zip(e["ci95"].as_array().unwrap()).enumerate() {
                ck.check_vec(&name, &format!("ci95[{k}]"), &nums(ev), &[ci.0, ci.1], t);
            }
            for (k, v) in [
                ("r_squared", r.r_squared),
                ("adj_r_squared", r.adj_r_squared),
                ("F", r.f),
                ("p_F", r.p_f),
                ("sy_x", r.sy_x),
                ("ss_residual", r.ss_residual),
                ("df_residual", r.df_residual),
            ] {
                chk(&mut ck, &c, e, k, v);
            }
            chk_vec(&mut ck, &c, e, "vif", &r.vif[1..]);
        }
    }
    ck
}

// ------------------------------------------------------------------ NIST linear and ANOVA

/// One NIST linear-regression dataset scored (LRE, capped at 15).
pub struct NistLinRow {
    pub name: String,
    pub difficulty: String,
    /// min LRE over the parameters
    pub params: f64,
    /// min LRE over the parameter SDs
    pub sds: f64,
    pub r_squared: f64,
    pub residual_sd: f64,
    /// min LRE over the ANOVA table entries (SS, MS, F)
    pub anova: f64,
    /// min of all the above
    pub min: f64,
    /// the same minimum against the exact solution for the inputs as parsed into doubles
    pub min_exact_on_doubles: f64,
}

pub const NIST_LINEAR: [&str; 11] = [
    "Norris", "Pontius", "NoInt1", "NoInt2", "Filip", "Longley", "Wampler1", "Wampler2",
    "Wampler3", "Wampler4", "Wampler5",
];

pub const NIST_ANOVA: [&str; 11] = [
    "SiRstv", "SmLs01", "SmLs02", "SmLs03", "AtmWtAg", "SmLs04", "SmLs05", "SmLs06", "SmLs07",
    "SmLs08", "SmLs09",
];

/// Fit a NIST linear dataset with the model it names.
pub fn nist_linear_fit(j: &Value) -> reg::MultipleRegression {
    let data = &j["data"];
    let y = nums(&data["y"]);
    let model = &j["model"];
    let intercept = model["intercept"].as_bool().unwrap();
    let opts = reg::MultipleRegressionOptions {
        intercept,
        ci_level: 0.95,
    };
    if model["kind"] == "polynomial" {
        let x = nums(&data["x"]);
        let deg = model["degree"].as_u64().unwrap() as usize;
        reg::polynomial_regression(&x, &y, deg, opts).unwrap()
    } else {
        let p = model["predictors"].as_u64().unwrap() as usize;
        let cols: Vec<Vec<f64>> = (1..=p).map(|k| nums(&data[&format!("x{k}")])).collect();
        let rows: Vec<Vec<f64>> = (0..y.len())
            .map(|i| cols.iter().map(|c| c[i]).collect())
            .collect();
        reg::multiple_regression(&rows, &y, opts).unwrap()
    }
}

fn cert(v: &Value) -> Option<f64> {
    if v.is_object() {
        num(&v["value"])
    } else {
        num(v)
    }
}

pub fn nist_linear() -> Vec<NistLinRow> {
    NIST_LINEAR
        .iter()
        .map(|&name| {
            let j = load(&format!("nist/{name}.json"));
            let r = nist_linear_fit(&j);
            let ps = j["parameters"].as_array().unwrap();
            let params = ps
                .iter()
                .enumerate()
                .map(|(k, p)| lre(r.coefficients[k], num(&p["certified"]).unwrap()))
                .fold(f64::INFINITY, f64::min);
            let sds = ps
                .iter()
                .enumerate()
                .map(|(k, p)| lre(r.se[k], num(&p["std_dev"]).unwrap()))
                .fold(f64::INFINITY, f64::min);
            let r2 = lre(r.r_squared, cert(&j["r_squared"]).unwrap());
            let rsd = lre(r.sy_x, cert(&j["residual_std_dev"]).unwrap());
            let a = &j["anova"];
            let mut an = f64::INFINITY;
            for (got, v) in [
                (r.ss_regression, &a["regression"]["ss"]),
                (r.ms_regression, &a["regression"]["ms"]),
                (r.f, &a["regression"]["f"]),
                (r.ss_residual, &a["residual"]["ss"]),
                (r.ms_residual, &a["residual"]["ms"]),
            ] {
                if let Some(cv) = cert(v) {
                    an = an.min(lre(got, cv));
                }
            }
            let min = params.min(sds).min(r2).min(rsd).min(an);
            let exact = NIST_LINEAR_EXACT_ON_DOUBLES
                .iter()
                .find(|e| e.0 == name)
                .map(|e| {
                    // Exact values below 1e-12 (the SDs and residual SD of the
                    // near-perfect Wampler1/2 fits) are scored by absolute error.
                    let l = |got: f64, ex: f64| {
                        if ex.abs() < 1e-12 {
                            lre(got - ex, 0.0)
                        } else {
                            lre(got, ex)
                        }
                    };
                    let mut m = f64::INFINITY;
                    for (k, &b) in e.1.iter().enumerate() {
                        m = m.min(l(r.coefficients[k], b));
                    }
                    for (k, &s) in e.2.iter().enumerate() {
                        m = m.min(l(r.se[k], s));
                    }
                    m.min(l(r.sy_x, e.3)).min(l(r.r_squared, e.4))
                })
                .unwrap_or(f64::NAN);
            NistLinRow {
                name: name.to_string(),
                difficulty: j["difficulty"].as_str().unwrap().to_string(),
                params,
                sds,
                r_squared: r2,
                residual_sd: rsd,
                anova: an,
                min,
                min_exact_on_doubles: exact,
            }
        })
        .collect()
}

/// One NIST one-way ANOVA dataset scored.
pub struct NistAnovaRow {
    pub name: String,
    pub difficulty: String,
    pub ss_between: f64,
    pub ss_within: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub f: f64,
    pub r_squared: f64,
    pub residual_sd: f64,
    pub min: f64,
    pub min_exact_on_doubles: f64,
}

pub fn nist_anova_fit(j: &Value) -> anova::OneWayAnova {
    let g = nums(&j["data"]["group"]);
    let y = nums(&j["data"]["y"]);
    let mut labels: Vec<i64> = g.iter().map(|v| *v as i64).collect();
    labels.sort_unstable();
    labels.dedup();
    let groups: Vec<Vec<f64>> = labels
        .iter()
        .map(|&l| {
            g.iter()
                .zip(&y)
                .filter(|(a, _)| **a as i64 == l)
                .map(|(_, b)| *b)
                .collect()
        })
        .collect();
    anova::one_way_anova(&refs(&groups)).unwrap()
}

pub fn nist_anova() -> Vec<NistAnovaRow> {
    NIST_ANOVA
        .iter()
        .map(|&name| {
            let j = load(&format!("nist/{name}.json"));
            let r = nist_anova_fit(&j);
            let b = &j["between"];
            let w = &j["within"];
            let row = [
                lre(r.ss_between, cert(&b["ss"]).unwrap()),
                lre(r.ss_within, cert(&w["ss"]).unwrap()),
                lre(r.ms_between, cert(&b["ms"]).unwrap()),
                lre(r.ms_within, cert(&w["ms"]).unwrap()),
                lre(r.f, cert(&b["f"]).unwrap()),
                lre(r.r_squared, cert(&j["r_squared"]).unwrap()),
                lre(r.residual_sd, cert(&j["residual_std_dev"]).unwrap()),
            ];
            let exact = NIST_ANOVA_EXACT_ON_DOUBLES
                .iter()
                .find(|e| e.0 == name)
                .map(|e| {
                    [
                        lre(r.ss_between, e.1[0]),
                        lre(r.ss_within, e.1[1]),
                        lre(r.f, e.1[2]),
                        lre(r.r_squared, e.1[3]),
                        lre(r.residual_sd, e.1[4]),
                    ]
                    .into_iter()
                    .fold(f64::INFINITY, f64::min)
                })
                .unwrap_or(f64::NAN);
            NistAnovaRow {
                name: name.to_string(),
                difficulty: j["difficulty"].as_str().unwrap().to_string(),
                ss_between: row[0],
                ss_within: row[1],
                ms_between: row[2],
                ms_within: row[3],
                f: row[4],
                r_squared: row[5],
                residual_sd: row[6],
                min: row.into_iter().fold(f64::INFINITY, f64::min),
                min_exact_on_doubles: exact,
            }
        })
        .collect()
}

/// (name, coefficients, SDs, residual SD, R^2): the exact least-squares solution for the
/// NIST inputs as parsed into doubles (polynomial powers formed exactly), computed in
/// rational arithmetic (Python `fractions`: normal equations solved exactly, square roots
/// in 60-digit decimals) and rounded to double. Separates the algorithm from input
/// rounding, as the univariate table above does.
/// (name, coefficients, SDs, residual SD, R^2).
pub type ExactLinear = (&'static str, &'static [f64], &'static [f64], f64, f64);

pub const NIST_LINEAR_EXACT_ON_DOUBLES: &[ExactLinear] = &[
    (
        "Norris",
        &[-0.26232307377402675, 1.0021168180204545],
        &[0.2328182343011548, 0.0004297968481999412],
        0.8847963961443813,
        0.9999937458837117,
    ),
    (
        "Pontius",
        &[
            0.0006735657894736632,
            7.320591604010026e-07,
            -3.1608187134503054e-15,
        ],
        &[
            0.00010793861203307534,
            1.5781739998165632e-10,
            4.866528499920286e-17,
        ],
        0.00020517742407618158,
        0.9999999001785371,
    ),
    (
        "NoInt1",
        &[2.074380165289256],
        &[0.01652892561983471],
        3.567530340063379,
        0.9993654922986628,
    ),
    (
        "NoInt2",
        &[0.7272727272727273],
        &[0.04208273180784325],
        0.3692744729379982,
        0.9933481152993349,
    ),
    (
        "Filip",
        &[
            -1467.4896142297885,
            -2772.17959193341,
            -2316.3710816089188,
            -1127.97394098371,
            -354.4782337033469,
            -75.12420173937532,
            -10.875318035534194,
            -1.062214985889462,
            -0.06701911545934047,
            -0.002467810782754773,
            -4.029625250804014e-05,
        ],
        &[
            298.08453099553685,
            559.7798654749496,
            466.47757212779624,
            227.20427447775123,
            71.6478660875927,
            15.28971787474,
            2.236911598160332,
            0.22162432193422732,
            0.014236376315472392,
            0.0005356174088898208,
            8.966328373738679e-06,
        ],
        0.0033480105132454342,
        0.9967274161856201,
    ),
    (
        "Longley",
        &[
            -3482258.6345958184,
            15.061872271373323,
            -0.03581917929259102,
            -2.020229803816825,
            -1.033226867173592,
            -0.05110410565358071,
            1829.151464613552,
        ],
        &[
            890420.3836073726,
            84.91492577476696,
            0.03349100777224318,
            0.4883996816516994,
            0.21427416316167527,
            0.2260732000693702,
            455.478499142212,
        ],
        304.8540735619648,
        0.9954790045772957,
    ),
    (
        "Wampler1",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        0.0,
        1.0,
    ),
    (
        "Wampler2",
        &[
            0.9999999999999998,
            0.10000000000000081,
            0.009999999999999617,
            0.001000000000000063,
            9.999999999999588e-05,
            1.000000000000009e-05,
        ],
        &[
            6.385093232145039e-16,
            7.011714979363868e-16,
            2.3120012917030466e-16,
            3.010373438771421e-17,
            1.674843589045736e-18,
            3.332230281013217e-20,
        ],
        7.001608627331804e-16,
        1.0,
    ),
    (
        "Wampler3",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        &[
            2152.3262467816967,
            2363.5517346968054,
            779.3435243315831,
            101.47550755034965,
            5.645665121707515,
            0.11232485467931194,
        ],
        2360.1450237926765,
        0.9999955590258199,
    ),
    (
        "Wampler4",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        &[
            215232.62467816967,
            236355.1734696805,
            77934.3524331583,
            10147.550755034965,
            564.5665121707516,
            11.232485467931193,
        ],
        236014.50237926765,
        0.9574784408256621,
    ),
    (
        "Wampler5",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        &[
            21523262.467816968,
            23635517.34696805,
            7793435.243315831,
            1014755.0755034965,
            56456.65121707516,
            1123.2485467931194,
        ],
        23601450.237926766,
        0.002246689215749398,
    ),
];

/// (name, [SS between, SS within, F, R^2, residual SD]) exact for the double inputs.
pub const NIST_ANOVA_EXACT_ON_DOUBLES: &[(&str, [f64; 5])] = &[
    (
        "SiRstv",
        [
            0.05114626159999952,
            0.2166365600000165,
            1.1804623744024467,
            0.19099903905111612,
            0.10407606833466003,
        ],
    ),
    (
        "SmLs01",
        [
            1.680000000000001,
            1.800000000000001,
            21.0,
            0.4827586206896552,
            0.10000000000000003,
        ],
    ),
    (
        "SmLs02",
        [
            16.08000000000001,
            18.00000000000001,
            201.00000000000003,
            0.471830985915493,
            0.10000000000000003,
        ],
    ),
    (
        "SmLs03",
        [
            160.0800000000001,
            180.00000000000009,
            2001.0000000000002,
            0.47071277346506707,
            0.10000000000000003,
        ],
    ),
    (
        "AtmWtAg",
        [
            3.6383418747907132e-09,
            1.049517291679747e-08,
            15.946733566676926,
            0.2574265445249428,
            1.5104831444735094e-05,
        ],
    ),
    (
        "SmLs04",
        [
            1.6800000001490116,
            1.8000000000931322,
            21.0000000007761,
            0.4827586206988835,
            0.100000000002587,
        ],
    ),
    (
        "SmLs05",
        [
            16.080000001825393,
            18.000000000931323,
            201.00000001241764,
            0.47183098593088874,
            0.100000000002587,
        ],
    ),
    (
        "SmLs06",
        [
            160.0800000185892,
            180.00000000931323,
            2001.0000001288329,
            0.4707127734811079,
            0.100000000002587,
        ],
    ),
    (
        "SmLs07",
        [
            1.6801562694014696,
            1.8000978373345875,
            21.00081188781877,
            0.4827682743479894,
            0.10000271766680997,
        ],
    ),
    (
        "SmLs08",
        [
            16.081914284204238,
            18.00097824625708,
            201.01300409594845,
            0.4718471083353959,
            0.10000271731379513,
        ],
    ),
    (
        "SmLs09",
        [
            160.09949443572512,
            180.00978232919425,
            2001.1349262209505,
            0.47072957244381347,
            0.1000027172767471,
        ],
    ),
];

// ------------------------------------------------------------------ nonlinear regression

use dioptra_stats::equations::{self, Equation};
use dioptra_stats::nonlinear::{self as nl, Dataset, FitOptions, ParamConstraint};

fn names_of(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn custom_eq(expr: &str, names: &[String]) -> Equation {
    let n: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    Equation::custom_with(expr, &["x"], Some(&n)).unwrap()
}

/// Compare a fit's free parameters (`vals`, `ses`, `cov` over them) with an expected fit
/// block (params, se, ci95, ss, df, sy_x, aicc, r_squared, cov_scaled), whichever exist.
fn check_fit_block(ck: &mut Checker, c: &Value, ctx: &str, e: &Value, fit: &nl::CurveFit) {
    let t = tol(c, "params");
    let d0 = &fit.datasets[0];
    let free: Vec<&nl::ParamEstimate> = d0
        .params
        .iter()
        .filter(|p| p.status != nl::ParamStatus::Fixed)
        .collect();
    if let Some(ps) = e.get("params") {
        let exp = nums(ps);
        let got: Vec<f64> = free.iter().map(|p| p.value).collect();
        ck.check_vec(ctx, "params", &exp, &got, t);
    }
    if let Some(ps) = e.get("se") {
        let got: Vec<f64> = free.iter().map(|p| p.se).collect();
        ck.check_vec(ctx, "se", &nums(ps), &got, tol(c, "se"));
    }
    if let Some(ci) = e.get("ci95") {
        for (k, row) in ci.as_array().unwrap().iter().enumerate() {
            let r = nums(row);
            ck.check(
                ctx,
                &format!("ci95[{k}].lo"),
                r[0],
                free[k].ci_low,
                tol(c, "ci95"),
            );
            ck.check(
                ctx,
                &format!("ci95[{k}].hi"),
                r[1],
                free[k].ci_high,
                tol(c, "ci95"),
            );
        }
    }
    for (key, got) in [
        ("ss", fit.ss),
        ("df", fit.df),
        ("sy_x", fit.sy_x),
        ("aicc", fit.aicc),
        ("r_squared", fit.r_squared),
    ] {
        if let Some(v) = e.get(key) {
            ck.check_value(ctx, key, v, got, tol(c, key));
        }
    }
    if let Some(cv) = e.get("cov_scaled") {
        let k = fit.k;
        for (a, row) in cv.as_array().unwrap().iter().enumerate() {
            for (b, v) in nums(row).iter().enumerate() {
                ck.check(
                    ctx,
                    &format!("cov_scaled[{a}][{b}]"),
                    *v,
                    fit.covariance[a * k + b],
                    tol(c, "cov_scaled"),
                );
            }
        }
    }
}

fn opts_with_initial(p0: Vec<f64>) -> FitOptions {
    FitOptions {
        initial: Some(vec![p0]),
        ..FitOptions::default()
    }
}

pub fn nonlinear_family() -> Checker {
    let mut ck = Checker::new("nonlinear regression (fits, comparison, weighting, robust)");
    let cases = cases("nonlinear_regression");
    let fx =
        |c: &Value| -> (Vec<f64>, Vec<f64>) { (nums(&c["input"]["x"]), nums(&c["input"]["y"])) };
    for c in &cases {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap();
        let inp = &c["input"];
        let e = &c["expected"];
        let (x, y) = fx(c);
        let data = Dataset::new(&x, &y);
        if let Some(stripped) = name.strip_prefix("fit/") {
            let names = names_of(&inp["param_names"]);
            let eq = custom_eq(inp["equation"].as_str().unwrap(), &names);
            let p0 = nums(&inp["p0"]);
            let fit = nl::fit(&eq, &data, &opts_with_initial(p0.clone())).unwrap();
            check_fit_block(&mut ck, c, &format!("{name} (parsed)"), e, &fit);
            let builtin = match stripped {
                "4pl_dose_response" => Some(equations::log_agonist_variable_slope()),
                "one_phase_decay" => Some(equations::one_phase_decay()),
                "michaelis_menten" => Some(equations::michaelis_menten()),
                "hill" => Some(equations::specific_binding_hill()),
                "gaussian" => Some(equations::gaussian()),
                _ => None,
            };
            if let Some(b) = builtin {
                // From the Prism-style initial-value rules, not the fixture's start.
                let fit = nl::fit(&b, &data, &FitOptions::default()).unwrap();
                check_fit_block(
                    &mut ck,
                    c,
                    &format!("{name} (built-in, rule starts)"),
                    e,
                    &fit,
                );
            }
        } else if name == "compare/3pl_vs_4pl" {
            let four = equations::log_agonist_variable_slope();
            let mut ps = nums(&inp["p0_simple"]);
            ps.push(1.0);
            let mut o = opts_with_initial(ps);
            o.constraints = Some(vec![
                ParamConstraint::free(),
                ParamConstraint::free(),
                ParamConstraint::free(),
                ParamConstraint::fixed(1.0),
            ]);
            let simple = nl::fit(&four, &data, &o).unwrap();
            let complex =
                nl::fit(&four, &data, &opts_with_initial(nums(&inp["p0_complex"]))).unwrap();
            check_compare(&mut ck, c, &simple, &complex);
        } else if name == "compare/one_vs_two_phase_decay" {
            let two = cases
                .iter()
                .find(|c| c["name"] == "fit/two_phase_decay")
                .unwrap();
            let eq2 = custom_eq(
                two["input"]["equation"].as_str().unwrap(),
                &names_of(&two["input"]["param_names"]),
            );
            let simple = nl::fit(
                &equations::one_phase_decay(),
                &data,
                &opts_with_initial(nums(&inp["p0_simple"])),
            )
            .unwrap();
            let complex =
                nl::fit(&eq2, &data, &opts_with_initial(nums(&inp["p0_complex"]))).unwrap();
            check_compare(&mut ck, c, &simple, &complex);
        } else if let Some(kind) = name.strip_prefix("weighted/") {
            let mut o = opts_with_initial(nums(&inp["p0"]));
            o.weighting = nl::Weighting::InverseY2;
            o.weight_base = if kind.ends_with("_data") {
                nl::WeightBase::Data
            } else {
                nl::WeightBase::Curve
            };
            let fit = nl::fit(&equations::michaelis_menten(), &data, &o).unwrap();
            check_fit_block(&mut ck, c, name, e, &fit);
            if let Some(v) = e.get("ss_weighted") {
                ck.check_value(name, "ss_weighted", v, fit.ss, tol(c, "ss_weighted"));
            }
        } else if name.starts_with("robust/") {
            let loss = match inp["loss"].as_str().unwrap() {
                "cauchy" => nl::RobustLoss::Cauchy,
                _ => nl::RobustLoss::SoftL1,
            };
            let mut o = opts_with_initial(nums(&inp["p0"]));
            o.method = nl::FitMethod::RobustFixedScale {
                loss,
                scale: num(&inp["f_scale"]).unwrap(),
            };
            let fit = nl::fit(&equations::log_agonist_variable_slope(), &data, &o).unwrap();
            let got = fit.params();
            ck.check_vec(name, "params", &nums(&e["params"]), &got, tol(c, "params"));
            ck.check_value(
                name,
                "objective_sum_rho",
                &e["objective_sum_rho"],
                fit.robust_objective.unwrap(),
                tol(c, "objective_sum_rho"),
            );
        } else {
            ck.fail(format!("{name}: case not handled"));
        }
    }
    ck
}

fn check_compare(ck: &mut Checker, c: &Value, simple: &nl::CurveFit, complex: &nl::CurveFit) {
    let name = c["name"].as_str().unwrap();
    let e = &c["expected"];
    check_fit_block(ck, c, &format!("{name} simple"), &e["simple_fit"], simple);
    check_fit_block(
        ck,
        c,
        &format!("{name} complex"),
        &e["complex_fit"],
        complex,
    );
    let cmp = nl::compare_models(simple, complex).unwrap();
    for (key, got) in [
        ("F", cmp.f),
        ("df_num", cmp.df_num),
        ("df_den", cmp.df_den),
        ("p", cmp.p),
        ("aicc_simple", cmp.aicc_simple),
        ("aicc_complex", cmp.aicc_complex),
        ("delta_aicc_simple_minus_complex", cmp.delta_aicc),
        ("prob_complex_correct", cmp.prob_complex),
    ] {
        chk(ck, c, e, key, got);
    }
}

// ------------------------------------------------------------------ NIST nonlinear

/// NIST StRD nonlinear sets, in the order of the NIST table (lower, average, higher).
pub const NIST_NONLINEAR: [&str; 27] = [
    "Misra1a", "Chwirut2", "Chwirut1", "Lanczos3", "Gauss1", "Gauss2", "DanWood", "Misra1b",
    "Kirby2", "Hahn1", "Nelson", "MGH17", "Lanczos1", "Lanczos2", "Gauss3", "Misra1c", "Misra1d",
    "Roszman1", "ENSO", "MGH09", "Thurber", "BoxBOD", "Rat42", "MGH10", "Eckerle4", "Rat43",
    "Bennett5",
];

/// One start of one NIST nonlinear set, scored (LRE capped at 11, the certified digits).
#[derive(Debug, Clone)]
pub struct NistNlStart {
    pub converged: bool,
    pub status: String,
    pub iterations: usize,
    pub params: f64,
    pub sds: f64,
    pub ss: f64,
    pub residual_sd: f64,
}

impl NistNlStart {
    pub fn min(&self) -> f64 {
        self.params.min(self.sds).min(self.ss).min(self.residual_sd)
    }
}

pub struct NistNlRow {
    pub name: String,
    pub difficulty: String,
    pub starts: [NistNlStart; 2],
}

/// Load a NIST nonlinear set as (equation, data).
pub fn nist_nonlinear_problem(j: &Value) -> (Equation, Dataset) {
    let preds = names_of(&j["predictors"]);
    let pv: Vec<&str> = preds.iter().map(|s| s.as_str()).collect();
    let pn: Vec<String> = j["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    let pr: Vec<&str> = pn.iter().map(|s| s.as_str()).collect();
    let eq = Equation::custom_with(j["equation"].as_str().unwrap(), &pv, Some(&pr)).unwrap();
    let mut y = nums(&j["data"]["y"]);
    if j["response_transform"] == "log" {
        for v in y.iter_mut() {
            *v = v.ln();
        }
    }
    let x = nums(&j["data"][preds[0].as_str()]);
    let mut d = Dataset::new(&x, &y);
    for p in &preds[1..] {
        d.extra_x.push(nums(&j["data"][p.as_str()]));
    }
    (eq, d)
}

pub fn nist_nonlinear() -> Vec<NistNlRow> {
    let l = |g: f64, c: f64| lre(g, c).clamp(0.0, 11.0);
    NIST_NONLINEAR
        .iter()
        .map(|&name| {
            let j = load(&format!("nist/{name}.json"));
            let (eq, data) = nist_nonlinear_problem(&j);
            let ps = j["parameters"].as_array().unwrap();
            let score = |start: &str| -> NistNlStart {
                let p0: Vec<f64> = ps.iter().map(|p| num(&p[start]).unwrap()).collect();
                let r = nl::fit(&eq, &data, &opts_with_initial(p0)).unwrap();
                let st = r.convergence.status;
                let converged = matches!(st, nl::FitStatus::Converged | nl::FitStatus::Ambiguous);
                let v = r.params();
                let se = r.se();
                NistNlStart {
                    converged,
                    status: format!("{st:?}"),
                    iterations: r.convergence.iterations,
                    params: ps
                        .iter()
                        .enumerate()
                        .map(|(k, p)| l(v[k], num(&p["certified"]).unwrap()))
                        .fold(f64::INFINITY, f64::min),
                    sds: ps
                        .iter()
                        .enumerate()
                        .map(|(k, p)| l(se[k], num(&p["std_dev"]).unwrap()))
                        .fold(f64::INFINITY, f64::min),
                    ss: l(r.ss, cert(&j["residual_sum_of_squares"]).unwrap()),
                    residual_sd: l(r.sy_x, cert(&j["residual_std_dev"]).unwrap()),
                }
            };
            NistNlRow {
                name: name.to_string(),
                difficulty: j["difficulty"].as_str().unwrap().to_string(),
                starts: [score("start1"), score("start2")],
            }
        })
        .collect()
}

// ------------------------------------------------------------------ survival

use dioptra_stats::survival::{self as sv, CoxTies, KmOptions, LogRankWeight};

fn surv_group(v: &Value) -> (Vec<f64>, Vec<bool>) {
    let t = nums(&v["time"]);
    let e = nums(&v["event"]).iter().map(|x| *x != 0.0).collect();
    (t, e)
}

/// Efron-ties Cox MLE (coef, SE, log-likelihood) for the survival fixture cases, from
/// Newton iterations in 50-digit decimal arithmetic (Python `decimal`, the score and
/// information written out for one binary covariate).
#[allow(clippy::excessive_precision)]
const COX_EXACT: [(&str, f64, f64, f64); 3] = [
    (
        "gehan_leukemia_classic",
        -1.5721251488290670,
        0.41239671770945558,
        -85.008424577371627,
    ),
    (
        "synthetic_ties_censoring_30_25",
        -0.56973776095324207,
        0.36345729490491777,
        -100.21568119293005,
    ),
    (
        "small_censor_at_end",
        -0.84574684893160798,
        0.71332884209670770,
        -17.930671038121084,
    ),
];

pub fn survival_family() -> Checker {
    let mut ck = Checker::new("survival (KM, log-rank family, hazard ratios, Cox)");
    for c in cases("survival") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let (t1, e1) = surv_group(&c["input"]["group1"]);
        let (t2, e2) = surv_group(&c["input"]["group2"]);
        for (g, (t, ev)) in [(1, (&t1, &e1)), (2, (&t2, &e2))] {
            let km = sv::kaplan_meier(t, ev, KmOptions::default()).unwrap();
            let rows = e[format!("km_group{g}")].as_array().unwrap();
            if rows.len() != km.rows.len() {
                ck.fail(format!(
                    "{name} group {g}: {} KM rows, expected {}",
                    km.rows.len(),
                    rows.len()
                ));
                continue;
            }
            for (k, (er, r)) in rows.iter().zip(&km.rows).enumerate() {
                let ctx = format!("{name} km_group{g}[{k}]");
                let tl = tol(&c, "km");
                for (key, got) in [
                    ("time", r.time),
                    ("n_at_risk", r.n_at_risk as f64),
                    ("events", r.events as f64),
                    ("censored_at_time", r.censored as f64),
                    ("survival", r.survival),
                    ("greenwood_var", r.greenwood_var),
                    ("se", r.se),
                ] {
                    ck.check_value(&ctx, key, &er[key], got, tl);
                }
                for (key, got) in [
                    ("ci95_loglog", r.ci_loglog),
                    ("ci95_linear_clipped", r.ci_linear),
                ] {
                    let pair = er[key].as_array().unwrap();
                    ck.check_value(&ctx, &format!("{key}.lo"), &pair[0], got.0, tl);
                    ck.check_value(&ctx, &format!("{key}.hi"), &pair[1], got.1, tl);
                }
            }
            chk(&mut ck, &c, e, &format!("median_group{g}"), km.median);
        }
        let groups: [(&[f64], &[bool]); 2] = [(&t1, &e1), (&t2, &e2)];
        for (key, w) in [
            ("logrank", LogRankWeight::LogRank),
            ("gehan_breslow_wilcoxon", LogRankWeight::Wilcoxon),
            ("tarone_ware", LogRankWeight::TaroneWare),
        ] {
            let lr = sv::logrank(&groups, w, None).unwrap();
            let t = tol(&c, key);
            ck.check_value(name, &format!("{key}.chi2"), &e[key]["chi2"], lr.chi2, t);
            ck.check_value(name, &format!("{key}.p"), &e[key]["p"], lr.p, t);
        }
        let hr = sv::hazard_ratio(groups[0], groups[1], 0.95).unwrap();
        let comp = &e["logrank_components"];
        let t = tol(&c, "logrank_components");
        for (key, got) in [
            ("O1", hr.o1),
            ("E1", hr.e1),
            ("O2", hr.o2),
            ("E2", hr.e2),
            ("V", hr.v),
            ("chi2_mantel_haenszel", hr.chi2),
            ("hr_logrank_OE", hr.hr_logrank),
            ("hr_mantel_haenszel", hr.hr_mantel_haenszel),
        ] {
            ck.check_value(name, key, &comp[key], got, t);
        }
        for (key, got) in [
            ("hr_logrank_OE_ci95", hr.hr_logrank_ci),
            ("hr_mantel_haenszel_ci95", hr.hr_mantel_haenszel_ci),
        ] {
            let pr = comp[key].as_array().unwrap();
            ck.check_value(name, &format!("{key}.lo"), &pr[0], got.0, t);
            ck.check_value(name, &format!("{key}.hi"), &pr[1], got.1, t);
        }
        // Cox with covariate group1 = 1
        let mut time = t1.clone();
        time.extend(&t2);
        let mut ev = e1.clone();
        ev.extend(&e2);
        let x: Vec<Vec<f64>> = (0..time.len())
            .map(|i| vec![if i < t1.len() { 1.0 } else { 0.0 }])
            .collect();
        let cox = sv::cox_ph(&time, &ev, &x, CoxTies::Efron, 0.95).unwrap();
        let cx = &e["cox_efron"];
        let cc = &cox.coefficients[0];
        // lifelines stops its Newton iterations early (its score at its own estimate is
        // -1.6e-6, -2.1e-7 and -4.6e-6 for the three cases), so the coefficient is checked
        // against the maximum found by Newton in 50-digit decimal arithmetic, and against
        // lifelines by the likelihood: ours must be at least as high.
        let exact = COX_EXACT.iter().find(|r| r.0 == name).unwrap();
        let te = Tol::new(1e-11, 1e-14);
        ck.check(name, "cox.coef (50-digit MLE)", exact.1, cc.coef, te);
        ck.check(name, "cox.se (50-digit MLE)", exact.2, cc.se, te);
        ck.check(
            name,
            "cox.log_likelihood (50-digit)",
            exact.3,
            cox.log_likelihood,
            te,
        );
        let t = tol(&c, "cox_efron");
        ck.check_value(
            name,
            "cox.log_likelihood",
            &cx["log_likelihood"],
            cox.log_likelihood,
            t,
        );
        let ll_lifelines =
            sv::cox_log_likelihood(&time, &ev, &x, &[num(&cx["coef"]).unwrap()], CoxTies::Efron)
                .unwrap();
        ck.count += 1;
        if cox.log_likelihood < ll_lifelines {
            ck.fail(format!(
                "{name}: our Cox log-likelihood {} below lifelines' {}",
                cox.log_likelihood, ll_lifelines
            ));
        }
        // The derived quantities follow from coef and se; compared with lifelines'
        // values at 1e-5 (its convergence error), the fixture asks 1e-6.
        let tl = Tol::new(1e-5, 1e-8);
        for (key, got) in [
            ("coef", cc.coef),
            ("se", cc.se),
            ("hr", cc.hr),
            ("p_wald", cc.p),
        ] {
            ck.check_value(name, &format!("cox.{key} (lifelines)"), &cx[key], got, tl);
        }
        let pr = cx["hr_ci95"].as_array().unwrap();
        ck.check_value(name, "cox.hr_ci95.lo (lifelines)", &pr[0], cc.hr_ci.0, tl);
        ck.check_value(name, "cox.hr_ci95.hi (lifelines)", &pr[1], cc.hr_ci.1, tl);
    }
    ck
}

// ------------------------------------------------------------------ ROC

use dioptra_stats::roc::{self as rocm, RocOptions};

pub fn roc_family() -> Checker {
    let mut ck = Checker::new("ROC (curve, Wilson CIs, AUC, DeLong, Hanley-McNeil, Youden)");
    for c in cases("roc") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let r = rocm::roc(
            &nums(&c["input"]["scores_positive"]),
            &nums(&c["input"]["scores_negative"]),
            RocOptions::default(),
        )
        .unwrap();
        chk(&mut ck, &c, e, "auc", r.auc);
        chk(&mut ck, &c, e, "delong_se", r.se_delong);
        chk(&mut ck, &c, e, "hanley_mcneil_se", r.se_hanley_mcneil);
        chk(&mut ck, &c, e, "p_auc_vs_0p5_delong_z", r.p);
        chk(
            &mut ck,
            &c,
            e,
            "youden_best_index",
            r.youden_best_index as f64,
        );
        chk(
            &mut ck,
            &c,
            e,
            "youden_best_threshold",
            r.youden_best_threshold,
        );
        chk(&mut ck, &c, e, "youden_best_J", r.youden_best_j);
        for (key, got) in [
            ("delong_ci95", r.ci_delong),
            ("hanley_mcneil_ci95", r.ci_hanley_mcneil),
        ] {
            let p = nums(&e[key]);
            ck.check(name, &format!("{key}.lo"), p[0], got.0, tol(&c, key));
            ck.check(name, &format!("{key}.hi"), p[1], got.1, tol(&c, key));
        }
        chk_vec(&mut ck, &c, e, "delong_v10", &r.v10);
        chk_vec(&mut ck, &c, e, "delong_v01", &r.v01);
        let col =
            |f: &dyn Fn(&rocm::RocPoint) -> f64| -> Vec<f64> { r.points.iter().map(f).collect() };
        chk_vec(&mut ck, &c, e, "thresholds", &col(&|p| p.threshold));
        chk_vec(&mut ck, &c, e, "tpr", &col(&|p| p.sensitivity));
        chk_vec(&mut ck, &c, e, "fpr", &col(&|p| p.fpr));
        chk_vec(&mut ck, &c, e, "sensitivity", &col(&|p| p.sensitivity));
        chk_vec(&mut ck, &c, e, "specificity", &col(&|p| p.specificity));
        chk_vec(&mut ck, &c, e, "youden_J", &col(&|p| p.youden));
        for (key, f) in [
            (
                "sensitivity_wilson_ci95",
                &(|p: &rocm::RocPoint| p.sensitivity_ci) as &dyn Fn(&rocm::RocPoint) -> (f64, f64),
            ),
            ("specificity_wilson_ci95", &|p: &rocm::RocPoint| {
                p.specificity_ci
            }),
        ] {
            let rows = e[key].as_array().unwrap();
            if rows.len() != r.points.len() {
                ck.fail(format!("{name}: {key} length"));
                continue;
            }
            for (k, (row, p)) in rows.iter().zip(&r.points).enumerate() {
                let v = nums(row);
                let g = f(p);
                ck.check(name, &format!("{key}[{k}].lo"), v[0], g.0, tol(&c, key));
                ck.check(name, &format!("{key}[{k}].hi"), v[1], g.1, tol(&c, key));
            }
        }
    }
    ck
}

// ------------------------------------------------------------------ Bland-Altman

use dioptra_stats::agreement::{self as agr, BaKind};

pub fn bland_altman_family() -> Checker {
    let mut ck = Checker::new("Bland-Altman (bias, limits, CIs, proportional bias)");
    for c in cases("bland_altman") {
        ck.cases += 1;
        let name = c["name"].as_str().unwrap();
        let e = &c["expected"];
        let r = agr::bland_altman(
            &nums(&c["input"]["a"]),
            &nums(&c["input"]["b"]),
            BaKind::Difference,
            0.95,
        )
        .unwrap();
        let pb = r.proportional_bias.as_ref().unwrap();
        for (key, got) in [
            ("bias", r.bias),
            ("sd_diff", r.sd),
            ("loa_lower", r.loa_lower),
            ("loa_upper", r.loa_upper),
            ("loa_se", r.loa_se),
            ("loa_lower_exact_z", r.loa_lower_exact_z),
            ("loa_upper_exact_z", r.loa_upper_exact_z),
            ("proportional_bias_slope", pb.slope),
            ("proportional_bias_intercept", pb.intercept),
            ("proportional_bias_p", pb.p),
            ("proportional_bias_slope_se", pb.slope_se),
        ] {
            chk(&mut ck, &c, e, key, got);
        }
        for (key, got) in [
            ("bias_ci95", r.bias_ci),
            ("loa_lower_ci95", r.loa_lower_ci),
            ("loa_upper_ci95", r.loa_upper_ci),
        ] {
            let p = nums(&e[key]);
            ck.check(name, &format!("{key}.lo"), p[0], got.0, tol(&c, key));
            ck.check(name, &format!("{key}.hi"), p[1], got.1, tol(&c, key));
        }
        chk_vec(&mut ck, &c, e, "differences", &r.differences);
        chk_vec(&mut ck, &c, e, "averages", &r.averages);
    }
    ck
}

// ------------------------------------------------------------------ PCA

use dioptra_stats::multivariate::{self as mv, PcaScaling};

pub fn pca_family() -> Checker {
    let mut ck = Checker::new("PCA (eigenvalues, eigenvectors, loadings, scores)");
    for c in cases("pca") {
        ck.cases += 1;
        let e = &c["expected"];
        let rows = matrix_of(&c["input"]["X"]);
        let r = mv::pca(&rows, PcaScaling::Standardize).unwrap();
        chk_vec(&mut ck, &c, e, "eigenvalues", &r.eigenvalues);
        chk_vec(
            &mut ck,
            &c,
            e,
            "explained_variance_ratio",
            &r.explained_variance_ratio,
        );
        chk_vec(&mut ck, &c, e, "cumulative_ratio", &r.cumulative_ratio);
        chk_vec(&mut ck, &c, e, "column_means", &r.column_means);
        chk_vec(&mut ck, &c, e, "column_sds", &r.column_sds);
        for (key, got) in [
            ("eigenvectors", &r.eigenvectors),
            ("loadings_scaled", &r.loadings),
            ("scores", &r.scores),
        ] {
            let rows_e = matrix_of(&e[key]);
            if rows_e.len() != got.len() {
                ck.fail(format!(
                    "{} :: {key}: {} rows vs {}",
                    c["name"],
                    got.len(),
                    rows_e.len()
                ));
                continue;
            }
            for (k, (re, rg)) in rows_e.iter().zip(got.iter()).enumerate() {
                ck.check_vec(
                    c["name"].as_str().unwrap(),
                    &format!("{key}[{k}]"),
                    re,
                    rg,
                    tol(&c, key),
                );
            }
        }
    }
    ck
}
