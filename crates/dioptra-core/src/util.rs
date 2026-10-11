//! Small shared helpers: base64, timestamps, number display, serde adapters.

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Base64 (standard alphabet, padded). Small enough not to justify a dependency.
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes as standard padded base64.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Decode standard or URL-safe base64; whitespace is ignored, padding optional.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &c in text.as_bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = val(c).ok_or_else(|| format!("invalid base64 character '{}'", c as char))?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

thread_local! {
    static CLOCK: RefCell<Option<Box<dyn Fn() -> String>>> = const { RefCell::new(None) };
}

/// Install a clock for this thread (used by the wasm facade to inject `Date.now()`, and by
/// tests for deterministic timestamps). `None` restores the default.
pub fn set_clock(clock: Option<Box<dyn Fn() -> String>>) {
    CLOCK.with(|c| *c.borrow_mut() = clock);
}

/// Current time as an ISO 8601 UTC string. On wasm, without an installed clock, this is
/// the Unix epoch, because `SystemTime::now()` is unavailable there.
pub fn now() -> String {
    if let Some(s) = CLOCK.with(|c| c.borrow().as_ref().map(|f| f())) {
        return s;
    }
    default_now()
}

#[cfg(not(target_arch = "wasm32"))]
fn default_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_from_unix(secs)
}

#[cfg(target_arch = "wasm32")]
fn default_now() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

/// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day). Hinnant's algorithm.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Number display
// ---------------------------------------------------------------------------

/// Format a number for display. `Some(d)` gives exactly `d` decimals; `None` gives the
/// shortest representation with at most six significant digits, switching to scientific
/// notation for very large or very small magnitudes. NaN formats as the empty string.
pub fn format_number(v: f64, decimals: Option<u8>) -> String {
    format_sig(v, decimals, 6)
}

/// Like [`format_number`] with a chosen number of significant digits for `None`.
pub fn format_sig(v: f64, decimals: Option<u8>, sig: usize) -> String {
    if v.is_nan() {
        return String::new();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if let Some(d) = decimals {
        let s = format!("{:.*}", d as usize, v);
        return strip_negative_zero(s);
    }
    if v == 0.0 {
        return "0".into();
    }
    let sig = sig.max(1);
    let rounded: f64 = format!("{:.*e}", sig - 1, v).parse().unwrap_or(v);
    let a = rounded.abs();
    if (1e-4..1e15).contains(&a) {
        // Shortest round-trip of the rounded value never shows more than `sig` digits.
        let s = format!("{rounded}");
        strip_negative_zero(s)
    } else {
        // Scientific: mantissa shortest form.
        let s = format!("{rounded:e}");
        s.replace("e", "E")
    }
}

fn strip_negative_zero(s: String) -> String {
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

/// Column letters like a spreadsheet: 0 → A, 25 → Z, 26 → AA.
pub fn letters(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).expect("ascii")
}

/// Inverse of [`letters`]: "A" → 0, "AA" → 26. `None` unless all uppercase ASCII letters.
pub fn letters_index(s: &str) -> Option<usize> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_uppercase()) || s.len() > 6 {
        return None;
    }
    let mut n = 0usize;
    for b in s.bytes() {
        n = n * 26 + (b - b'A' + 1) as usize;
    }
    Some(n - 1)
}

// ---------------------------------------------------------------------------
// Serde adapters
// ---------------------------------------------------------------------------

/// Serialise `Vec<f64>` with NaN as `null` (the file format's empty cell).
pub mod nan_vec {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serialise.
    pub fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for x in v {
            if x.is_finite() {
                seq.serialize_element(x)?;
            } else {
                seq.serialize_element(&Option::<f64>::None)?;
            }
        }
        seq.end()
    }

    /// Deserialise.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
        let v: Vec<Option<f64>> = Vec::deserialize(d)?;
        Ok(v.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect())
    }
}

/// Serialise a single `f64` with NaN/infinite as `null`.
pub mod nan_f64 {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serialise.
    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else {
            s.serialize_none()
        }
    }

    /// Deserialise.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
    }
}

/// Serialise `Option<f64>`-like values where NaN is `null`, inside `Vec<Vec<f64>>`.
pub fn nan_eq(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            let e = base64_encode(s.as_bytes());
            assert_eq!(base64_decode(&e).unwrap(), s.as_bytes());
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn number_display() {
        assert_eq!(format_number(1.0, None), "1");
        assert_eq!(format_number(0.1 + 0.2, None), "0.3");
        assert_eq!(format_number(123456.789, None), "123457");
        assert_eq!(format_number(1.234567891, None), "1.23457");
        assert_eq!(format_number(1.5e20, None), "1.5E20");
        assert_eq!(format_number(1.234567e-7, None), "1.23457E-7");
        assert_eq!(format_number(2.5, Some(2)), "2.50");
        assert_eq!(format_number(-0.0001, Some(2)), "0.00");
        assert_eq!(format_number(f64::NAN, None), "");
    }

    #[test]
    fn letters_round_trip() {
        for i in [0, 1, 25, 26, 27, 51, 52, 701, 702, 5000] {
            assert_eq!(letters_index(&letters(i)), Some(i));
        }
        assert_eq!(letters(0), "A");
        assert_eq!(letters(26), "AA");
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(1_760_173_200), "2025-10-11T09:00:00Z");
    }
}
