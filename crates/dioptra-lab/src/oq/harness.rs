// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The fixture helpers of `crates/dioptra-stats/tests/common/mod.rs`, reading the fixtures
//! embedded in the build and recording every comparison for the OQ report.

#![allow(dead_code)]

pub use serde_json::Value;

/// Parse an embedded fixture, `nist/<name>.json` or `oracle/<name>.json`.
pub fn load(rel: &str) -> Value {
    let text = super::fixtures::get(rel).unwrap_or_else(|| panic!("no embedded fixture {rel}"));
    serde_json::from_str(text).unwrap_or_else(|e| panic!("cannot parse {rel}: {e}"))
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

/// One comparison as recorded.
#[derive(Debug, Clone)]
pub struct Compared {
    /// Case and context.
    pub ctx: String,
    /// Quantity.
    pub field: String,
    pub expected: f64,
    pub got: f64,
    pub tol: Option<Tol>,
    pub pass: bool,
    /// Why it failed, when it did.
    pub note: String,
}

/// Accumulates comparisons, as the test checker does, keeping each one.
#[derive(Default)]
pub struct Checker {
    pub failures: Vec<String>,
    pub count: usize,
    pub family: String,
    pub cases: usize,
    pub max_rel: f64,
    pub min_digits: f64,
    pub max_tol_ratio: f64,
    pub compared: Vec<Compared>,
}

impl Checker {
    pub fn new(family: &str) -> Self {
        Checker {
            family: family.to_string(),
            min_digits: f64::INFINITY,
            ..Default::default()
        }
    }

    fn record(
        &mut self,
        ctx: &str,
        field: &str,
        expected: f64,
        got: f64,
        tol: Option<Tol>,
        note: Option<String>,
    ) {
        let pass = note.is_none();
        if let Some(n) = &note {
            self.failures.push(n.clone());
        }
        self.compared.push(Compared {
            ctx: ctx.to_string(),
            field: field.to_string(),
            expected,
            got,
            tol,
            pass,
            note: note.unwrap_or_default(),
        });
    }

    /// Compare one number.
    pub fn check(&mut self, ctx: &str, field: &str, expected: f64, got: f64, tol: Tol) {
        self.count += 1;
        if expected.is_nan() {
            let note =
                (!got.is_nan()).then(|| format!("{ctx} :: {field}: expected NaN, got {got:e}"));
            self.record(ctx, field, expected, got, Some(tol), note);
            return;
        }
        if expected.is_infinite() {
            let note = (got != expected)
                .then(|| format!("{ctx} :: {field}: expected {expected}, got {got:e}"));
            self.record(ctx, field, expected, got, Some(tol), note);
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
        let note = (!ok || got.is_nan()).then(|| {
            format!(
                "{ctx} :: {field}: expected {expected:.17e}, got {got:.17e}, abs err {err:.3e}, rel err {rel:.3e} (tol rel {:e} abs {:e})",
                tol.rel, tol.abs
            )
        });
        self.record(ctx, field, expected, got, Some(tol), note);
    }

    /// Compare a fixture value (number, null or array) against `got`. An expected `null`
    /// (undefined) is not counted as a comparison, as in the test suite; it fails when `got`
    /// is defined.
    pub fn check_value(&mut self, ctx: &str, field: &str, expected: &Value, got: f64, tol: Tol) {
        match num(expected) {
            Some(e) => self.check(ctx, field, e, got, tol),
            None => {
                if !expected.is_null() {
                    let n = format!("{ctx} :: {field}: unparseable expected {expected}");
                    self.record(ctx, field, f64::NAN, got, None, Some(n));
                } else if !got.is_nan() {
                    let n = format!("{ctx} :: {field}: expected null (undefined), got {got:e}");
                    self.record(ctx, field, f64::NAN, got, None, Some(n));
                }
            }
        }
    }

    /// Compare arrays elementwise.
    pub fn check_vec(&mut self, ctx: &str, field: &str, expected: &[f64], got: &[f64], tol: Tol) {
        if expected.len() != got.len() {
            let n = format!(
                "{ctx} :: {field}: length {} vs {}",
                expected.len(),
                got.len()
            );
            self.record(
                ctx,
                field,
                expected.len() as f64,
                got.len() as f64,
                None,
                Some(n),
            );
            return;
        }
        for (i, (e, g)) in expected.iter().zip(got).enumerate() {
            self.check(ctx, &format!("{field}[{i}]"), *e, *g, tol);
        }
    }

    /// Record a non-numeric failure.
    pub fn fail(&mut self, msg: String) {
        self.count += 1;
        self.record(&msg, "", f64::NAN, f64::NAN, None, Some(msg.clone()));
    }

    /// Count one yes/no comparison; `msg` is the failure.
    pub fn assert(&mut self, ctx: &str, field: &str, ok: bool, msg: String) {
        self.count += 1;
        let (e, g) = (1.0, if ok { 1.0 } else { 0.0 });
        self.record(ctx, field, e, g, None, (!ok).then_some(msg));
    }
}
