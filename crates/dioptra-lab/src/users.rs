// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Users and roles for regulated records, kept per computer.
//!
//! The shell stores the users file (JSON) in the app's config directory and hands it to the
//! engine with `lab_users_load`; every change comes back in a `lab_users` reply with
//! `save: true` for the shell to write. Passwords are stored as PBKDF2-HMAC-SHA256 hashes.
//! Users are disabled, never deleted, so an ID is never reused. Every creation, change and
//! cancellation of access is recorded in the file's own hash-chained log, and each log entry
//! carries a hash of the user list after the change, so an edit made outside Dioptra shows.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::audit::{canonical, sha256_hex, GENESIS};

/// PBKDF2 iterations for new password hashes.
pub const ITERATIONS: u32 = 600_000;
const _: () = assert!(ITERATIONS >= 600_000);
/// Failed sign-ins in a row before an account locks.
pub const MAX_FAILURES: u32 = 5;
/// Shortest accepted password.
pub const MIN_PASSWORD: usize = 8;
/// Format identifier of the users file.
pub const FORMAT: &str = "dioptra-lab-users/1";

/// What a user may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Changes data and signs as author.
    Analyst,
    /// Also signs as reviewer or approver.
    Reviewer,
    /// Also manages users.
    Admin,
}

/// One user account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// User ID, typed at sign-in (unique, never reused).
    pub id: String,
    /// Full name, shown in the audit trail and on signatures.
    pub name: String,
    /// Role.
    pub role: Role,
    /// Base64 of the random 16-byte salt.
    pub salt: String,
    /// Base64 of the 32-byte PBKDF2-HMAC-SHA256 output.
    pub hash: String,
    /// PBKDF2 iterations used for `hash`.
    pub iterations: u32,
    /// Access cancelled.
    #[serde(default)]
    pub disabled: bool,
    /// Locked after too many failed sign-ins.
    #[serde(default)]
    pub locked: bool,
    /// Failed sign-ins in a row.
    #[serde(default)]
    pub failures: u32,
    /// When the account was created (UTC).
    pub created: String,
}

/// A user as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserView {
    /// User ID.
    pub id: String,
    /// Full name.
    pub name: String,
    /// Role.
    pub role: Role,
    /// Access cancelled.
    pub disabled: bool,
    /// Locked after failed sign-ins.
    pub locked: bool,
}

/// One line of the users file's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsersLogEntry {
    /// 1, 2, 3, ...
    pub seq: u64,
    /// When (UTC).
    pub at: String,
    /// Who did it (a user ID, or `system` for a lockout).
    pub by: String,
    /// `create`, `disable`, `reset_password` or `lock`.
    pub action: String,
    /// The account concerned.
    pub user: String,
    /// Details in words.
    pub detail: String,
    /// Hash of the user list after this entry.
    pub state: String,
    /// Hash of the previous entry.
    pub prev: String,
    /// SHA-256 of the canonical JSON of this entry without `hash`.
    pub hash: String,
}

/// The users file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsersFile {
    /// [`FORMAT`].
    pub format: String,
    /// Accounts, in creation order.
    pub users: Vec<User>,
    /// Access log, oldest first.
    pub log: Vec<UsersLogEntry>,
}

impl Default for UsersFile {
    fn default() -> Self {
        UsersFile {
            format: FORMAT.into(),
            users: Vec::new(),
            log: Vec::new(),
        }
    }
}

fn b64(bytes: &[u8]) -> String {
    dioptra_core::util::base64_encode(bytes)
}

fn derive(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations, &mut out);
    out
}

/// Hash a new password: `(salt, hash)` in base64.
pub fn hash_password(password: &str) -> Result<(String, String), String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| format!("no random numbers available: {e}"))?;
    Ok((b64(&salt), b64(&derive(password, &salt, ITERATIONS))))
}

/// Whether `password` matches a stored hash (compared in constant time).
pub fn verify_password(user: &User, password: &str) -> bool {
    let (Ok(salt), Ok(want)) = (
        dioptra_core::util::base64_decode(&user.salt),
        dioptra_core::util::base64_decode(&user.hash),
    ) else {
        return false;
    };
    let got = derive(password, &salt, user.iterations);
    want.len() == got.len() && want.iter().zip(got).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn check_password_rules(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD {
        return Err(format!(
            "A password needs at least {MIN_PASSWORD} characters."
        ));
    }
    Ok(())
}

fn same_id(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl User {
    /// The UI's view.
    pub fn view(&self) -> UserView {
        UserView {
            id: self.id.clone(),
            name: self.name.clone(),
            role: self.role,
            disabled: self.disabled,
            locked: self.locked,
        }
    }
}

impl UsersFile {
    /// Read a users file.
    pub fn parse(json: &str) -> Result<Self, String> {
        let f: UsersFile = serde_json::from_str(json)
            .map_err(|e| format!("The users file cannot be read: {e}"))?;
        if f.format != FORMAT {
            return Err(format!(
                "The users file is format {}, which this version does not read.",
                f.format
            ));
        }
        Ok(f)
    }

    /// The file as stored.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("users file serialises")
    }

    /// Hash of the accounts, without the failed-sign-in counters.
    pub fn state_hash(&self) -> String {
        let users: Vec<Json> = self
            .users
            .iter()
            .map(|u| {
                let mut v = serde_json::to_value(u).expect("user");
                if let Some(o) = v.as_object_mut() {
                    o.remove("failures");
                }
                v
            })
            .collect();
        sha256_hex(canonical(&Json::Array(users)).as_bytes())
    }

    /// Check the log chain and that the accounts match its last entry.
    pub fn verify(&self) -> Result<(), String> {
        let mut prev = GENESIS.to_string();
        for (i, e) in self.log.iter().enumerate() {
            let at = format!("The users file's access log is broken at entry {}", i + 1);
            if e.seq != i as u64 + 1 || e.prev != prev {
                return Err(format!("{at}: an entry is missing or out of order."));
            }
            if entry_hash(e) != e.hash {
                return Err(format!("{at}: the entry was changed."));
            }
            prev = e.hash.clone();
        }
        let want = self.log.last().map(|e| e.state.as_str());
        if !self.users.is_empty() && want != Some(self.state_hash().as_str()) {
            return Err(
                "The users file was changed outside Dioptra: its accounts do not match its access log. Restore it from a backup."
                    .into(),
            );
        }
        Ok(())
    }

    /// A user by ID (case-insensitive).
    pub fn find(&self, id: &str) -> Option<&User> {
        self.users.iter().find(|u| same_id(&u.id, id))
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut User> {
        self.users.iter_mut().find(|u| same_id(&u.id, id))
    }

    fn record(&mut self, at: &str, by: &str, action: &str, user: &str, detail: String) {
        let mut e = UsersLogEntry {
            seq: self.log.len() as u64 + 1,
            at: at.into(),
            by: by.into(),
            action: action.into(),
            user: user.into(),
            detail,
            state: self.state_hash(),
            prev: self
                .log
                .last()
                .map_or_else(|| GENESIS.to_string(), |e| e.hash.clone()),
            hash: String::new(),
        };
        e.hash = entry_hash(&e);
        self.log.push(e);
    }

    fn create(
        &mut self,
        at: &str,
        by: &str,
        id: &str,
        name: &str,
        role: Role,
        password: &str,
    ) -> Result<(), String> {
        let id = id.trim();
        let name = name.trim();
        if id.is_empty() || id.chars().any(char::is_whitespace) {
            return Err("A user ID is one word, without spaces.".into());
        }
        if name.is_empty() {
            return Err("Enter the user's full name.".into());
        }
        if self.find(id).is_some() {
            return Err(format!(
                "The user ID {id} exists already. IDs are never reused, also after an account is disabled."
            ));
        }
        check_password_rules(password)?;
        let (salt, hash) = hash_password(password)?;
        self.users.push(User {
            id: id.into(),
            name: name.into(),
            role,
            salt,
            hash,
            iterations: ITERATIONS,
            disabled: false,
            locked: false,
            failures: 0,
            created: at.into(),
        });
        let role_word = serde_json::to_value(role).expect("role");
        self.record(
            at,
            by,
            "create",
            id,
            format!("Created {name} as {}", role_word.as_str().unwrap_or("")),
        );
        Ok(())
    }

    /// Create the first administrator.
    pub fn setup(&mut self, at: &str, id: &str, name: &str, password: &str) -> Result<(), String> {
        if !self.users.is_empty() {
            return Err("Users are set up already on this computer.".into());
        }
        self.create(at, id.trim(), id, name, Role::Admin, password)
    }

    fn admin<'a>(&'a self, by: &str) -> Result<&'a User, String> {
        match self.find(by) {
            Some(u) if u.role == Role::Admin && !u.disabled && !u.locked => Ok(u),
            _ => Err("Only an administrator can manage users.".into()),
        }
    }

    /// Add a user (administrators only).
    pub fn add(
        &mut self,
        at: &str,
        by: &str,
        id: &str,
        name: &str,
        role: Role,
        password: &str,
    ) -> Result<(), String> {
        let by = self.admin(by)?.id.clone();
        self.create(at, &by, id, name, role, password)
    }

    /// Cancel a user's access (administrators only). The account stays in the file.
    pub fn disable(&mut self, at: &str, by: &str, id: &str) -> Result<(), String> {
        let by = self.admin(by)?.id.clone();
        if same_id(&by, id) {
            return Err("An administrator cannot disable their own account.".into());
        }
        let u = self.find_mut(id).ok_or_else(|| format!("No user {id}."))?;
        if u.disabled {
            return Err(format!("{} is disabled already.", u.name));
        }
        u.disabled = true;
        let (uid, name) = (u.id.clone(), u.name.clone());
        self.record(at, &by, "disable", &uid, format!("Disabled {name}"));
        Ok(())
    }

    /// Set a new password and unlock the account (administrators only).
    pub fn reset_password(
        &mut self,
        at: &str,
        by: &str,
        id: &str,
        password: &str,
    ) -> Result<(), String> {
        let by = self.admin(by)?.id.clone();
        check_password_rules(password)?;
        let (salt, hash) = hash_password(password)?;
        let u = self.find_mut(id).ok_or_else(|| format!("No user {id}."))?;
        if u.disabled {
            return Err(format!("{} is disabled.", u.name));
        }
        let unlocked = u.locked;
        u.salt = salt;
        u.hash = hash;
        u.iterations = ITERATIONS;
        u.locked = false;
        u.failures = 0;
        let (uid, name) = (u.id.clone(), u.name.clone());
        let detail = if unlocked {
            format!("Reset the password of {name} and unlocked the account")
        } else {
            format!("Reset the password of {name}")
        };
        self.record(at, &by, "reset_password", &uid, detail);
        Ok(())
    }

    /// Check a user ID and password. Failures count towards the lockout; returns the
    /// user's view, or the message to show.
    pub fn authenticate(&mut self, at: &str, id: &str, password: &str) -> Result<UserView, String> {
        const WRONG: &str = "Unknown user ID or wrong password.";
        let Some(u) = self.find_mut(id) else {
            return Err(WRONG.into());
        };
        if u.disabled {
            return Err("This account is disabled.".into());
        }
        if u.locked {
            return Err(
                "This account is locked after too many failed sign-ins. An administrator can reset its password."
                    .into(),
            );
        }
        if verify_password(u, password) {
            u.failures = 0;
            return Ok(u.view());
        }
        u.failures += 1;
        if u.failures >= MAX_FAILURES {
            u.locked = true;
            let (uid, n) = (u.id.clone(), u.failures);
            self.record(
                at,
                "system",
                "lock",
                &uid,
                format!("Locked after {n} failed sign-ins"),
            );
            return Err(
                "Wrong password. The account is now locked; an administrator can reset its password."
                    .into(),
            );
        }
        let left = MAX_FAILURES - u.failures;
        Err(format!(
            "{WRONG} {left} attempt{} left before the account locks.",
            if left == 1 { "" } else { "s" }
        ))
    }
}

fn entry_hash(e: &UsersLogEntry) -> String {
    let mut v = serde_json::to_value(e).expect("log entry");
    if let Some(o) = v.as_object_mut() {
        o.remove("hash");
    }
    sha256_hex(canonical(&v).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "2026-01-15T10:00:00Z";

    fn admin_file() -> UsersFile {
        let mut f = UsersFile::default();
        f.setup(AT, "ada", "Ada Lovelace", "correct horse").unwrap();
        f
    }

    #[test]
    fn passwords_hash_and_verify() {
        let f = admin_file();
        let u = f.find("ADA").unwrap();
        assert_eq!(u.iterations, ITERATIONS);
        assert_eq!(
            dioptra_core::util::base64_decode(&u.salt).unwrap().len(),
            16
        );
        assert!(verify_password(u, "correct horse"));
        assert!(!verify_password(u, "correct horsE"));
        // Same password, different salt, different hash.
        let (s1, h1) = hash_password("same password").unwrap();
        let (s2, h2) = hash_password("same password").unwrap();
        assert_ne!((s1, h1), (s2, h2));
        assert!(f.verify().is_ok());
    }

    #[test]
    fn lockout_after_five_failures() {
        let mut f = admin_file();
        for i in 1..MAX_FAILURES {
            let e = f.authenticate(AT, "ada", "nope").unwrap_err();
            assert!(e.contains("left"), "{i}: {e}");
        }
        assert!(f
            .authenticate(AT, "ada", "nope")
            .unwrap_err()
            .contains("locked"));
        // The right password no longer helps.
        assert!(f
            .authenticate(AT, "ada", "correct horse")
            .unwrap_err()
            .contains("locked"));
        assert_eq!(f.log.last().unwrap().action, "lock");
        assert!(f.verify().is_ok());
    }

    #[test]
    fn only_admins_manage_users() {
        let mut f = admin_file();
        f.add(AT, "ada", "bob", "Bob Analyst", Role::Analyst, "password1")
            .unwrap();
        assert!(f
            .add(AT, "bob", "eve", "Eve", Role::Admin, "password1")
            .unwrap_err()
            .contains("administrator"));
        assert!(f.disable(AT, "bob", "ada").is_err());
        assert!(f.reset_password(AT, "bob", "ada", "password2").is_err());
        assert!(f
            .add(AT, "ada", "BOB", "Again", Role::Analyst, "password1")
            .is_err());
        assert!(f
            .add(AT, "ada", "carl", "Carl", Role::Analyst, "short")
            .is_err());
        f.disable(AT, "ada", "bob").unwrap();
        assert!(f
            .authenticate(AT, "bob", "password1")
            .unwrap_err()
            .contains("disabled"));
        assert!(f.find("bob").is_some(), "disabled, not deleted");
        assert_eq!(
            f.log.iter().map(|e| e.action.as_str()).collect::<Vec<_>>(),
            ["create", "create", "disable"]
        );
        assert!(f.verify().is_ok());
    }

    #[test]
    fn reset_unlocks_and_edits_outside_show() {
        let mut f = admin_file();
        f.add(AT, "ada", "bob", "Bob", Role::Reviewer, "password1")
            .unwrap();
        for _ in 0..MAX_FAILURES {
            let _ = f.authenticate(AT, "bob", "x");
        }
        f.reset_password(AT, "ada", "bob", "password2").unwrap();
        assert_eq!(f.authenticate(AT, "bob", "password2").unwrap().id, "bob");
        let mut g = UsersFile::parse(&f.to_json()).unwrap();
        g.verify().unwrap();
        g.users[1].role = Role::Admin;
        assert!(g.verify().unwrap_err().contains("outside Dioptra"));
        let mut h = f.clone();
        h.log.remove(1);
        assert!(h.verify().unwrap_err().contains("broken"));
    }
}
