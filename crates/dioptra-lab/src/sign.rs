// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Records for regulated labs: the `lab_*` commands for users, regulated projects,
//! electronic signatures, verification and the audit report.
//!
//! A signature records the signer's name and ID, the time, the meaning (authored, reviewed,
//! approved), the SHA-256 of the project content and the hash of the trail head, as an entry
//! of the audit trail. The password is asked again for every signature. A signed project is
//! locked until `lab_new_version`; earlier signatures then show as superseded.
//!
//! Free without a license: verifying (`lab_verify_records`, `lab_audit_trail`,
//! `lab_audit_report`), signing in and out, and every lock and check on a regulated project.

use dioptra_core::api::Session;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

use crate::audit::{self, AuditEntry, Break, EntryKind, SignatureData};
use crate::users::{Role, UserView, UsersFile, UsersLogEntry};

const MEMO_USERS: &str = "lab.users";
const MEMO_USER: &str = "lab.user";
const MEMO_DIRTY: &str = "lab.users_dirty";

/// What a signature means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Meaning {
    /// The signer wrote the record.
    Authored,
    /// The signer reviewed it.
    Reviewed,
    /// The signer approved it.
    Approved,
}

impl Meaning {
    /// "Authored", "Reviewed", "Approved".
    pub fn label(self) -> &'static str {
        match self {
            Meaning::Authored => "Authored",
            Meaning::Reviewed => "Reviewed",
            Meaning::Approved => "Approved",
        }
    }
}

/// A Records command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordsCommand {
    /// Hand the engine this computer's users file (absent: none yet). Replies `lab_users`.
    LabUsersLoad {
        /// The file's text.
        #[serde(default)]
        file: Option<String>,
    },
    /// The users, the signed-in user and the file to store if it changed.
    LabUsersList,
    /// Create the first administrator (needs a license) and sign them in.
    LabUsersSetup {
        /// User ID.
        id: String,
        /// Full name.
        name: String,
        /// Password.
        password: String,
    },
    /// Add a user (administrators; needs a license).
    LabUserAdd {
        /// User ID.
        id: String,
        /// Full name.
        name: String,
        /// Role.
        role: Role,
        /// Initial password.
        password: String,
    },
    /// Cancel a user's access (administrators; needs a license).
    LabUserDisable {
        /// User ID.
        id: String,
    },
    /// Set a new password and unlock (administrators; needs a license).
    LabUserResetPassword {
        /// User ID.
        id: String,
        /// New password.
        password: String,
    },
    /// Sign in with user ID and password.
    LabLogin {
        /// User ID.
        user: String,
        /// Password.
        password: String,
    },
    /// Sign out.
    LabLogout,
    /// Turn on the audit trail for the open project; it cannot be turned off.
    LabMakeRegulated {
        /// Why.
        #[serde(default)]
        reason: String,
    },
    /// Sign the open project; the password is asked again.
    LabSign {
        /// Meaning.
        meaning: Meaning,
        /// The signer's password.
        password: String,
    },
    /// Start a new version of a signed project, which unlocks it.
    LabNewVersion {
        /// Why.
        reason: String,
    },
    /// Check the trail and the signatures (free). Replies `lab_records`.
    LabVerifyRecords,
    /// The trail's entries and status (free).
    LabAuditTrail,
    /// The audit report as `csv` or `pdf` (free). Replies `lab_file`; with `path`
    /// (desktop), the file is written there and `base64` is empty.
    LabAuditReport {
        /// `csv` or `pdf`.
        format: String,
        /// Where to write it.
        #[serde(default)]
        path: Option<String>,
    },
}

/// Whether a signature still holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    /// The content is what was signed.
    Valid,
    /// A new version was started after it.
    Superseded,
    /// The content differs from what was signed.
    ContentChanged,
}

/// A signature as the UI and the report show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureView {
    /// Trail entry.
    pub seq: u64,
    /// When (UTC).
    pub at: String,
    /// Signer's user ID.
    pub user: String,
    /// Signer's full name.
    pub name: String,
    /// Meaning.
    pub meaning: Meaning,
    /// Record version signed.
    pub version: u32,
    /// Content hash signed.
    pub content_hash: String,
    /// Status today.
    pub status: SignatureStatus,
}

/// The state of the open project's records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordsStatus {
    /// The audit trail is on.
    pub regulated: bool,
    /// Every entry is present, unchanged and in order.
    pub intact: bool,
    /// Entries.
    pub entries: usize,
    /// Record version.
    pub version: u32,
    /// Signed: changes are refused until a new version.
    pub locked: bool,
    /// The project content is what the last entry recorded.
    pub content_matches: bool,
    /// First failure of the chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_break: Option<Break>,
    /// Signatures, oldest first.
    pub signatures: Vec<SignatureView>,
    /// Signed-in user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<UserView>,
    /// Whether this computer has users set up.
    pub has_users: bool,
}

impl RecordsStatus {
    /// The message to show loudly, if the records cannot be trusted.
    pub fn warning(&self) -> Option<String> {
        if !self.regulated {
            return None;
        }
        if let Some(b) = &self.first_break {
            return Some(format!(
                "The audit trail of this project is broken at entry {}: {}. The record cannot be trusted.",
                b.line, b.problem
            ));
        }
        if !self.content_matches {
            return Some(
                "This project was changed outside its audit trail. The record cannot be trusted."
                    .into(),
            );
        }
        None
    }
}

/// A Records reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordsResponse {
    /// Users on this computer.
    LabUsers {
        /// Accounts.
        users: Vec<UserView>,
        /// Signed-in user.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current: Option<UserView>,
        /// Whether a users file has been loaded.
        loaded: bool,
        /// The users file to store.
        file: String,
        /// Whether `file` changed and must be stored.
        save: bool,
        /// The access log, oldest first.
        log: Vec<UsersLogEntry>,
    },
    /// Records state.
    LabRecords(RecordsStatus),
    /// The trail.
    LabAuditTrail {
        /// State.
        status: RecordsStatus,
        /// Entries, oldest first.
        entries: Vec<AuditEntry>,
    },
    /// A file to save (the same shape as Lab's other files).
    LabFile {
        /// Suggested file name.
        name: String,
        /// MIME type.
        mime: String,
        /// Base64 content.
        base64: String,
        /// Notes.
        notes: Vec<String>,
    },
}

/// Whether `kind` is one of the commands of this module.
pub fn is_records_command(kind: &str) -> bool {
    matches!(
        kind,
        "lab_users_load"
            | "lab_users_list"
            | "lab_users_setup"
            | "lab_user_add"
            | "lab_user_disable"
            | "lab_user_reset_password"
            | "lab_login"
            | "lab_logout"
            | "lab_make_regulated"
            | "lab_sign"
            | "lab_new_version"
            | "lab_verify_records"
            | "lab_audit_trail"
            | "lab_audit_report"
    )
}

/// JSON in, JSON out for the commands of this module.
pub fn dispatch_json(session: &mut Session, json: &str) -> String {
    let reply = match serde_json::from_str::<RecordsCommand>(json) {
        Ok(cmd) => dispatch(session, cmd),
        Err(e) => Err(format!("bad command: {e}")),
    };
    match reply {
        Ok(v) => serde_json::to_string(&v).expect("reply serialises"),
        Err(message) => serde_json::to_string(&json!({ "type": "error", "message": message }))
            .expect("error serialises"),
    }
}

// ---- users in the session ---------------------------------------------------

fn users(session: &Session) -> Option<UsersFile> {
    serde_json::from_value(session.memo(MEMO_USERS)?.clone()).ok()
}

fn put_users(session: &mut Session, f: &UsersFile) {
    session.set_memo(MEMO_USERS, Some(serde_json::to_value(f).expect("users")));
    session.set_memo(MEMO_DIRTY, Some(Json::Bool(true)));
}

/// The signed-in user, if their account is still active.
pub fn current_user(session: &Session) -> Option<UserView> {
    let id = session.memo(MEMO_USER)?.as_str()?;
    let u = users(session)?.find(id)?.view();
    (!u.disabled && !u.locked).then_some(u)
}

fn require_user(session: &Session) -> Result<UserView, String> {
    current_user(session).ok_or_else(|| "Sign in first (Records → Sign in…).".to_string())
}

/// Ask the signed-in user's password again, for a signature outside a project (the OQ
/// report). A wrong password counts towards the lockout, as for any signature.
pub fn confirm_signer(session: &mut Session, password: &str) -> Result<UserView, String> {
    let user = require_user(session)?;
    let mut f = users(session).ok_or("No users file is loaded.")?;
    let r = f.authenticate(&dioptra_core::util::now(), &user.id, password);
    put_users(session, &f);
    r
}

fn users_reply(session: &mut Session) -> RecordsResponse {
    let f = users(session);
    let save = session
        .memo(MEMO_DIRTY)
        .and_then(Json::as_bool)
        .unwrap_or(false);
    session.set_memo(MEMO_DIRTY, None);
    let current = current_user(session);
    let loaded = f.is_some();
    let f = f.unwrap_or_default();
    RecordsResponse::LabUsers {
        users: f.users.iter().map(|u| u.view()).collect(),
        current,
        loaded,
        file: f.to_json(),
        save,
        log: f.log,
    }
}

// ---- content and status -------------------------------------------------------

/// SHA-256 of the canonical JSON of the project content: title, p-value style, tables,
/// analyses (specs), graphs and layouts, each ordered by id. Formula and derived values
/// are brought up to date first, as saving does.
pub fn content_hash(session: &mut Session) -> String {
    session.refresh_tables();
    let p = session.project();
    fn by_id<T: Serialize>(items: impl Iterator<Item = (String, T)>) -> Json {
        let mut v: Vec<(String, T)> = items.collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        Json::Array(
            v.into_iter()
                .map(|(_, x)| serde_json::to_value(x).unwrap_or(Json::Null))
                .collect(),
        )
    }
    let content = json!({
        "title": p.title,
        "p_value_style": p.p_value_style,
        "tables": by_id(p.tables.iter().map(|(k, v)| (k.clone(), v))),
        "analyses": by_id(p.analyses.iter().map(|(k, v)| (k.clone(), v))),
        "graphs": by_id(p.graphs.iter().map(|(k, v)| (k.clone(), v))),
        "layouts": by_id(p.layouts.iter().map(|(k, v)| (k.clone(), v))),
    });
    audit::sha256_hex(audit::canonical(&content).as_bytes())
}

/// The records state of the open project.
pub fn status(session: &mut Session) -> RecordsStatus {
    let user = current_user(session);
    let has_users = users(session).is_some_and(|f| !f.users.is_empty());
    let trail = audit::read_trail(session);
    let regulated = audit::is_regulated(session);
    let content = if regulated {
        content_hash(session)
    } else {
        String::new()
    };
    let signatures = signatures(&trail.entries, &content);
    RecordsStatus {
        regulated,
        intact: trail.first_break.is_none(),
        entries: trail.entries.len(),
        version: audit::version_of(&trail.entries),
        locked: audit::locked(&trail.entries),
        content_matches: !regulated || trail.entries.last().is_some_and(|e| e.content == content),
        first_break: trail.first_break,
        signatures,
        user,
        has_users,
    }
}

fn signatures(entries: &[AuditEntry], content: &str) -> Vec<SignatureView> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let s = e.signature.as_ref()?;
            let superseded = entries[i + 1..]
                .iter()
                .any(|l| l.kind == EntryKind::NewVersion);
            Some(SignatureView {
                seq: e.seq,
                at: e.at.clone(),
                user: e.user.clone(),
                name: e.name.clone(),
                meaning: s.meaning,
                version: e.version,
                content_hash: s.content_hash.clone(),
                status: if superseded {
                    SignatureStatus::Superseded
                } else if s.content_hash == content {
                    SignatureStatus::Valid
                } else {
                    SignatureStatus::ContentChanged
                },
            })
        })
        .collect()
}

// ---- commands ------------------------------------------------------------------

const FEATURE: &str = "Records for regulated labs";

/// Run one Records command.
pub fn dispatch(session: &mut Session, cmd: RecordsCommand) -> Result<RecordsResponse, String> {
    let now = dioptra_core::util::now();
    match cmd {
        RecordsCommand::LabUsersLoad { file } => {
            let f = match file.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                None => UsersFile::default(),
                Some(text) => {
                    let f = UsersFile::parse(text)?;
                    if let Err(e) = f.verify() {
                        session.set_memo(MEMO_USERS, None);
                        session.set_memo(MEMO_USER, None);
                        return Err(e);
                    }
                    f
                }
            };
            session.set_memo(MEMO_USERS, Some(serde_json::to_value(&f).expect("users")));
            session.set_memo(MEMO_DIRTY, None);
            Ok(users_reply(session))
        }
        RecordsCommand::LabUsersList => Ok(users_reply(session)),
        RecordsCommand::LabUsersSetup { id, name, password } => {
            session.require_lab(FEATURE)?;
            let mut f = users(session).unwrap_or_default();
            f.setup(&now, &id, &name, &password)?;
            put_users(session, &f);
            let uid = f.find(&id).map(|u| u.id.clone()).unwrap_or(id);
            session.set_memo(MEMO_USER, Some(Json::String(uid)));
            Ok(users_reply(session))
        }
        RecordsCommand::LabUserAdd {
            id,
            name,
            role,
            password,
        } => {
            session.require_lab(FEATURE)?;
            let by = require_user(session)?;
            let mut f = users(session).ok_or("No users file is loaded.")?;
            f.add(&now, &by.id, &id, &name, role, &password)?;
            put_users(session, &f);
            Ok(users_reply(session))
        }
        RecordsCommand::LabUserDisable { id } => {
            session.require_lab(FEATURE)?;
            let by = require_user(session)?;
            let mut f = users(session).ok_or("No users file is loaded.")?;
            f.disable(&now, &by.id, &id)?;
            put_users(session, &f);
            Ok(users_reply(session))
        }
        RecordsCommand::LabUserResetPassword { id, password } => {
            session.require_lab(FEATURE)?;
            let by = require_user(session)?;
            let mut f = users(session).ok_or("No users file is loaded.")?;
            f.reset_password(&now, &by.id, &id, &password)?;
            put_users(session, &f);
            Ok(users_reply(session))
        }
        RecordsCommand::LabLogin { user, password } => {
            let mut f = users(session)
                .filter(|f| !f.users.is_empty())
                .ok_or("No users are set up on this computer (Records → Set up users…).")?;
            let r = f.authenticate(&now, &user, &password);
            put_users(session, &f);
            let u = r?;
            session.set_memo(MEMO_USER, Some(Json::String(u.id)));
            Ok(users_reply(session))
        }
        RecordsCommand::LabLogout => {
            session.set_memo(MEMO_USER, None);
            Ok(users_reply(session))
        }
        RecordsCommand::LabMakeRegulated { reason } => {
            session.require_lab(FEATURE)?;
            let user = require_user(session)?;
            if audit::is_regulated(session) {
                return Err("This project is regulated already.".into());
            }
            let reason = match reason.trim() {
                "" => "Records for regulated work start".to_string(),
                r => r.to_string(),
            };
            let e = audit::entry(
                &user,
                EntryKind::Regulated,
                "Turned on the audit trail".into(),
                reason,
            );
            audit::append(session, e, Some(1));
            Ok(RecordsResponse::LabRecords(status(session)))
        }
        RecordsCommand::LabSign { meaning, password } => {
            session.require_lab(FEATURE)?;
            let user = require_user(session)?;
            if !audit::is_regulated(session) {
                return Err("Only a regulated project can be signed (Records → Make this project regulated…).".into());
            }
            let st = status(session);
            if let Some(w) = st.warning() {
                return Err(format!("{w} It cannot be signed."));
            }
            if meaning != Meaning::Authored && user.role == Role::Analyst {
                return Err(format!(
                    "Signing as {} needs the reviewer or administrator role.",
                    meaning.label().to_lowercase()
                ));
            }
            if st.signatures.iter().any(|s| {
                s.status == SignatureStatus::Valid && s.user == user.id && s.meaning == meaning
            }) {
                return Err(format!(
                    "{} has signed this version as {} already.",
                    user.name,
                    meaning.label().to_lowercase()
                ));
            }
            let mut f = users(session).ok_or("No users file is loaded.")?;
            let r = f.authenticate(&now, &user.id, &password);
            put_users(session, &f);
            r?;
            let trail = audit::read_trail(session);
            let head = trail
                .entries
                .last()
                .map_or_else(|| audit::GENESIS.to_string(), |e| e.hash.clone());
            let content = content_hash(session);
            let mut e = audit::entry(
                &user,
                EntryKind::Signature,
                format!("Signed: {}", meaning.label()),
                String::new(),
            );
            e.signature = Some(SignatureData {
                meaning,
                content_hash: content,
                audit_head: head,
            });
            audit::append(session, e, None);
            Ok(RecordsResponse::LabRecords(status(session)))
        }
        RecordsCommand::LabNewVersion { reason } => {
            session.require_lab(FEATURE)?;
            let user = require_user(session)?;
            let trail = audit::read_trail(session);
            if !audit::locked(&trail.entries) {
                return Err("This project is not signed; it can be changed as it is.".into());
            }
            let reason = reason.trim().to_string();
            if reason.is_empty() {
                return Err(audit::REASON_REQUIRED.into());
            }
            let v = audit::version_of(&trail.entries) + 1;
            let e = audit::entry(
                &user,
                EntryKind::NewVersion,
                format!("Started version {v}"),
                reason,
            );
            audit::append(session, e, Some(v));
            Ok(RecordsResponse::LabRecords(status(session)))
        }
        RecordsCommand::LabVerifyRecords => Ok(RecordsResponse::LabRecords(status(session))),
        RecordsCommand::LabAuditTrail => {
            let entries = audit::read_trail(session).entries;
            Ok(RecordsResponse::LabAuditTrail {
                status: status(session),
                entries,
            })
        }
        RecordsCommand::LabAuditReport { format, path } => {
            let r = report(session, &format)?;
            match (path, r) {
                (Some(path), RecordsResponse::LabFile { mime, base64, .. }) => {
                    let bytes = dioptra_core::util::base64_decode(&base64)?;
                    dioptra_core::file::write_path(&path, &bytes)?;
                    Ok(RecordsResponse::LabFile {
                        name: path,
                        mime,
                        base64: String::new(),
                        notes: Vec::new(),
                    })
                }
                (_, r) => Ok(r),
            }
        }
    }
}

// ---- the audit report --------------------------------------------------------------

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn objects_text(session: &Session, ids: &[String]) -> String {
    ids.iter()
        .map(|id| {
            session
                .project()
                .title_of(id)
                .map_or_else(|| id.chars().take(8).collect(), str::to_string)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn meaning_of(e: &AuditEntry) -> &'static str {
    e.signature.as_ref().map_or("", |s| s.meaning.label())
}

fn report(session: &mut Session, format: &str) -> Result<RecordsResponse, String> {
    if !audit::is_regulated(session) {
        return Err("This project has no audit trail.".into());
    }
    let st = status(session);
    let entries = audit::read_trail(session).entries;
    let stem = match session.project().title.trim() {
        "" => "Untitled".to_string(),
        t => t.to_string(),
    };
    let (bytes, mime, ext) = match format {
        "csv" => {
            let mut out = String::from(
                "seq,at_utc,user_id,full_name,kind,action,objects,reason,version,signature_meaning,content_sha256,prev,hash\n",
            );
            for e in &entries {
                let kind = serde_json::to_value(e.kind).expect("kind");
                let row = [
                    e.seq.to_string(),
                    e.at.clone(),
                    e.user.clone(),
                    e.name.clone(),
                    kind.as_str().unwrap_or("").to_string(),
                    e.action.clone(),
                    objects_text(session, &e.objects),
                    e.reason.clone(),
                    e.version.to_string(),
                    meaning_of(e).to_string(),
                    e.content.clone(),
                    e.prev.clone(),
                    e.hash.clone(),
                ];
                out.push_str(
                    &row.iter()
                        .map(|f| csv_field(f))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.push('\n');
            }
            (out.into_bytes(), "text/csv", "csv")
        }
        "pdf" => {
            let pages = pdf_pages(session, &st, &entries);
            let pdf = dioptra_plot::output::svgs_to_pdf(&pages).map_err(|e| e.to_string())?;
            (pdf, "application/pdf", "pdf")
        }
        f => return Err(format!("Unknown report format {f}; use csv or pdf.")),
    };
    Ok(RecordsResponse::LabFile {
        name: format!("{stem} audit trail.{ext}"),
        mime: mime.into(),
        base64: dioptra_core::util::base64_encode(&bytes),
        notes: Vec::new(),
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Split text into lines of at most `width` characters, at spaces where possible.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let mut word = word.to_string();
            while word.chars().count() > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let head: String = word.chars().take(width).collect();
                word = word.chars().skip(width).collect();
                lines.push(head);
            }
            let need = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
            if need > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word);
        }
        lines.push(line);
    }
    lines
}

const PAGE_W: f64 = 842.0;
const PAGE_H: f64 = 595.0;
const MARGIN: f64 = 36.0;
const FONT: f64 = 7.5;
const LINE: f64 = 9.5;

struct Page {
    body: String,
    y: f64,
}

impl Page {
    fn new() -> Self {
        Page {
            body: String::new(),
            y: MARGIN + 10.0,
        }
    }

    fn text(&mut self, x: f64, y: f64, size: f64, weight: u16, s: &str) {
        self.body.push_str(&format!(
            "<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"Inter\" font-size=\"{size}\" font-weight=\"{weight}\" fill=\"#1a1a1a\">{}</text>",
            esc(s)
        ));
    }

    fn line(&mut self, y: f64) {
        self.body.push_str(&format!(
            "<line x1=\"{MARGIN}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#c8c8c8\" stroke-width=\"0.5\"/>",
            PAGE_W - MARGIN
        ));
    }

    fn svg(&self, footer: &str) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"297mm\" height=\"210mm\" viewBox=\"0 0 {PAGE_W} {PAGE_H}\"><rect width=\"{PAGE_W}\" height=\"{PAGE_H}\" fill=\"#ffffff\"/>{}<text x=\"{MARGIN}\" y=\"{:.1}\" font-family=\"Inter\" font-size=\"7\" fill=\"#666666\">{}</text></svg>",
            self.body,
            PAGE_H - 18.0,
            esc(footer)
        )
    }
}

/// Columns of the entry table: title, x, width in characters.
const COLS: [(&str, f64, usize); 7] = [
    ("#", MARGIN, 5),
    ("Time (UTC)", MARGIN + 22.0, 22),
    ("User", MARGIN + 112.0, 24),
    ("Action", MARGIN + 210.0, 44),
    ("Objects", MARGIN + 385.0, 26),
    ("Reason", MARGIN + 490.0, 40),
    ("Hash", MARGIN + 650.0, 30),
];

fn pdf_pages(session: &Session, st: &RecordsStatus, entries: &[AuditEntry]) -> Vec<String> {
    let title = session.project().title.clone();
    let mut pages: Vec<Page> = Vec::new();
    let mut p = Page::new();
    p.text(MARGIN, p.y, 14.0, 600, &format!("Audit trail: {title}"));
    p.y += 18.0;
    let who = st
        .user
        .as_ref()
        .map_or("nobody signed in".to_string(), |u| {
            format!("{} ({})", u.name, u.id)
        });
    p.text(
        MARGIN,
        p.y,
        FONT,
        400,
        &format!(
            "Exported {} by {who}. Version {}.",
            dioptra_core::util::now(),
            st.version
        ),
    );
    p.y += LINE;
    let chain = match (&st.first_break, st.content_matches) {
        (Some(b), _) => format!(
            "Chain BROKEN at entry {}: {}. {} entries.",
            b.line, b.problem, st.entries
        ),
        (None, false) => format!(
            "Chain intact, {} entries. The project content DIFFERS from the last entry: it was changed outside the trail.",
            st.entries
        ),
        (None, true) => format!(
            "Chain intact, {} entries. The project content matches the last entry.",
            st.entries
        ),
    };
    p.text(MARGIN, p.y, FONT, 600, &chain);
    p.y += LINE * 1.5;
    if !st.signatures.is_empty() {
        p.text(MARGIN, p.y, 9.0, 600, "Signatures");
        p.y += LINE + 2.0;
        for s in &st.signatures {
            let status = match s.status {
                SignatureStatus::Valid => "valid",
                SignatureStatus::Superseded => "superseded by a later version",
                SignatureStatus::ContentChanged => "CONTENT CHANGED SINCE SIGNING",
            };
            p.text(
                MARGIN,
                p.y,
                FONT,
                400,
                &format!(
                    "{} by {} ({}), {} UTC, version {}: {status}. Content SHA-256 {}",
                    s.meaning.label(),
                    s.name,
                    s.user,
                    s.at,
                    s.version,
                    s.content_hash
                ),
            );
            p.y += LINE;
        }
        p.y += LINE * 0.5;
    }
    let header = |p: &mut Page| {
        for (t, x, _) in COLS {
            p.text(x, p.y, FONT, 600, t);
        }
        p.y += 3.0;
        p.line(p.y);
        p.y += LINE;
    };
    header(&mut p);
    for e in entries {
        let user = format!("{} ({})", e.name, e.user);
        let mut action = e.action.clone();
        if let Some(s) = &e.signature {
            action = format!(
                "{action}. Signs content {}",
                &s.content_hash[..16.min(s.content_hash.len())]
            );
        }
        let cells = [
            e.seq.to_string(),
            e.at.clone(),
            user,
            action,
            objects_text(session, &e.objects),
            e.reason.clone(),
            e.hash.clone(),
        ];
        let wrapped: Vec<Vec<String>> = cells
            .iter()
            .zip(COLS)
            .map(|(c, (_, _, w))| wrap(c, w))
            .collect();
        let rows = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        if p.y + rows as f64 * LINE > PAGE_H - MARGIN - 10.0 {
            pages.push(std::mem::replace(&mut p, Page::new()));
            header(&mut p);
        }
        for (lines, (_, x, _)) in wrapped.iter().zip(COLS) {
            for (i, l) in lines.iter().enumerate() {
                p.text(x, p.y + i as f64 * LINE, FONT, 400, l);
            }
        }
        p.y += rows as f64 * LINE + 2.0;
        p.line(p.y - LINE + 2.0);
    }
    pages.push(p);
    let n = pages.len();
    pages
        .iter()
        .enumerate()
        .map(|(i, pg)| pg.svg(&format!("{title}: audit trail, page {} of {n}", i + 1)))
        .collect()
}
