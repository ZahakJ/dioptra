// Records for regulated labs (Dioptra Lab): users, sign-in, regulated projects with an audit
// trail, electronic signatures, new versions, the audit-trail viewer and the audit report.
import { useEffect, useMemo, useState } from 'react';
import type { Engine } from '../engine/bridge';
import { setResponseGuard } from '../engine/guard';
import { onEngineReady } from '../engine/engine';
import type { Command, Response } from '../engine/types';
import { baseName, desktopSavePath, downloadBytes, isDesktop } from '../io/files';
import { withEngine } from '../store/engine';
import { labEnabled, useLab } from '../store/lab';
import { notify, notifyError, useNotices } from '../store/notices';
import { useProject } from '../store/project';
import {
  labCall,
  MEANING_LABEL,
  REASON_REQUIRED,
  recordsWarning,
  ROLE_LABEL,
  SIGN_IN_REQUIRED,
  useRecords,
  type AuditEntry,
  type Meaning,
  type RecordsStatus,
  type Role,
  type SignatureView,
} from '../store/records';
import { applyChanged, onChanged } from '../store/sync';
import { Button, DialogHeader, RadioList, Select, TextInput } from '../ui';
import { chooseDialog, openDialog, promptDialog } from './dialogs';
import type { AppCommand } from './registry';
import '../styles/records.css';

const FEATURE = 'Records for regulated labs';

/** "2026-01-15T10:00:00Z" → "2026-01-15 10:00:00 UTC". */
export function utc(at: string): string {
  const m = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})/.exec(at);
  return m ? `${m[1]} ${m[2]} UTC` : at;
}

// ---- Lab off ------------------------------------------------------------------------------

/** When Lab is off, explain and offer Help → Dioptra Lab; returns whether Lab is on. */
async function needLab(what: string): Promise<boolean> {
  if (labEnabled()) return true;
  const open = await openDialog<boolean>(
    (close) => (
      <div className="modal-body modal-narrow" data-testid="records-lab-off">
        <DialogHeader title={what} onClose={() => close(false)} />
        <p className="modal-text">
          {what} is part of Dioptra Lab, in {FEATURE}. Verifying a project's audit trail and signatures needs no
          license.
        </p>
        <footer className="modal-foot">
          <Button onClick={() => close(false)}>Close</Button>
          <Button variant="primary" onClick={() => close(true)} data-autofocus>
            Dioptra Lab...
          </Button>
        </footer>
      </div>
    ),
    false,
  );
  if (open) useLab.getState().setDialogOpen(true);
  return false;
}

// ---- sign in ------------------------------------------------------------------------------

/** Ask for user ID and password; resolves whether someone is now signed in. */
export function signIn(): Promise<boolean> {
  const { users } = useRecords.getState();
  if (!users?.users.length) {
    notify('No users are set up on this computer. Records → Set up users...');
    return Promise.resolve(false);
  }
  return openDialog<boolean>((close) => <SignInBody close={close} />, false);
}

function SignInBody({ close }: { close: (v: boolean) => void }) {
  const [id, setId] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="records-sign-in"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        try {
          await useRecords.getState().usersCommand({ type: 'lab_login', user: id, password });
          close(true);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          setPassword('');
        } finally {
          setBusy(false);
        }
      }}
    >
      <DialogHeader title="Sign in" sub="Your user ID and password on this computer." onClose={() => close(false)} />
      <label className="field">
        <span className="field-label">User ID</span>
        <TextInput value={id} onChange={(e) => setId(e.target.value)} autoComplete="username" testId="records-user" />
      </label>
      <label className="field">
        <span className="field-label">Password</span>
        <TextInput
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="current-password"
          invalid={!!error}
          testId="records-password"
        />
        {error && (
          <span className="field-error" role="alert">
            {error}
          </span>
        )}
      </label>
      <footer className="modal-foot">
        <Button onClick={() => close(false)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!id.trim() || !password || busy}>
          Sign in
        </Button>
      </footer>
    </form>
  );
}

export async function signOut(): Promise<void> {
  try {
    await useRecords.getState().usersCommand({ type: 'lab_logout' });
    notify('Signed out.');
  } catch (err) {
    notifyError(err);
  }
}

async function ensureSignedIn(): Promise<boolean> {
  if (useRecords.getState().users?.current) return true;
  return signIn();
}

// ---- users --------------------------------------------------------------------------------

const ROLE_CHOICES: { value: Role; label: string; hint: string }[] = [
  { value: 'analyst', label: 'Analyst', hint: 'Changes data and signs as author.' },
  { value: 'reviewer', label: 'Reviewer', hint: 'Also signs as reviewer or approver.' },
  { value: 'admin', label: 'Administrator', hint: 'Also adds, disables and resets users.' },
];

const WEB_NOTE =
  'In the browser, users are kept in this browser’s storage, which anyone with access to the browser can clear or copy. It is not suitable for regulated work; use the desktop app.';

export async function setUpUsers(): Promise<void> {
  const st = useRecords.getState();
  if (st.usersError) {
    notifyError(new Error(st.usersError));
    return;
  }
  if (!st.users?.users.length) {
    if (!(await needLab('Set up users'))) return;
    await openDialog<boolean>((close) => <SetupBody close={close} />, false);
    return;
  }
  await openDialog<null>((close) => <UsersBody close={() => close(null)} />, null);
}

function PasswordFields({
  password,
  setPassword,
  repeat,
  setRepeat,
}: {
  password: string;
  setPassword: (v: string) => void;
  repeat: string;
  setRepeat: (v: string) => void;
}) {
  return (
    <>
      <label className="field">
        <span className="field-label">Password</span>
        <TextInput type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
        <span className="field-hint">At least 8 characters.</span>
      </label>
      <label className="field">
        <span className="field-label">Password again</span>
        <TextInput
          type="password"
          autoComplete="new-password"
          value={repeat}
          onChange={(e) => setRepeat(e.target.value)}
          invalid={!!repeat && repeat !== password}
        />
      </label>
    </>
  );
}

function SetupBody({ close }: { close: (v: boolean) => void }) {
  const [id, setId] = useState('');
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const [error, setError] = useState<string | null>(null);
  const ready = id.trim() && name.trim() && password.length >= 8 && password === repeat;
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="records-setup"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!ready) return;
        try {
          await useRecords.getState().usersCommand({ type: 'lab_users_setup', id, name, password });
          notify(`${name.trim()} is the first administrator and is signed in.`);
          close(true);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }}
    >
      <DialogHeader
        title="Set up users"
        sub="Create the first administrator of this computer. Administrators add the other users."
        onClose={() => close(false)}
      />
      {!isDesktop() && <p className="records-note is-caution">{WEB_NOTE}</p>}
      <label className="field">
        <span className="field-label">User ID</span>
        <TextInput value={id} onChange={(e) => setId(e.target.value)} autoComplete="username" />
        <span className="field-hint">One word. IDs are never reused.</span>
      </label>
      <label className="field">
        <span className="field-label">Full name</span>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} />
        <span className="field-hint">Shown in the audit trail and on signatures.</span>
      </label>
      <PasswordFields password={password} setPassword={setPassword} repeat={repeat} setRepeat={setRepeat} />
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      <footer className="modal-foot">
        <Button onClick={() => close(false)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ready}>
          Create administrator
        </Button>
      </footer>
    </form>
  );
}

function UsersBody({ close }: { close: () => void }) {
  const users = useRecords((s) => s.users);
  const isAdmin = users?.current?.role === 'admin';
  const [adding, setAdding] = useState(false);
  const [showLog, setShowLog] = useState(false);
  const run = async (command: Record<string, unknown>, done: string) => {
    if (!(await needLab('Managing users'))) return;
    try {
      await useRecords.getState().usersCommand(command);
      notify(done);
    } catch (err) {
      notifyError(err);
    }
  };
  const reset = async (id: string, name: string) => {
    const password = await promptDialog({
      title: `New password for ${name}`,
      label: 'Password',
      hint: 'At least 8 characters. The account is unlocked too.',
      confirm: 'Set password',
      validate: (v) => (v.length < 8 ? 'At least 8 characters.' : null),
    });
    if (password) await run({ type: 'lab_user_reset_password', id, password }, `Password of ${name} set.`);
  };
  return (
    <div className="modal-body modal-wide" data-testid="records-users">
      <DialogHeader
        title="Users"
        sub={isAdmin ? 'Users of this computer. Disabled users stay listed; their IDs are never reused.' : 'Sign in as an administrator to add, disable or reset users.'}
        onClose={close}
      />
      {!isDesktop() && <p className="records-note is-caution">{WEB_NOTE}</p>}
      <div className="records-table-wrap">
        <table className="records-table">
          <thead>
            <tr>
              <th>User ID</th>
              <th>Full name</th>
              <th>Role</th>
              <th>Status</th>
              {isAdmin && <th aria-label="Actions" />}
            </tr>
          </thead>
          <tbody>
            {users?.users.map((u) => (
              <tr key={u.id}>
                <td className="mono">{u.id}</td>
                <td>{u.name}</td>
                <td>{ROLE_LABEL[u.role]}</td>
                <td>{u.disabled ? 'Disabled' : u.locked ? 'Locked' : 'Active'}</td>
                {isAdmin && (
                  <td className="records-actions">
                    {!u.disabled && (
                      <Button size="sm" variant="quiet" onClick={() => void reset(u.id, u.name)}>
                        {u.locked ? 'Reset and unlock' : 'Reset password'}
                      </Button>
                    )}
                    {!u.disabled && u.id !== users.current?.id && (
                      <Button size="sm" variant="quiet" onClick={() => void run({ type: 'lab_user_disable', id: u.id }, `${u.name} is disabled.`)}>
                        Disable
                      </Button>
                    )}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {adding && <AddUser done={() => setAdding(false)} />}
      {showLog && (
        <ul className="records-log" aria-label="Access log">
          {[...(users?.log ?? [])].reverse().map((l) => (
            <li key={l.seq}>
              <span className="mono muted">{utc(l.at)}</span> {l.detail} <span className="muted">by {l.by}</span>
            </li>
          ))}
        </ul>
      )}
      <footer className="modal-foot">
        <Button variant="quiet" onClick={() => setShowLog(!showLog)} pressed={showLog}>
          Access log ({users?.log.length ?? 0})
        </Button>
        {isAdmin && !adding && (
          <Button onClick={() => setAdding(true)} testId="records-add-user">
            Add user...
          </Button>
        )}
        <Button variant="primary" onClick={close}>
          Done
        </Button>
      </footer>
    </div>
  );
}

function AddUser({ done }: { done: () => void }) {
  const [id, setId] = useState('');
  const [name, setName] = useState('');
  const [role, setRole] = useState<Role>('analyst');
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const [error, setError] = useState<string | null>(null);
  const ready = id.trim() && name.trim() && password.length >= 8 && password === repeat;
  return (
    <form
      className="records-add"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!ready || !(await needLab('Adding users'))) return;
        try {
          await useRecords.getState().usersCommand({ type: 'lab_user_add', id, name, role, password });
          notify(`${name.trim()} added.`);
          done();
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }}
    >
      <label className="field">
        <span className="field-label">User ID</span>
        <TextInput value={id} onChange={(e) => setId(e.target.value)} />
      </label>
      <label className="field">
        <span className="field-label">Full name</span>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="field">
        <span className="field-label">Role</span>
        <RadioList label="Role" value={role} onChange={setRole} choices={ROLE_CHOICES} />
      </div>
      <PasswordFields password={password} setPassword={setPassword} repeat={repeat} setRepeat={setRepeat} />
      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      <div className="records-add-foot">
        <Button onClick={done}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ready}>
          Add user
        </Button>
      </div>
    </form>
  );
}

// ---- regulated project --------------------------------------------------------------------

async function afterRecords(): Promise<void> {
  await applyChanged({ ids: [], tree: true }, { open: false });
}

export async function makeRegulated(): Promise<void> {
  if (!(await needLab('Make this project regulated'))) return;
  if (useRecords.getState().status?.regulated) {
    notify('This project is regulated already.');
    return;
  }
  if (!(await ensureSignedIn())) return;
  const reason = await openDialog<string | null>((close) => <RegulateBody close={close} />, null);
  if (reason === null) return;
  try {
    await useRecords.getState().records({ type: 'lab_make_regulated', reason });
    await afterRecords();
    notify('The audit trail is on. Every change now needs a reason.');
  } catch (err) {
    notifyError(err);
  }
}

function RegulateBody({ close }: { close: (v: string | null) => void }) {
  const [reason, setReason] = useState('');
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="records-regulate"
      onSubmit={(e) => {
        e.preventDefault();
        if (reason.trim()) close(reason);
      }}
    >
      <DialogHeader title="Make this project regulated" onClose={() => close(null)} />
      <p className="modal-text">
        From now on every change needs a signed-in user and a reason, and is recorded in the project&rsquo;s audit trail
        with the time and what changed. The trail cannot be turned off. Signed versions are locked.
      </p>
      <label className="field">
        <span className="field-label">Reason</span>
        <TextInput value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Study 12, GLP" />
      </label>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!reason.trim()}>
          Make regulated
        </Button>
      </footer>
    </form>
  );
}

/** Ask why a change is made; resolves `null` when cancelled. */
export function askReason(what: string): Promise<string | null> {
  return openDialog<string | null>((close) => <ReasonBody what={what} close={close} />, null);
}

function ReasonBody({ what, close }: { what: string; close: (v: string | null) => void }) {
  const recent = useRecords((s) => s.recentReasons);
  const [reason, setReason] = useState('');
  const submit = (r: string) => {
    if (!r.trim()) return;
    useRecords.getState().rememberReason(r);
    close(r.trim());
  };
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="records-reason"
      onSubmit={(e) => {
        e.preventDefault();
        submit(reason);
      }}
    >
      <DialogHeader title="Reason for the change" sub={what} onClose={() => close(null)} />
      <label className="field">
        <span className="field-label">Reason</span>
        <TextInput value={reason} onChange={(e) => setReason(e.target.value)} testId="records-reason-input" />
      </label>
      {recent.length > 0 && (
        <div className="records-recent" aria-label="Recent reasons">
          {recent.map((r) => (
            <Button key={r} size="sm" variant="secondary" onClick={() => submit(r)}>
              {r}
            </Button>
          ))}
        </div>
      )}
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!reason.trim()}>
          Change
        </Button>
      </footer>
    </form>
  );
}

// ---- signatures ---------------------------------------------------------------------------

export async function signProject(): Promise<void> {
  if (!(await needLab('Signing'))) return;
  const status = await useRecords.getState().refresh();
  if (!status?.regulated) {
    notify('Only a regulated project can be signed. Records → Make this project regulated...');
    return;
  }
  if (!(await ensureSignedIn())) return;
  await openDialog<boolean>((close) => <SignBody close={close} />, false);
}

function SignBody({ close }: { close: (v: boolean) => void }) {
  const user = useRecords((s) => s.users?.current);
  const status = useRecords((s) => s.status);
  const [meaning, setMeaning] = useState<Meaning>(user?.role === 'analyst' ? 'authored' : 'reviewed');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!user) return null;
  const choices = (['authored', 'reviewed', 'approved'] as Meaning[])
    .filter((m) => m === 'authored' || user.role !== 'analyst')
    .map((m) => ({ value: m, label: MEANING_LABEL[m] }));
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="records-sign"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        try {
          await useRecords.getState().records({ type: 'lab_sign', meaning, password });
          await afterRecords();
          notify(`Signed: ${MEANING_LABEL[meaning]} by ${user.name}. The project is locked.`);
          close(true);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
          setPassword('');
        } finally {
          setBusy(false);
        }
      }}
    >
      <DialogHeader
        title={`Sign version ${status?.version ?? 1}`}
        sub="The signature binds your name, the time and its meaning to the current content. The project is then locked."
        onClose={() => close(false)}
      />
      <div className="field">
        <span className="field-label">Meaning</span>
        <RadioList label="Meaning" value={meaning} onChange={setMeaning} choices={choices} />
      </div>
      <p className="records-manifest" data-testid="records-manifest">
        {MEANING_LABEL[meaning]} by {user.name} ({user.id}), {utc(new Date().toISOString())}
      </p>
      <label className="field">
        <span className="field-label">Password of {user.name}</span>
        <TextInput
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          invalid={!!error}
        />
        {error && (
          <span className="field-error" role="alert">
            {error}
          </span>
        )}
      </label>
      <footer className="modal-foot">
        <Button onClick={() => close(false)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!password || busy}>
          Sign
        </Button>
      </footer>
    </form>
  );
}

export async function newVersion(): Promise<void> {
  if (!(await needLab('New version'))) return;
  const status = await useRecords.getState().refresh();
  if (!status?.locked) {
    notify('This project is not signed; it can be changed as it is.');
    return;
  }
  if (!(await ensureSignedIn())) return;
  const reason = await promptDialog({
    title: `Start version ${status.version + 1}`,
    label: 'Reason',
    hint: 'The signatures of this version stay in the trail, marked superseded.',
    confirm: 'Start new version',
    validate: (v) => (v.trim() ? null : 'Enter a reason.'),
  });
  if (!reason) return;
  try {
    await useRecords.getState().records({ type: 'lab_new_version', reason });
    await afterRecords();
    notify(`Version ${status.version + 1} started. The project can be changed again.`);
  } catch (err) {
    notifyError(err);
  }
}

// ---- audit trail ----------------------------------------------------------------------------

export function openAuditTrail(): Promise<null> {
  return openDialog<null>((close) => <TrailBody close={() => close(null)} />, null);
}

const STATUS_TEXT: Record<SignatureView['status'], string> = {
  valid: 'Valid',
  superseded: 'Superseded',
  content_changed: 'Content changed since signing',
};

export function Manifestation({ s }: { s: SignatureView }) {
  return (
    <li className={'records-sig is-' + s.status}>
      <strong>{MEANING_LABEL[s.meaning]}</strong> by {s.name} ({s.user}), {utc(s.at)}, version {s.version}
      <span className="records-sig-status">{STATUS_TEXT[s.status]}</span>
    </li>
  );
}

function ChainBanner({ status }: { status: RecordsStatus }) {
  const warning = recordsWarning(status);
  return (
    <p className={'records-banner ' + (warning ? 'is-fail' : 'is-ok')} role={warning ? 'alert' : undefined} data-testid="records-chain">
      {warning ??
        `Chain intact: ${status.entries} entries, unchanged and in order; the project content matches the last entry. Version ${status.version}${status.locked ? ', signed and locked' : ''}.`}
    </p>
  );
}

function TrailBody({ close }: { close: () => void }) {
  const items = useProject((s) => s.view?.items);
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  const [status, setStatus] = useState<RecordsStatus | null>(null);
  const [user, setUser] = useState('');
  const [object, setObject] = useState('');
  const load = async () => {
    try {
      const r = await withEngine((e) =>
        labCall<{ status: RecordsStatus; entries: AuditEntry[] }>(e, { type: 'lab_audit_trail' }),
      );
      setEntries(r.entries);
      setStatus(r.status);
      useRecords.setState({ status: r.status });
    } catch (err) {
      notifyError(err);
    }
  };
  useEffect(() => {
    void load();
  }, []);
  const title = (id: string) => items?.find((i) => i.id === id)?.title ?? `${id.slice(0, 8)} (deleted)`;
  const userOptions = useMemo(() => {
    const seen = new Map<string, string>();
    for (const e of entries ?? []) seen.set(e.user, e.name);
    return [{ value: '', label: 'Every user' }, ...[...seen].map(([id, name]) => ({ value: id, label: `${name} (${id})` }))];
  }, [entries]);
  const objectOptions = useMemo(() => {
    const seen = new Set<string>();
    for (const e of entries ?? []) for (const o of e.objects ?? []) seen.add(o);
    return [{ value: '', label: 'Every object' }, ...[...seen].map((id) => ({ value: id, label: title(id) }))];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entries, items]);
  const shown = (entries ?? []).filter((e) => (!user || e.user === user) && (!object || (e.objects ?? []).includes(object)));
  return (
    <div className="modal-body modal-wide records-trail" data-testid="records-trail">
      <DialogHeader title="Audit trail" sub="Every change to this regulated project: who, when, what and why." onClose={close} />
      {status && !status.regulated ? (
        <p className="modal-text">This project is not regulated and has no audit trail. Records → Make this project regulated...</p>
      ) : (
        <>
          {status && <ChainBanner status={status} />}
          {status && status.signatures.length > 0 && (
            <ul className="records-sigs">
              {status.signatures.map((s) => (
                <Manifestation key={s.seq} s={s} />
              ))}
            </ul>
          )}
          <div className="records-filters">
            <Select label="User" value={user} options={userOptions} onChange={setUser} />
            <Select label="Object" value={object} options={objectOptions} onChange={setObject} />
            <span className="muted">
              {shown.length} of {entries?.length ?? 0} entries
            </span>
          </div>
          <div className="records-table-wrap">
            <table className="records-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Time</th>
                  <th>User</th>
                  <th>Action</th>
                  <th>Objects</th>
                  <th>Reason</th>
                  <th>Version</th>
                </tr>
              </thead>
              <tbody>
                {shown.map((e) => (
                  <tr key={e.seq} className={'is-' + e.kind}>
                    <td className="num">{e.seq}</td>
                    <td className="mono nowrap">{utc(e.at)}</td>
                    <td>
                      {e.name} <span className="muted">({e.user})</span>
                    </td>
                    <td>{e.action}</td>
                    <td>{(e.objects ?? []).map(title).join(', ')}</td>
                    <td>{e.reason}</td>
                    <td className="num">{e.version}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      <footer className="modal-foot">
        <Button onClick={() => void load()} testId="records-verify">
          Verify
        </Button>
        {status?.regulated && (
          <>
            <Button onClick={() => void exportAuditReport('csv')}>Export CSV</Button>
            <Button onClick={() => void exportAuditReport('pdf')}>Export PDF</Button>
          </>
        )}
        <Button variant="primary" onClick={close}>
          Close
        </Button>
      </footer>
    </div>
  );
}

export async function exportAuditReport(format?: 'csv' | 'pdf'): Promise<void> {
  const status = await useRecords.getState().refresh();
  if (!status?.regulated) {
    notify('This project has no audit trail.');
    return;
  }
  const f =
    format ??
    (await chooseDialog<'pdf' | 'csv'>({
      title: 'Export audit report',
      label: 'Format',
      initial: 'pdf',
      options: [
        { value: 'pdf', label: 'PDF', hint: 'For reading and printing, with the signatures and the chain status.' },
        { value: 'csv', label: 'CSV', hint: 'Every field of every entry, for a spreadsheet.' },
      ],
      confirm: 'Export',
    }));
  if (!f) return;
  try {
    const title = useProject.getState().view?.title || 'Untitled';
    if (isDesktop()) {
      let path = await desktopSavePath(`${title} audit trail.${f}`, [{ name: f.toUpperCase(), extensions: [f] }]);
      if (!path) return;
      if (!path.toLowerCase().endsWith(`.${f}`)) path += `.${f}`;
      await withEngine((e) => labCall(e, { type: 'lab_audit_report', format: f, path }));
      notify(`Exported to ${baseName(path)}.`);
    } else {
      const r = await withEngine((e) =>
        labCall<{ name: string; mime: string; base64: string }>(e, { type: 'lab_audit_report', format: f }),
      );
      downloadBytes(r.name, r.mime, r.base64);
      notify(`Exported ${r.name}.`);
    }
  } catch (err) {
    notifyError(err);
  }
}

// ---- status line ----------------------------------------------------------------------------

export function RecordsStatusItem() {
  const status = useRecords((s) => s.status);
  const user = useRecords((s) => s.users?.current);
  if (!status?.regulated && !user) return null;
  const warning = recordsWarning(status);
  return (
    <Button
      variant="quiet"
      size="sm"
      className={'sl-item sl-records' + (warning ? ' is-fail' : '')}
      onClick={() => void openAuditTrail()}
      testId="records-status"
      tipText="Audit trail"
    >
      {user ? user.name : 'Nobody signed in'}
      {status?.regulated && (
        <span className="muted">
          {' · '}
          {warning ? 'Audit trail broken' : `Regulated, version ${status.version}${status.locked ? ', signed' : ''}`}
        </span>
      )}
    </Button>
  );
}

// ---- guard and start-up -----------------------------------------------------------------------

function describe(command: Command): string {
  const words = command.type.replace(/_/g, ' ');
  return words.charAt(0).toUpperCase() + words.slice(1) + '.';
}

const notChanged = (message: string): Response => ({ type: 'error', message });

async function recordsGuard(engine: Engine, command: Command, response: Extract<Response, { type: 'error' }>): Promise<Response> {
  const type = command.type as string;
  if (type === 'lab_with_reason' || type.startsWith('lab_users') || type === 'lab_login') return response;
  if (response.message.startsWith(SIGN_IN_REQUIRED)) {
    if (!(await signIn())) return notChanged('Not changed: nobody is signed in.');
    const again = await engine.dispatch(command);
    return again.type === 'error' ? recordsGuard(engine, command, again) : again;
  }
  if (response.message.startsWith(REASON_REQUIRED) && type !== 'lab_new_version') {
    const reason = await askReason(describe(command));
    if (reason === null) return notChanged('Not changed: a regulated project needs a reason for every change.');
    return engine.dispatch({ type: 'lab_with_reason', reason, command } as unknown as Command);
  }
  return response;
}

let lastWarning: string | null = null;

function warnLoudly(status: RecordsStatus | null): void {
  const w = recordsWarning(status);
  if (w && w !== lastWarning) {
    useNotices.getState().push(
      { kind: 'error', text: w, action: { label: 'Audit trail', run: () => void openAuditTrail() } },
      0,
    );
  }
  lastWarning = w;
}

let started = false;

/** Install the guard and keep the records state current. */
export function startRecords(): void {
  if (started) return;
  started = true;
  setResponseGuard(recordsGuard);
  onEngineReady(() => {
    void useRecords.getState().load().then(() => warnLoudly(useRecords.getState().status));
  });
  onChanged((changed) => {
    if (changed.tree) void useRecords.getState().refresh().then(warnLoudly);
  });
  useRecords.subscribe((s, prev) => {
    if (s.usersError && s.usersError !== prev.usersError) notifyError(new Error(s.usersError));
  });
}

const hasUsers = () => !!useRecords.getState().users?.users.length;
const signedIn = () => !!useRecords.getState().users?.current;
const regulated = () => !!useRecords.getState().status?.regulated;

export const RECORDS_COMMANDS: AppCommand[] = [
  { id: 'records.users', title: 'Set up users...', menu: 'Records', group: 'users', keywords: ['users', 'roles', 'admin', 'part 11', 'gxp'], run: setUpUsers },
  { id: 'records.signIn', title: 'Sign in...', menu: 'Records', group: 'users', keywords: ['log in', 'login', 'user'], enabled: () => hasUsers() && !signedIn(), run: signIn },
  { id: 'records.signOut', title: 'Sign out', menu: 'Records', group: 'users', keywords: ['log out', 'logout'], enabled: signedIn, run: signOut },
  { id: 'records.regulate', title: 'Make this project regulated...', menu: 'Records', group: 'project', keywords: ['audit trail', 'gxp', 'glp', 'part 11'], enabled: () => !regulated(), run: makeRegulated },
  { id: 'records.sign', title: 'Sign...', menu: 'Records', group: 'project', keywords: ['signature', 'approve', 'review', 'e-signature'], enabled: regulated, run: signProject },
  { id: 'records.newVersion', title: 'New version...', menu: 'Records', group: 'project', keywords: ['unlock', 'version'], enabled: () => !!useRecords.getState().status?.locked, run: newVersion },
  { id: 'records.trail', title: 'Audit trail...', menu: 'Records', group: 'trail', keywords: ['verify', 'history', 'who', 'part 11'], run: openAuditTrail },
  { id: 'records.report', title: 'Export audit report...', menu: 'Records', group: 'trail', keywords: ['audit', 'pdf', 'csv'], enabled: regulated, run: () => exportAuditReport() },
];
