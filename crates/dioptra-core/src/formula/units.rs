//! Unit conversion for `convert(x, "from", "to"[, "substance"])`.
//!
//! A unit is a factor to an SI-like base and a dimension vector. Compound units are written
//! `a/b` (one slash). Temperatures are affine and only convert among C, F and K. Mass and
//! molar concentrations convert into each other through a substance's molar mass; the
//! clinical substances are tabulated so that the textbook factors come out exactly:
//! glucose mg/dL ÷ 18.016 = mmol/L, cholesterol ÷ 38.67, triglycerides ÷ 88.57, and
//! creatinine mg/dL × 88.42 = µmol/L.

/// Dimension exponents: mass, volume, amount, time, length, pressure.
type Dim = [i8; 6];

const MASS: Dim = [1, 0, 0, 0, 0, 0];
const VOLUME: Dim = [0, 1, 0, 0, 0, 0];
const AMOUNT: Dim = [0, 0, 1, 0, 0, 0];
const TIME: Dim = [0, 0, 0, 1, 0, 0];
const LENGTH: Dim = [0, 0, 0, 0, 1, 0];
const PRESSURE: Dim = [0, 0, 0, 0, 0, 1];
const MOLAR: Dim = [0, -1, 1, 0, 0, 0];

#[derive(Debug, Clone, Copy, PartialEq)]
struct Unit {
    factor: f64,
    dim: Dim,
}

fn prefix(p: &str) -> Option<f64> {
    Some(match p {
        "" => 1.0,
        "k" => 1e3,
        "h" => 1e2,
        "d" => 1e-1,
        "c" => 1e-2,
        "m" => 1e-3,
        "µ" | "μ" | "u" | "mc" => 1e-6,
        "n" => 1e-9,
        "p" => 1e-12,
        "f" => 1e-15,
        _ => return None,
    })
}

fn simple(name: &str) -> Option<Unit> {
    let exact = |factor, dim| Some(Unit { factor, dim });
    match name {
        "min" => return exact(60.0, TIME),
        "h" | "hr" | "hour" | "hours" => return exact(3600.0, TIME),
        "d" | "day" | "days" => return exact(86_400.0, TIME),
        "wk" | "week" | "weeks" => return exact(604_800.0, TIME),
        "sec" => return exact(1.0, TIME),
        "mmHg" => return exact(133.322_387_415, PRESSURE),
        "atm" => return exact(101_325.0, PRESSURE),
        "bar" => return exact(1e5, PRESSURE),
        "cc" => return exact(1e-3, VOLUME),
        _ => {}
    }
    // Prefix + base unit; the base is matched from the end.
    for (base, dim) in [
        ("mol", AMOUNT),
        ("Pa", PRESSURE),
        ("g", MASS),
        ("L", VOLUME),
        ("l", VOLUME),
        ("M", MOLAR),
        ("s", TIME),
        ("m", LENGTH),
    ] {
        if let Some(p) = name.strip_suffix(base) {
            if let Some(f) = prefix(p) {
                // Volume base is the litre; molar base is mol/L.
                return Some(Unit { factor: f, dim });
            }
        }
    }
    None
}

fn unit(name: &str) -> Option<Unit> {
    let name = name.trim();
    if let Some((a, b)) = name.split_once('/') {
        let (a, b) = (simple(a.trim())?, simple(b.trim())?);
        let mut dim = a.dim;
        for (d, e) in dim.iter_mut().zip(b.dim) {
            *d -= e;
        }
        return Some(Unit {
            factor: a.factor / b.factor,
            dim,
        });
    }
    simple(name)
}

/// Molar mass in g/mol for the named substance.
fn molar_mass(substance: &str) -> Option<f64> {
    Some(match substance.trim().to_ascii_lowercase().as_str() {
        // mg/dL -> mmol/L divides by M/10.
        "glucose" => 180.16,
        "cholesterol" => 386.7,
        "triglycerides" | "triglyceride" => 885.7,
        // mg/dL -> µmol/L multiplies by 10000/M = 88.42.
        "creatinine" => 10_000.0 / 88.42,
        "urea" | "bun" => 60.06,
        _ => return None,
    })
}

fn temp(name: &str) -> Option<char> {
    match name.trim() {
        "C" | "°C" | "degC" => Some('C'),
        "F" | "°F" | "degF" => Some('F'),
        "K" => Some('K'),
        _ => None,
    }
}

/// A resolved conversion: `y = x * factor + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conversion {
    /// Multiplier.
    pub factor: f64,
    /// Offset added after multiplying.
    pub offset: f64,
}

/// Resolve a conversion between two units, optionally through a substance's molar mass.
pub fn conversion(from: &str, to: &str, substance: Option<&str>) -> Result<Conversion, String> {
    if let (Some(a), Some(b)) = (temp(from), temp(to)) {
        // To kelvin, then from kelvin.
        let (fa, oa) = match a {
            'C' => (1.0, 273.15),
            'F' => (5.0 / 9.0, 273.15 - 32.0 * 5.0 / 9.0),
            _ => (1.0, 0.0),
        };
        let (fb, ob) = match b {
            'C' => (1.0, 273.15),
            'F' => (5.0 / 9.0, 273.15 - 32.0 * 5.0 / 9.0),
            _ => (1.0, 0.0),
        };
        // k = x*fa + oa; y = (k - ob)/fb
        return Ok(Conversion {
            factor: fa / fb,
            offset: (oa - ob) / fb,
        });
    }
    let ua = unit(from).ok_or_else(|| format!("unknown unit \"{from}\""))?;
    let ub = unit(to).ok_or_else(|| format!("unknown unit \"{to}\""))?;
    if ua.dim == ub.dim {
        return Ok(Conversion {
            factor: ua.factor / ub.factor,
            offset: 0.0,
        });
    }
    // Mass <-> amount through molar mass: dims differ by +mass -amount.
    let mut diff = [0i8; 6];
    for (d, (a, b)) in diff.iter_mut().zip(ua.dim.iter().zip(ub.dim.iter())) {
        *d = a - b;
    }
    let mass_to_amount = diff == [1, 0, -1, 0, 0, 0];
    let amount_to_mass = diff == [-1, 0, 1, 0, 0, 0];
    if mass_to_amount || amount_to_mass {
        let Some(s) = substance else {
            return Err(format!(
                "converting \"{from}\" to \"{to}\" needs the substance, e.g. convert(A, \"{from}\", \"{to}\", \"glucose\"); known: glucose, cholesterol, triglycerides, creatinine, urea"
            ));
        };
        let m = molar_mass(s).ok_or_else(|| format!("unknown substance \"{s}\""))?;
        let base = ua.factor / ub.factor;
        let factor = if mass_to_amount { base / m } else { base * m };
        return Ok(Conversion {
            factor,
            offset: 0.0,
        });
    }
    Err(format!(
        "cannot convert \"{from}\" to \"{to}\": different kinds of quantity"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(a: &str, b: &str, s: Option<&str>, x: f64) -> f64 {
        let c = conversion(a, b, s).unwrap();
        x * c.factor + c.offset
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    #[test]
    fn simple_units() {
        assert!(close(conv("mg", "g", None, 1500.0), 1.5));
        assert!(close(conv("µL", "mL", None, 250.0), 0.25));
        assert!(close(conv("h", "min", None, 2.0), 120.0));
        assert!(close(conv("nM", "µM", None, 1500.0), 1.5));
        assert!(close(conv("mmol", "mol", None, 1.0), 1e-3));
        assert!(close(conv("cm", "m", None, 150.0), 1.5));
        assert!(close(conv("atm", "kPa", None, 1.0), 101.325));
        // 1 mmHg is 133.322387415 Pa, so 760 mmHg is 101.325 kPa to six digits.
        assert!((conv("mmHg", "kPa", None, 760.0) - 101.325).abs() < 1e-4);
        assert!(close(conv("mg/dL", "g/L", None, 100.0), 1.0));
        assert!(close(conv("d", "h", None, 1.0), 24.0));
    }

    #[test]
    fn temperatures() {
        assert!(close(conv("C", "F", None, 100.0), 212.0));
        assert!(close(conv("F", "C", None, 32.0), 0.0));
        assert!(close(conv("C", "K", None, 0.0), 273.15));
        assert!(close(conv("K", "F", None, 0.0), -459.67));
    }

    #[test]
    fn clinical() {
        assert!(close(
            conv("mg/dL", "mmol/L", Some("glucose"), 180.16),
            10.0
        ));
        assert!(close(
            conv("mg/dL", "mmol/L", Some("glucose"), 90.0),
            90.0 / 18.016
        ));
        assert!(close(
            conv("mg/dL", "mmol/L", Some("cholesterol"), 200.0),
            200.0 / 38.67
        ));
        assert!(close(
            conv("mg/dL", "mmol/L", Some("triglycerides"), 150.0),
            150.0 / 88.57
        ));
        assert!(close(
            conv("mg/dL", "µmol/L", Some("creatinine"), 1.0),
            88.42
        ));
        assert!(close(
            conv("mmol/L", "mg/dL", Some("glucose"), 5.0),
            5.0 * 18.016
        ));
        assert!(conversion("mg/dL", "mmol/L", None)
            .unwrap_err()
            .contains("substance"));
        assert!(conversion("g", "s", None).is_err());
    }
}
