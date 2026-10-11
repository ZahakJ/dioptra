//! Ranking with ties (midranks), the basis of every rank test.

/// Midranks (average ranks for ties, 1-based) of `x`, and the sizes of the tie groups
/// that contain two or more equal values (groups of size 1 are omitted; they contribute
/// nothing to tie corrections). `NaN`s should be removed by the caller; if present they
/// are ranked last (IEEE total order) rather than causing a panic.
///
/// Reference: Kendall (1945) midrank convention; identical to SciPy `rankdata(method='average')`.
pub fn rank_with_ties(x: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let n = x.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let mut ranks = vec![0.0; n];
    let mut ties = Vec::new();
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && x[idx[j]] == x[idx[i]] {
            j += 1;
        }
        // positions i..j (0-based) share the average of ranks i+1..=j
        let r = 0.5 * ((i + 1) + j) as f64;
        for &k in &idx[i..j] {
            ranks[k] = r;
        }
        if j - i > 1 {
            ties.push(j - i);
        }
        i = j;
    }
    (ranks, ties)
}

/// sum over tie groups of (t^3 - t), the quantity in every rank-test tie correction.
pub fn tie_term(tie_sizes: &[usize]) -> f64 {
    tie_sizes
        .iter()
        .map(|&t| {
            let t = t as f64;
            t * t * t - t
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midranks() {
        let (r, t) = rank_with_ties(&[3.0, 1.0, 3.0, 2.0, 3.0]);
        assert_eq!(r, vec![4.0, 1.0, 4.0, 2.0, 4.0]);
        assert_eq!(t, vec![3]);
        assert_eq!(tie_term(&t), 24.0);
    }
}
