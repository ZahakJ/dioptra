//! Shared helpers for the data-driven fixture tests.
#![allow(dead_code)]

use serde_json::Value;

pub mod families;

/// Load `fixtures/<rel>` relative to the workspace root.
pub fn load(rel: &str) -> Value {
    let path = format!("{}/../../fixtures/{}", env!("CARGO_MANIFEST_DIR"), rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

/// The cases of an oracle fixture file.
pub fn cases(file: &str) -> Vec<Value> {
    load(&format!("oracle/{file}.json"))["cases"]
        .as_array()
        .expect("cases")
        .clone()
}

/// Find a case by name.
pub fn case<'a>(cases: &'a [Value], name: &str) -> &'a Value {
    cases
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

/// A tolerance: pass if |got-exp| <= abs or |got-exp| <= rel*|exp|.
#[derive(Debug, Clone, Copy)]
pub struct Tol {
    pub rel: f64,
    pub abs: f64,
}

impl Tol {
    pub fn new(rel: f64, abs: f64) -> Self {
        Tol { rel, abs }
    }
}

/// Tolerance of `case` for `field` (field overrides apply to the key and everything nested).
pub fn tol(case: &Value, field: &str) -> Tol {
    let t = &case["tolerance"];
    let top = field.split(['.', '[']).next().unwrap_or(field);
    let f = &t["fields"][top];
    if f.is_object() {
        return Tol::new(f["rel"].as_f64().unwrap(), f["abs"].as_f64().unwrap());
    }
    Tol::new(t["rel"].as_f64().unwrap(), t["abs"].as_f64().unwrap())
}

/// Parse a fixture number: JSON number, or the strings "Infinity", "-Infinity", "NaN".
pub fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            "NaN" => Some(f64::NAN),
            _ => None,
        },
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Vector of numbers.
pub fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap_or_else(|| panic!("not an array: {v}"))
        .iter()
        .map(|x| num(x).expect("number"))
        .collect()
}

/// Accumulates comparisons; panics at the end with every failure listed.
#[derive(Default)]
pub struct Checker {
    pub failures: Vec<String>,
    pub count: usize,
    pub max_rel: f64,
    pub min_digits: f64,
    pub family: String,
    /// Number of fixture cases (or datasets) visited.
    pub cases: usize,
    /// Largest |got - expected| / allowed error over all comparisons (< 1 means pass).
    pub max_tol_ratio: f64,
}

impl Checker {
    pub fn new(family: &str) -> Self {
        Checker {
            family: family.to_string(),
            min_digits: f64::INFINITY,
            ..Default::default()
        }
    }

    /// Compare one number.
    pub fn check(&mut self, ctx: &str, field: &str, expected: f64, got: f64, tol: Tol) {
        self.count += 1;
        if expected.is_nan() {
            if !got.is_nan() {
                self.failures
                    .push(format!("{ctx} :: {field}: expected NaN, got {got:e}"));
            }
            return;
        }
        if expected.is_infinite() {
            if got != expected {
                self.failures.push(format!(
                    "{ctx} :: {field}: expected {expected}, got {got:e}"
                ));
            }
            return;
        }
        let err = (got - expected).abs();
        let rel = if expected != 0.0 {
            err / expected.abs()
        } else {
            err
        };
        if err > 0.0 && expected.abs() > 1e-290 {
            self.max_rel = self.max_rel.max(rel);
            self.min_digits = self.min_digits.min(-(rel.max(1e-17)).log10());
        } else if err == 0.0 {
            self.min_digits = self.min_digits.min(17.0);
        }
        let ok = err <= tol.abs || err <= tol.rel * expected.abs();
        let allowed = tol.abs.max(tol.rel * expected.abs());
        if allowed > 0.0 {
            self.max_tol_ratio = self.max_tol_ratio.max(err / allowed);
        } else if err > 0.0 {
            self.max_tol_ratio = f64::INFINITY;
        }
        if std::env::var("DIOPTRA_SHOW_REL")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .is_some_and(|thr| rel > thr && expected.abs() > 1e-290)
        {
            println!("  {ctx} :: {field}: rel {rel:.3e}");
        }
        if !ok || got.is_nan() {
            self.failures.push(format!(
                "{ctx} :: {field}: expected {expected:.17e}, got {got:.17e}, abs err {err:.3e}, rel err {rel:.3e} (tol rel {:e} abs {:e})",
                tol.rel, tol.abs
            ));
        }
    }

    /// Compare a fixture value (number, null or array) against `got`.
    pub fn check_value(&mut self, ctx: &str, field: &str, expected: &Value, got: f64, tol: Tol) {
        match num(expected) {
            Some(e) => self.check(ctx, field, e, got, tol),
            None => {
                if !expected.is_null() {
                    self.failures
                        .push(format!("{ctx} :: {field}: unparseable expected {expected}"));
                } else if !got.is_nan() {
                    self.failures.push(format!(
                        "{ctx} :: {field}: expected null (undefined), got {got:e}"
                    ));
                }
            }
        }
    }

    /// Compare arrays elementwise.
    pub fn check_vec(&mut self, ctx: &str, field: &str, expected: &[f64], got: &[f64], tol: Tol) {
        if expected.len() != got.len() {
            self.failures.push(format!(
                "{ctx} :: {field}: length {} vs {}",
                expected.len(),
                got.len()
            ));
            return;
        }
        for (i, (e, g)) in expected.iter().zip(got).enumerate() {
            self.check(ctx, &format!("{field}[{i}]"), *e, *g, tol);
        }
    }

    /// Record a non-numeric failure.
    pub fn fail(&mut self, msg: String) {
        self.count += 1;
        self.failures.push(msg);
    }

    /// Panic with the full list of failures, if any; print a summary line otherwise.
    pub fn finish(self) {
        println!(
            "[{}] {} comparisons, max rel err {:.3e}, min digits {:.2}",
            self.family, self.count, self.max_rel, self.min_digits
        );
        if !self.failures.is_empty() {
            let n = self.failures.len();
            let shown: Vec<_> = self.failures.into_iter().take(80).collect();
            panic!(
                "{} of {} comparisons failed in {}:\n{}",
                n,
                self.count,
                self.family,
                shown.join("\n")
            );
        }
    }
}
