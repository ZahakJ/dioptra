//! Dioptra Lab license keys, verified offline.
//!
//! A key is `DIOPTRA-LAB-1.<payload>.<signature>`: the payload is the JSON of a [`License`],
//! the signature an ed25519 signature of [`DOMAIN`] followed by the payload bytes, both in
//! unpadded base64url. Whitespace anywhere in the key is ignored, so a key wrapped by an
//! email client still reads. Nothing is sent anywhere: the app checks the signature against
//! the public key compiled in below and compares the expiry date with the clock.
//!
//! When a license ends, Lab features stay on for [`GRACE_DAYS`] and then pause. Projects,
//! data and everything made with Lab features keep opening; the core never checks a key.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Prefix of every key; the digit is the format version.
pub const PREFIX: &str = "DIOPTRA-LAB-1";

/// Signed before the payload, so a signature cannot be replayed from another protocol.
pub const DOMAIN: &[u8] = b"dioptra-lab-license-v1\0";

/// Days a lapsed license keeps Lab features on.
pub const GRACE_DAYS: i64 = 30;

/// The public half of the signing key that issues Dioptra Lab licenses.
pub const PUBLIC_KEY: [u8; 32] = [
    0xbd, 0x2f, 0x31, 0xc0, 0xc0, 0x78, 0xd7, 0x81, 0xad, 0x70, 0xc2, 0xcf, 0xa5, 0xbf, 0x7f, 0x5b,
    0x2c, 0x19, 0x8b, 0x6f, 0xcf, 0x5c, 0x71, 0x20, 0xf3, 0x59, 0x92, 0x4b, 0x55, 0xce, 0xbc, 0x58,
];

/// Who holds a license, and until when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct License {
    /// Issuer's identifier, quoted in support email.
    pub id: String,
    /// Person or organisation the license is issued to.
    pub licensee: String,
    /// Contact address.
    pub email: String,
    /// What was bought.
    pub plan: Plan,
    /// Number of people who may use it.
    pub seats: u32,
    /// Email domain of a site license (`uni.edu`), shown in the About box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// First day of validity, `YYYY-MM-DD`.
    pub issued: String,
    /// Last day of validity, `YYYY-MM-DD`.
    pub expires: String,
    /// Order number at the store, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

/// The kinds of license sold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    /// One academic or non-profit user.
    Academic,
    /// One user at a company.
    Corporate,
    /// Every user at an institution or a department.
    Site,
    /// Early academic or corporate buyers at the founding price.
    Founding,
    /// A time-limited evaluation, such as a site pilot.
    Trial,
}

impl Plan {
    /// The name shown in the interface.
    pub fn label(self) -> &'static str {
        match self {
            Plan::Academic => "Academic",
            Plan::Corporate => "Corporate",
            Plan::Site => "Site",
            Plan::Founding => "Founding member",
            Plan::Trial => "Trial",
        }
    }
}

/// Why a key was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// Not of the form `DIOPTRA-LAB-1.<payload>.<signature>`.
    Malformed,
    /// The signature does not match: mistyped, altered, or not issued by Dioptra.
    BadSignature,
    /// Signed, but the payload is not a license this version understands.
    BadPayload(String),
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::Malformed => write!(f, "this is not a Dioptra Lab key"),
            KeyError::BadSignature => {
                write!(f, "the key does not check out; paste it again in full")
            }
            KeyError::BadPayload(e) => write!(f, "the key is from a newer Dioptra ({e})"),
        }
    }
}

impl std::error::Error for KeyError {}

/// Where a valid license stands on a given day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Standing {
    /// Before `issued` (a clock set back, or a key issued ahead).
    NotYetValid {
        /// Days until it starts.
        days: i64,
    },
    /// Within its dates.
    Active {
        /// Days left, counting today.
        days_left: i64,
    },
    /// Past `expires` but within the grace period: Lab features still on.
    Grace {
        /// Days of grace left.
        days_left: i64,
    },
    /// Past the grace period: Lab features pause.
    Lapsed {
        /// Days since `expires`.
        days_since: i64,
    },
}

impl Standing {
    /// Whether Lab features are on.
    pub fn lab_enabled(self) -> bool {
        matches!(self, Standing::Active { .. } | Standing::Grace { .. })
    }
}

/// Removes whitespace a mail client or a terminal may have put into a key.
pub fn normalise(key: &str) -> String {
    key.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Checks a key against the built-in public key and returns its license.
pub fn verify(key: &str) -> Result<License, KeyError> {
    verify_with(key, &PUBLIC_KEY)
}

/// Checks a key against the given public key.
pub fn verify_with(key: &str, public_key: &[u8; 32]) -> Result<License, KeyError> {
    let key = normalise(key);
    let mut parts = key.split('.');
    let (Some(prefix), Some(payload), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(KeyError::Malformed);
    };
    if !prefix.eq_ignore_ascii_case(PREFIX) {
        return Err(KeyError::Malformed);
    }
    let payload = B64.decode(payload).map_err(|_| KeyError::Malformed)?;
    let sig = B64.decode(sig).map_err(|_| KeyError::Malformed)?;
    let sig = Signature::from_slice(&sig).map_err(|_| KeyError::Malformed)?;
    let vk = VerifyingKey::from_bytes(public_key).map_err(|_| KeyError::BadSignature)?;
    vk.verify_strict(&signing_message(&payload), &sig)
        .map_err(|_| KeyError::BadSignature)?;
    let license: License =
        serde_json::from_slice(&payload).map_err(|e| KeyError::BadPayload(e.to_string()))?;
    day_number(&license.issued).ok_or_else(|| KeyError::BadPayload("issued date".into()))?;
    day_number(&license.expires).ok_or_else(|| KeyError::BadPayload("expiry date".into()))?;
    Ok(license)
}

/// The bytes the issuer signs for a payload.
pub fn signing_message(payload: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(DOMAIN.len() + payload.len());
    m.extend_from_slice(DOMAIN);
    m.extend_from_slice(payload);
    m
}

/// Assembles a key from a payload and its signature (used by the issuing tool and tests).
pub fn encode(payload: &[u8], signature: &[u8; 64]) -> String {
    format!("{PREFIX}.{}.{}", B64.encode(payload), B64.encode(signature))
}

impl License {
    /// Where this license stands on `today` (`YYYY-MM-DD`).
    pub fn standing(&self, today: &str) -> Standing {
        // Both dates were checked by `verify`; an unparseable `today` counts as lapsed.
        let (Some(t), Some(issued), Some(expires)) = (
            day_number(today),
            day_number(&self.issued),
            day_number(&self.expires),
        ) else {
            return Standing::Lapsed { days_since: 0 };
        };
        if t < issued {
            Standing::NotYetValid { days: issued - t }
        } else if t <= expires {
            Standing::Active {
                days_left: expires - t + 1,
            }
        } else if t <= expires + GRACE_DAYS {
            Standing::Grace {
                days_left: expires + GRACE_DAYS - t + 1,
            }
        } else {
            Standing::Lapsed {
                days_since: t - expires,
            }
        }
    }
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` date (proleptic Gregorian).
pub fn day_number(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = date[0..4].parse().ok()?;
    let m: i64 = date[5..7].parse().ok()?;
    let d: i64 = date[8..10].parse().ok()?;
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_len = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&m) || d < 1 || d > month_len[(m - 1) as usize] {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The civil date of a day number, as `YYYY-MM-DD`.
pub fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today's date from the system clock (UTC). Not available on wasm, where the caller passes
/// the date from JavaScript.
#[cfg(not(target_arch = "wasm32"))]
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_date(secs.div_euclid(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn issuer() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn sample() -> License {
        License {
            id: "L-0001".into(),
            licensee: "Ada Lovelace".into(),
            email: "ada@uni.edu".into(),
            plan: Plan::Academic,
            seats: 1,
            site: None,
            issued: "2026-10-01".into(),
            expires: "2027-09-30".into(),
            order: Some("LS-123".into()),
        }
    }

    fn key_for(l: &License, sk: &SigningKey) -> String {
        let payload = serde_json::to_vec(l).unwrap();
        let sig = sk.sign(&signing_message(&payload));
        encode(&payload, &sig.to_bytes())
    }

    #[test]
    fn round_trip() {
        let sk = issuer();
        let pk = sk.verifying_key().to_bytes();
        let key = key_for(&sample(), &sk);
        assert_eq!(verify_with(&key, &pk).unwrap(), sample());
    }

    #[test]
    fn wrapped_key_still_reads() {
        let sk = issuer();
        let pk = sk.verifying_key().to_bytes();
        let key = key_for(&sample(), &sk);
        let wrapped: String = key
            .as_bytes()
            .chunks(60)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\n  ");
        assert!(verify_with(&format!("  {wrapped}\n"), &pk).is_ok());
    }

    #[test]
    fn tampering_is_caught() {
        let sk = issuer();
        let pk = sk.verifying_key().to_bytes();
        let key = key_for(&sample(), &sk);
        let mut l = sample();
        l.expires = "2099-12-31".into();
        let forged_payload = B64.encode(serde_json::to_vec(&l).unwrap());
        let parts: Vec<&str> = key.split('.').collect();
        let forged = format!("{}.{}.{}", parts[0], forged_payload, parts[2]);
        assert_eq!(verify_with(&forged, &pk), Err(KeyError::BadSignature));
        // A key from another issuer.
        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            verify_with(&key_for(&sample(), &other), &pk),
            Err(KeyError::BadSignature)
        );
        // The real public key does not accept test keys.
        assert_eq!(verify(&key), Err(KeyError::BadSignature));
    }

    #[test]
    fn malformed() {
        assert_eq!(verify("hello"), Err(KeyError::Malformed));
        assert_eq!(verify("DIOPTRA-LAB-1.a.b.c"), Err(KeyError::Malformed));
        assert_eq!(verify("OTHER.YQ.YQ"), Err(KeyError::Malformed));
        assert_eq!(verify("DIOPTRA-LAB-1.!!.YQ"), Err(KeyError::Malformed));
    }

    #[test]
    fn standing_through_the_year() {
        let l = sample();
        assert_eq!(l.standing("2026-09-30"), Standing::NotYetValid { days: 1 });
        assert_eq!(
            l.standing("2026-10-01"),
            Standing::Active { days_left: 365 }
        );
        assert_eq!(l.standing("2027-09-30"), Standing::Active { days_left: 1 });
        assert_eq!(l.standing("2027-10-01"), Standing::Grace { days_left: 30 });
        assert_eq!(l.standing("2027-10-30"), Standing::Grace { days_left: 1 });
        assert_eq!(
            l.standing("2027-10-31"),
            Standing::Lapsed { days_since: 31 }
        );
        assert!(!l.standing("2027-10-31").lab_enabled());
        assert!(l.standing("2027-10-30").lab_enabled());
    }

    #[test]
    fn dates() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2000-03-01"), Some(11_017));
        assert_eq!(day_number("2026-02-29"), None);
        assert_eq!(day_number("2024-02-29"), Some(19_782));
        assert_eq!(day_number("2026-13-01"), None);
        assert_eq!(day_number("26-10-01"), None);
        for d in [-1000, 0, 11_017, 19_782, 20_740, 60_000] {
            assert_eq!(day_number(&civil_date(d)), Some(d));
        }
    }
}
