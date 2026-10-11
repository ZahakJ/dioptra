//! Principal component analysis by the singular value decomposition, with Kaiser's rule,
//! Horn's parallel analysis and scree data, and a covariance-matrix helper. (Pairwise
//! correlation matrices with p-values are in `correlation`.)

use crate::descriptive::compensated_sum;
use crate::resample::Xoshiro256;
use crate::{Result, StatsError};
use nalgebra::DMatrix;

fn invalid(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

/// How the columns are scaled before the decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PcaScaling {
    /// Centre and divide by the SD (n - 1): PCA of the correlation matrix (Prism's
    /// default, "standardize").
    #[default]
    Standardize,
    /// Centre only: PCA of the covariance matrix.
    Center,
}

/// PCA result. Components are ordered by decreasing eigenvalue.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pca {
    /// Rows used (rows with a missing value are dropped).
    pub n: usize,
    /// Variables.
    pub p: usize,
    /// Scaling.
    pub scaling: PcaScaling,
    /// Column means.
    pub column_means: Vec<f64>,
    /// Column SDs (n - 1).
    pub column_sds: Vec<f64>,
    /// Eigenvalues = singular values^2/(n - 1) (variances of the components).
    pub eigenvalues: Vec<f64>,
    /// Eigenvalue / sum.
    pub explained_variance_ratio: Vec<f64>,
    /// Cumulative explained ratio.
    pub cumulative_ratio: Vec<f64>,
    /// Eigenvectors, one row per component (component x variable), each flipped so that
    /// its entry of largest magnitude is positive (first on ties).
    pub eigenvectors: Vec<Vec<f64>>,
    /// Loadings: eigenvector x sqrt(eigenvalue) (for standardized data, the correlations
    /// between the variables and the component).
    pub loadings: Vec<Vec<f64>>,
    /// Scores: one row per observation, one column per component.
    pub scores: Vec<Vec<f64>>,
    /// Components with eigenvalue > 1 (Kaiser 1960; meaningful for standardized data) or
    /// above the mean eigenvalue (covariance PCA).
    pub kaiser_retained: usize,
}

fn complete_rows(rows: &[Vec<f64>]) -> Result<(Vec<&Vec<f64>>, usize)> {
    let p = rows.first().map_or(0, |r| r.len());
    if p == 0 {
        return Err(invalid("no variables"));
    }
    let keep: Vec<&Vec<f64>> = rows
        .iter()
        .filter(|r| r.len() == p && r.iter().all(|v| v.is_finite()))
        .collect();
    if keep.len() < 2 {
        return Err(StatsError::NotEnoughData(
            "need at least two complete rows".into(),
        ));
    }
    Ok((keep, p))
}

/// Covariance matrix (n - 1 denominator, two-pass compensated sums) of the complete rows,
/// p x p row-major.
pub fn covariance_matrix(rows: &[Vec<f64>]) -> Result<Vec<f64>> {
    let (keep, p) = complete_rows(rows)?;
    let n = keep.len() as f64;
    let means: Vec<f64> = (0..p)
        .map(|j| compensated_sum(keep.iter().map(|r| r[j])) / n)
        .collect();
    let mut c = vec![0.0; p * p];
    for a in 0..p {
        for b in 0..=a {
            let v = compensated_sum(keep.iter().map(|r| (r[a] - means[a]) * (r[b] - means[b])))
                / (n - 1.0);
            c[a * p + b] = v;
            c[b * p + a] = v;
        }
    }
    Ok(c)
}

/// Eigenvalues (descending) and right singular vectors of a centred/scaled n x p matrix.
fn svd_parts(z: &DMatrix<f64>) -> (Vec<f64>, DMatrix<f64>, DMatrix<f64>) {
    let svd = z.clone().svd(true, true);
    let u = svd.u.unwrap_or_else(|| DMatrix::zeros(0, 0));
    let vt = svd.v_t.unwrap_or_else(|| DMatrix::zeros(0, 0));
    (svd.singular_values.iter().copied().collect(), u, vt)
}

fn standardize(
    keep: &[&Vec<f64>],
    p: usize,
    scaling: PcaScaling,
) -> (DMatrix<f64>, Vec<f64>, Vec<f64>) {
    let n = keep.len();
    let nf = n as f64;
    let means: Vec<f64> = (0..p)
        .map(|j| compensated_sum(keep.iter().map(|r| r[j])) / nf)
        .collect();
    let sds: Vec<f64> = (0..p)
        .map(|j| {
            (compensated_sum(keep.iter().map(|r| (r[j] - means[j]) * (r[j] - means[j])))
                / (nf - 1.0))
                .sqrt()
        })
        .collect();
    let z = DMatrix::from_fn(n, p, |i, j| {
        let c = keep[i][j] - means[j];
        match scaling {
            PcaScaling::Standardize => c / sds[j],
            PcaScaling::Center => c,
        }
    });
    (z, means, sds)
}

/// Principal component analysis of the rows (observations) of `rows` by the SVD of the
/// centred (and, by default, standardized) data matrix Z = U S V' (Jolliffe 2002,
/// *Principal Component Analysis*, 2nd ed., ch. 1-3): eigenvalues S^2/(n-1), eigenvectors
/// the columns of V, scores U S. sklearn's `PCA` on the standardized matrix is the
/// oracle; the sign of each component is fixed so its largest-magnitude loading is
/// positive.
pub fn pca(rows: &[Vec<f64>], scaling: PcaScaling) -> Result<Pca> {
    let (keep, p) = complete_rows(rows)?;
    let n = keep.len();
    let (z, means, sds) = standardize(&keep, p, scaling);
    if scaling == PcaScaling::Standardize && sds.iter().any(|s| !(*s > 0.0)) {
        return Err(invalid("a variable is constant and cannot be standardized"));
    }
    let (sv, u, vt) = svd_parts(&z);
    let mut order: Vec<usize> = (0..sv.len()).collect();
    order.sort_by(|&a, &b| sv[b].total_cmp(&sv[a]));
    let k = sv.len();
    let nf1 = n as f64 - 1.0;
    let eig: Vec<f64> = order.iter().map(|&c| sv[c] * sv[c] / nf1).collect();
    let total: f64 = match scaling {
        PcaScaling::Standardize => p as f64,
        PcaScaling::Center => (0..p).map(|j| sds[j] * sds[j]).sum(),
    };
    let ratio: Vec<f64> = eig.iter().map(|e| e / total).collect();
    let mut cum = Vec::with_capacity(k);
    let mut acc = 0.0;
    for r in &ratio {
        acc += r;
        cum.push(acc);
    }
    let mut vecs = Vec::with_capacity(k);
    let mut scores = vec![vec![0.0; k]; n];
    for (c, &col) in order.iter().enumerate() {
        let mut v: Vec<f64> = (0..p).map(|j| vt[(col, j)]).collect();
        let mut big = 0;
        for j in 1..p {
            if v[j].abs() > v[big].abs() {
                big = j;
            }
        }
        let sign = if v[big] < 0.0 { -1.0 } else { 1.0 };
        for x in v.iter_mut() {
            *x *= sign;
        }
        for (i, row) in scores.iter_mut().enumerate() {
            row[c] = sign * u[(i, col)] * sv[col];
        }
        vecs.push(v);
    }
    let loadings: Vec<Vec<f64>> = vecs
        .iter()
        .zip(&eig)
        .map(|(v, e)| v.iter().map(|x| x * e.sqrt()).collect())
        .collect();
    let thresh = match scaling {
        PcaScaling::Standardize => 1.0,
        PcaScaling::Center => total / p as f64,
    };
    Ok(Pca {
        n,
        p,
        scaling,
        column_means: means,
        column_sds: sds,
        kaiser_retained: eig.iter().filter(|&&e| e > thresh).count(),
        eigenvalues: eig,
        explained_variance_ratio: ratio,
        cumulative_ratio: cum,
        eigenvectors: vecs,
        loadings,
        scores,
    })
}

/// Horn's parallel analysis.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParallelAnalysis {
    /// Mean eigenvalue per component over the random data sets.
    pub mean_eigenvalues: Vec<f64>,
    /// The `percentile` quantile per component.
    pub percentile_eigenvalues: Vec<f64>,
    /// The percentile used (e.g. 0.95).
    pub percentile: f64,
    /// Leading components of the data whose eigenvalue exceeds the percentile eigenvalue.
    pub retained: usize,
    /// Random data sets.
    pub replicates: usize,
    /// Seed.
    pub seed: u64,
}

/// Parallel analysis (Horn 1965, Psychometrika 30:179; Glorfeld 1995 percentile rule):
/// eigenvalues of `replicates` standard-normal data sets of the same n x p (seeded
/// xoshiro256**), scaled as the PCA was; a component is retained while its eigenvalue
/// exceeds the random eigenvalue at `percentile`.
pub fn parallel_analysis(
    fit: &Pca,
    replicates: usize,
    percentile: f64,
    seed: u64,
) -> Result<ParallelAnalysis> {
    if replicates < 2 || !(percentile > 0.0 && percentile < 1.0) {
        return Err(invalid("need >= 2 replicates and a percentile in (0, 1)"));
    }
    let (n, p) = (fit.n, fit.p);
    let k = fit.eigenvalues.len();
    let mut rng = Xoshiro256::new(seed);
    let mut all = vec![Vec::with_capacity(replicates); k];
    for _ in 0..replicates {
        let rows: Vec<Vec<f64>> = (0..n)
            .map(|_| (0..p).map(|_| rng.normal()).collect())
            .collect();
        let refs: Vec<&Vec<f64>> = rows.iter().collect();
        let (z, _, _) = standardize(&refs, p, fit.scaling);
        let (sv, _, _) = svd_parts(&z);
        let mut e: Vec<f64> = sv.iter().map(|s| s * s / (n as f64 - 1.0)).collect();
        e.sort_by(|a, b| b.total_cmp(a));
        for (c, a) in all.iter_mut().enumerate() {
            a.push(e.get(c).copied().unwrap_or(0.0));
        }
    }
    let mut mean = Vec::with_capacity(k);
    let mut pct = Vec::with_capacity(k);
    for v in all.iter_mut() {
        v.sort_by(f64::total_cmp);
        mean.push(v.iter().sum::<f64>() / v.len() as f64);
        pct.push(
            crate::descriptive::quantile_sorted(
                v,
                percentile,
                crate::descriptive::QuantileMethod::Linear,
            )
            .unwrap_or(f64::NAN),
        );
    }
    let retained = fit
        .eigenvalues
        .iter()
        .zip(&pct)
        .take_while(|(e, q)| e > q)
        .count();
    Ok(ParallelAnalysis {
        mean_eigenvalues: mean,
        percentile_eigenvalues: pct,
        percentile,
        retained,
        replicates,
        seed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covariance_and_pca_identities() {
        let rows = vec![
            vec![1.0, 2.0, 0.5],
            vec![2.0, 3.9, 0.1],
            vec![3.0, 6.1, 0.7],
            vec![4.0, 8.0, 0.2],
            vec![5.0, 9.8, 0.9],
        ];
        let c = covariance_matrix(&rows).unwrap();
        assert!((c[0] - 2.5).abs() < 1e-14);
        let f = pca(&rows, PcaScaling::Center).unwrap();
        // eigenvalues of the covariance matrix sum to its trace
        let tr = c[0] + c[4] + c[8];
        assert!((f.eigenvalues.iter().sum::<f64>() - tr).abs() < 1e-12);
        // scores have the eigenvalues as variances
        for k in 0..3 {
            let v: f64 = f.scores.iter().map(|r| r[k] * r[k]).sum::<f64>() / 4.0;
            assert!((v - f.eigenvalues[k]).abs() < 1e-12);
        }
        let s = pca(&rows, PcaScaling::Standardize).unwrap();
        assert!((s.cumulative_ratio[2] - 1.0).abs() < 1e-14);
    }

    #[test]
    fn parallel_analysis_finds_one_factor() {
        let mut rng = Xoshiro256::new(5);
        let rows: Vec<Vec<f64>> = (0..60)
            .map(|_| {
                let f = rng.normal();
                (0..5).map(|_| f + 0.6 * rng.normal()).collect()
            })
            .collect();
        let s = pca(&rows, PcaScaling::Standardize).unwrap();
        assert_eq!(s.kaiser_retained, 1);
        let pa = parallel_analysis(&s, 200, 0.95, 7).unwrap();
        assert_eq!(pa.retained, 1);
        assert!(pa.mean_eigenvalues[0] > 1.0 && pa.mean_eigenvalues[4] < 1.0);
    }
}
