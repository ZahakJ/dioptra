//! The formula language (DESIGN §4.2; reference in docs/FORMULAS.md).
//!
//! `parse` turns text into an [`Expr`]; `Display` prints it back in canonical form;
//! [`eval_column`] fills a column; [`eval_cell`] computes one cell.

mod ast;
mod eval;
mod parse;
pub mod units;

pub use ast::{BinOp, ColRef, ColTarget, Expr, UnOp};
pub use eval::{
    cell_value, compile, eval_cell, eval_column, evaluate, function_names, mean, quantile7,
    resolve, round_half_away, variance, ColumnResult, Compiled, Value,
};
pub use parse::{parse, ParseError};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SubcolumnFormat, TableKind};
    use crate::table::Table;

    fn pp(s: &str) -> String {
        parse(s).unwrap_or_else(|e| panic!("{s}: {e}")).to_string()
    }

    /// Golden pretty-printer output, and parse(print(parse(s))) == parse(s).
    #[test]
    fn parser_goldens() {
        let cases = [
            ("=log10(A)", "log10(A)"),
            ("(A - mean(A[1:3])) / sd(A)", "(A - mean(A[1:3])) / sd(A)"),
            (
                "normalize(B, min(B), max(B)) * 100",
                "normalize(B, min(B), max(B)) * 100",
            ),
            ("if(A > 0, A, NA)", "if(A > 0, A, NA)"),
            ("`Dose (nM)` * 1e-9", "`Dose (nM)` * 0.000000001"),
            ("B.2", "B.2"),
            ("lag(A, 1) - A", "lag(A, 1) - A"),
            (
                "convert(A, \"mg/dL\", \"mmol/L\", \"glucose\")",
                "convert(A, \"mg/dL\", \"mmol/L\", \"glucose\")",
            ),
            ("A3 * 2", "A3 * 2"),
            ("A[3]", "A3"),
            ("1+2*3", "1 + 2 * 3"),
            ("(1+2)*3", "(1 + 2) * 3"),
            ("2^3^2", "2 ^ 3 ^ 2"),
            ("(2^3)^2", "(2 ^ 3) ^ 2"),
            ("-2^2", "-2 ^ 2"),
            ("(-2)^2", "(-2) ^ 2"),
            ("a - (b - c)", ""),
            ("A - (B - C)", "A - (B - C)"),
            ("A - B - C", "A - B - C"),
            ("A / (B * C)", "A / (B * C)"),
            (
                "A > 1 && B < 2 || not C == 3",
                "A > 1 && B < 2 || not C == 3",
            ),
            ("A > 1 and (B < 2 or C)", "A > 1 && (B < 2 || C)"),
            ("!(A > 1)", "not A > 1"),
            ("(not A) > 1", "(not A) > 1"),
            ("A <> B", "A != B"),
            ("A = B", "A == B"),
            ("A % 2", "A % 2"),
            ("X * 2", "X * 2"),
            ("X3", "X3"),
            ("AB12 + `Weight (g)`.2", "AB12 + `Weight (g)`.2"),
            ("A.2[3]", "A.2[3]"),
            ("`a``b`", "`a``b`"),
            ("[1, 2, 3]", "[1, 2, 3]"),
            ("A[2:5]", "A[2:5]"),
            ("A[:5] + A[2:]", "A[:5] + A[2:]"),
            ("quantile(A, 0.25)", "quantile(A, 0.25)"),
            ("MEAN(A)", "mean(A)"),
            ("TRUE", "true"),
            ("- -A", "-(-A)"),
            ("2 ^ -1", "2 ^ (-1)"),
            ("\"say \\\"hi\\\"\"", "\"say \\\"hi\\\"\""),
            (".5 + 1.", "0.5 + 1"),
        ];
        for (src, want) in cases {
            if want.is_empty() {
                assert!(parse(src).is_err(), "{src} should not parse");
                continue;
            }
            let got = pp(src);
            assert_eq!(got, want, "printing {src}");
            assert_eq!(
                parse(&got).unwrap(),
                parse(src).unwrap(),
                "round trip of {src}"
            );
        }
    }

    #[test]
    fn parse_errors_are_helpful() {
        for (src, frag) in [
            ("", "empty"),
            ("(1 + 2", "expected ')'"),
            ("1 +", "ends too early"),
            ("foo + 1", "unknown name 'foo'"),
            ("A $ B", "unexpected character"),
            ("\"abc", "unclosed string"),
            ("1 2", "unexpected number"),
        ] {
            let e = parse(src).unwrap_err().to_string();
            assert!(e.contains(frag), "{src}: {e}");
        }
    }

    fn table() -> Table {
        let mut t = Table::new(
            "t".into(),
            TableKind::Xy,
            "T".into(),
            3,
            SubcolumnFormat::Values(1),
            0,
        );
        t.datasets[0].name = "Dose (nM)".into();
        let xs = ["1", "2", "3", "4", "5"];
        let a = ["10", "20", "", "40", "50"];
        let b = ["1", "4", "9", "oops", "25"];
        for i in 0..5 {
            t.set_cell_raw(i, 0, xs[i]).unwrap();
            t.set_cell_raw(i, 1, a[i]).unwrap();
            t.set_cell_raw(i, 2, b[i]).unwrap();
        }
        t
    }

    fn col(t: &Table, src: &str) -> Vec<String> {
        let r = eval_column(&parse(src).unwrap(), t).unwrap_or_else(|e| panic!("{src}: {e}"));
        (0..t.rows)
            .map(|i| {
                if let Some(e) = r.errors.get(&i) {
                    format!("E:{e}")
                } else if let Some(s) = r.text.get(&i) {
                    format!("T:{s}")
                } else if r.values[i].is_nan() {
                    "NA".into()
                } else {
                    crate::util::format_number(r.values[i], None)
                }
            })
            .collect()
    }

    fn scalar(t: &Table, src: &str) -> Value {
        evaluate(&parse(src).unwrap(), t).unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    #[test]
    fn evaluator_goldens() {
        let t = table();
        let cases: Vec<(&str, Vec<&str>)> = vec![
            ("A * 2", vec!["20", "40", "NA", "80", "100"]),
            ("`Dose (nM)` / 10", vec!["1", "2", "NA", "4", "5"]),
            ("X + 1", vec!["2", "3", "4", "5", "6"]),
            ("log10(A)", vec!["1", "1.30103", "NA", "1.60206", "1.69897"]),
            (
                "sqrt(B)",
                vec!["1", "2", "3", "E:\"oops\" is text, not a number", "5"],
            ),
            ("if(A > 15, 1, 0)", vec!["0", "1", "NA", "1", "1"]),
            ("coalesce(A, -1)", vec!["10", "20", "-1", "40", "50"]),
            ("isna(A)", vec!["0", "0", "1", "0", "0"]),
            ("A - mean(A)", vec!["-20", "-10", "NA", "10", "20"]),
            ("lag(X, 1)", vec!["NA", "1", "2", "3", "4"]),
            ("lead(X)", vec!["2", "3", "4", "5", "NA"]),
            ("diff(X)", vec!["NA", "1", "1", "1", "1"]),
            ("cumsum(A)", vec!["10", "30", "NA", "70", "120"]),
            ("rank(A)", vec!["1", "2", "NA", "3", "4"]),
            ("normalize(X)", vec!["0", "0.25", "0.5", "0.75", "1"]),
            ("normalize(X, 1, 3)", vec!["0", "0.5", "1", "1.5", "2"]),
            ("pct(A, 50)", vec!["20", "40", "NA", "80", "100"]),
            (
                "baseline(X, 1, 2)",
                vec!["-0.5", "0.5", "1.5", "2.5", "3.5"],
            ),
            ("baseline(X, seq(1, 3))", vec!["-1", "0", "1", "2", "3"]),
            ("clamp(X, 2, 4)", vec!["2", "2", "3", "4", "4"]),
            ("round(X / 3, 2)", vec!["0.33", "0.67", "1", "1.33", "1.67"]),
            ("X % 2", vec!["1", "0", "1", "0", "1"]),
            ("A1 + X", vec!["11", "12", "13", "14", "15"]),
            ("A[2:5] * 0 + 1", vec![]),
            ("[1,2,3,4,5] * X", vec!["1", "4", "9", "16", "25"]),
            ("row()", vec!["1", "2", "3", "4", "5"]),
            (
                "1 / (X - 3)",
                vec!["-0.5", "-1", "E:division by zero", "1", "0.5"],
            ),
            (
                "zscore(X)",
                vec!["-1.26491", "-0.632456", "0", "0.632456", "1.26491"],
            ),
            ("X > 2 && A > 30", vec!["0", "0", "NA", "1", "1"]),
            ("X < 2 || A > 30", vec!["1", "0", "NA", "1", "1"]),
            ("X > 4 || A > 1000", vec!["0", "0", "NA", "0", "1"]),
            (
                "convert(X, \"h\", \"min\")",
                vec!["60", "120", "180", "240", "300"],
            ),
            (
                "B.1 + 0",
                vec!["1", "4", "9", "E:\"oops\" is text, not a number", "25"],
            ),
            (
                "if(isna(A), \"none\", \"some\")",
                vec!["T:some", "T:some", "T:none", "T:some", "T:some"],
            ),
            ("2", vec!["2", "2", "2", "2", "2"]),
        ];
        for (src, want) in cases {
            if want.is_empty() {
                assert!(
                    eval_column(&parse(src).unwrap(), &t).is_err(),
                    "{src} should fail (length mismatch)"
                );
                continue;
            }
            assert_eq!(col(&t, src), want, "{src}");
        }
    }

    #[test]
    fn aggregates() {
        let t = table();
        let n = |s: &str| match scalar(&t, s) {
            Value::Number(x) => x,
            v => panic!("{s}: {v:?}"),
        };
        assert_eq!(n("mean(A)"), 30.0);
        assert_eq!(n("median(A)"), 30.0);
        assert_eq!(n("count(A)"), 4.0);
        assert_eq!(n("sum(A)"), 120.0);
        assert_eq!(n("min(A)"), 10.0);
        assert_eq!(n("max(A, X)"), 50.0);
        assert_eq!(n("range(A)"), 40.0);
        assert!((n("sd(A)") - 18.257418583505537).abs() < 1e-12);
        assert!((n("sem(A)") - 9.128709291752768).abs() < 1e-12);
        assert!((n("var(A)") - 333.3333333333333).abs() < 1e-9);
        assert_eq!(n("quantile(X, 0.25)"), 2.0);
        assert!((n("geomean(X)") - 2.605171084697352).abs() < 1e-12);
        assert!((n("cv(X)") - 52.70462766947299).abs() < 1e-9);
        assert_eq!(n("first(A)"), 10.0);
        assert_eq!(n("last(A)"), 50.0);
        assert_eq!(n("mean(A[1:2])"), 15.0);
        assert_eq!(n("A[4]"), 40.0);
        assert_eq!(n("mean([1, 2, 3])"), 2.0);
        assert_eq!(n("count(B)"), 4.0);
        assert!(matches!(scalar(&t, "A[3]"), Value::Missing));
        assert!(matches!(scalar(&t, "sd(A1)"), Value::Error(_)));
        assert!(matches!(scalar(&t, "geomean(X - 3)"), Value::Error(_)));
    }

    #[test]
    fn excluded_values_are_missing() {
        let mut t = table();
        t.datasets[0].subcolumns[0].excluded.insert(4);
        assert_eq!(scalar(&t, "mean(A)"), Value::Number(70.0 / 3.0));
        assert_eq!(col(&t, "A + 0")[4], "NA");
    }

    #[test]
    fn errors_name_the_problem() {
        let t = table();
        let e = |s: &str| eval_column(&parse(s).unwrap(), &t).unwrap_err();
        assert!(e("D * 2").contains("no data set D"));
        assert!(e("`Nope` + 1").contains("no column named"));
        assert!(e("A.2").contains("does not exist"));
        assert!(e("frob(A)").contains("unknown function"));
        assert!(e("sqrt(A, B)").contains("takes 1 argument"));
        assert!(e("convert(A, \"mg/dL\", \"mmol/L\")").contains("substance"));
        assert!(e("quantile(A, X)").contains("single number"));
    }

    #[test]
    fn round_like_a_spreadsheet() {
        assert_eq!(round_half_away(2.675, 2), 2.68);
        assert_eq!(round_half_away(2.5, 0), 3.0);
        assert_eq!(round_half_away(-2.5, 0), -3.0);
        assert_eq!(round_half_away(1234.0, -2), 1200.0);
    }

    #[test]
    fn cell_formula_values() {
        let t = table();
        assert_eq!(
            eval_cell(&parse("A2 * 2").unwrap(), &t, 0),
            Value::Number(40.0)
        );
        // Implicit intersection: a column in a cell formula reads the cell's own row.
        assert_eq!(
            eval_cell(&parse("X * 10").unwrap(), &t, 2),
            Value::Number(30.0)
        );
        assert!(matches!(
            eval_cell(&parse("A[1:2]").unwrap(), &t, 0),
            Value::Error(_)
        ));
    }
}
