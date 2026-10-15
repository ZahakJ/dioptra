//! Dioptra Lab license state for a session. The key itself is stored by the shell (a file
//! in the app's config directory, or browser storage); the engine checks it and answers
//! whether Lab features are on. See docs/LAB-TIER.md.

use dioptra_license::{License, Standing};
use serde::{Deserialize, Serialize};

/// What the UI shows about the license.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LicenseView {
    /// Whether Lab features are on.
    pub lab_enabled: bool,
    /// The license, if a valid key is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    /// Plan name for display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_label: Option<String>,
    /// Where the license stands today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standing: Option<serde_json::Value>,
    /// Why the last key was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The license held by a session.
#[derive(Debug, Clone, Default)]
pub struct LabState {
    license: Option<License>,
    standing: Option<Standing>,
}

impl LabState {
    /// Set or clear the key. A refused key leaves the previous license in place.
    pub fn set_key(&mut self, key: Option<&str>, today: &str) -> LicenseView {
        match key.map(str::trim).filter(|k| !k.is_empty()) {
            None => {
                *self = LabState::default();
                self.view(today)
            }
            Some(k) => match dioptra_license::verify(k) {
                Ok(l) => {
                    self.license = Some(l);
                    self.view(today)
                }
                Err(e) => LicenseView {
                    error: Some(e.to_string()),
                    ..self.view(today)
                },
            },
        }
    }

    /// The current view, with the standing computed for `today`.
    pub fn view(&mut self, today: &str) -> LicenseView {
        self.standing = self.license.as_ref().map(|l| l.standing(today));
        LicenseView {
            lab_enabled: self.enabled(),
            plan_label: self.license.as_ref().map(|l| l.plan.label().to_string()),
            standing: self
                .standing
                .map(|s| serde_json::to_value(s).expect("standing")),
            license: self.license.clone(),
            error: None,
        }
    }

    /// Whether Lab features are on, as of the last `view` or `set_key`.
    pub fn enabled(&self) -> bool {
        self.standing.is_some_and(Standing::lab_enabled)
    }

    /// An error for a Lab command run without a license.
    pub fn require(&self, feature: &str) -> Result<(), String> {
        if self.enabled() {
            Ok(())
        } else {
            Err(format!(
                "{feature} is part of Dioptra Lab. Add a license key in Help → Dioptra Lab."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{Command, Response, Session};

    fn license(s: &mut Session, key: Option<&str>) -> super::LicenseView {
        match s.dispatch(Command::SetLicense {
            key: key.map(Into::into),
            today: "2026-10-14".into(),
        }) {
            Response::License(v) => v,
            r => panic!("unexpected {r:?}"),
        }
    }

    #[test]
    fn refused_and_removed_keys_leave_lab_off() {
        let mut s = Session::new();
        let v = license(&mut s, Some("DIOPTRA-LAB-1.e30.AAAA"));
        assert!(!v.lab_enabled);
        assert!(v.error.is_some());
        assert!(s.lab.require("Batch analysis").is_err());
        let v = license(&mut s, None);
        assert!(!v.lab_enabled && v.error.is_none() && v.license.is_none());
        let v = license(&mut s, Some(dioptra_license::TEST_KEY));
        assert!(v.license.is_some() && v.error.is_none());
        assert!(!v.lab_enabled, "the test key ended in January 2026");
        match s.dispatch(Command::GetLicense {
            today: "2026-01-15".into(),
        }) {
            Response::License(v) => assert!(v.lab_enabled),
            r => panic!("unexpected {r:?}"),
        }
        assert!(s.lab.require("Batch analysis").is_ok());
        // The command is not an edit: nothing enters the undo history.
        assert!(s.history.log.is_empty());
    }
}
