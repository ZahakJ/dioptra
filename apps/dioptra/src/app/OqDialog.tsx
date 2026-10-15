// Records → Run OQ...: the automated operational qualification (Dioptra Lab, desktop only).
// Runs the engine's OQ a step at a time to show progress, then the result by section and
// check, with the report as PDF (signed as executed by the signed-in user) or CSV.
import { useEffect, useMemo, useRef, useState } from 'react';
import { baseName, desktopSavePath, downloadBytes, isDesktop } from '../io/files';
import { withEngine } from '../store/engine';
import { labEnabled, useLab } from '../store/lab';
import { notify, notifyError } from '../store/notices';
import { labCall, useRecords } from '../store/records';
import { Button, DialogHeader, Select, TextInput } from '../ui';
import { openDialog } from './dialogs';
import type { AppCommand } from './registry';
import '../styles/records.css';
import '../styles/oq.css';

export interface OqCheck {
  id: string;
  section: string;
  reference: string;
  quantity: string;
  expected: number | string;
  got: number | string;
  tolerance: string;
  pass: boolean;
  note?: string;
}

export interface OqSection {
  code: string;
  title: string;
  source: string;
  checks: number;
  failed: number;
  seconds: number;
}

export interface OqReport {
  product: string;
  version: string;
  target: string;
  os: string;
  date: string;
  fixtures_sha256: string;
  fixture_files: number;
  pass: boolean;
  totals: { checks: number; passed: number; failed: number; nist_sets: number; nist_checks: number; oracle_checks: number };
  seconds: number;
  sections: OqSection[];
  checks: OqCheck[];
  report_sha256: string;
}

interface OqStep {
  type: 'lab_oq';
  step: number;
  steps: number;
  next?: string | null;
  report?: OqReport | null;
}

const ROW_LIMIT = 500;

/** A number as the table shows it: up to 10 significant digits. */
export function numText(v: number | string): string {
  if (typeof v === 'string') return v;
  if (Number.isInteger(v) && Math.abs(v) < 1e15) return String(v);
  return Number(v.toPrecision(10)).toString();
}

export async function runOq(): Promise<void> {
  if (!labEnabled()) {
    notify('Run OQ is part of Dioptra Lab, in the validation pack.');
    useLab.getState().setDialogOpen(true);
    return;
  }
  await openDialog<null>((close) => <OqBody close={() => close(null)} />, null);
}

function OqBody({ close }: { close: () => void }) {
  const [progress, setProgress] = useState<{ done: number; total: number; current: string } | null>(null);
  const [report, setReport] = useState<OqReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState('failed');
  const stop = useRef(false);

  useEffect(() => {
    stop.current = false;
    void (async () => {
      setProgress({ done: 0, total: 0, current: 'Starting' });
      try {
        for (let i = 0; ; i++) {
          const r = await withEngine((e) => labCall<OqStep>(e, { type: 'lab_run_oq', step: i }));
          if (r.report) {
            setReport(r.report);
            setView(r.report.pass ? 'all' : 'failed');
            break;
          }
          setProgress({ done: r.step, total: r.steps, current: r.next ?? '' });
          if (stop.current) return;
          // Let the progress paint before the next step.
          await new Promise((res) => setTimeout(res, 0));
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setProgress(null);
      }
    })();
    return () => {
      stop.current = true;
    };
  }, []);

  const options = useMemo(() => {
    if (!report) return [];
    const failed = report.totals.failed;
    return [
      { value: 'failed', label: `Failed checks (${failed})` },
      { value: 'all', label: 'Every section' },
      ...[...new Set(report.sections.map((s) => s.code))].map((code) => ({ value: code, label: code })),
    ];
  }, [report]);

  const rows = useMemo(() => {
    if (!report || view === 'all') return [];
    return report.checks.filter((c) => (view === 'failed' ? !c.pass : c.section === view));
  }, [report, view]);

  return (
    <div className="modal-body modal-wide oq" data-testid="oq-dialog">
      <DialogHeader
        title="Operational qualification"
        sub="Runs the NIST StRD reference data sets and the oracle comparisons of the test suite inside this installation."
        onClose={close}
      />
      {progress && (
        <div className="oq-progress" role="status" data-testid="oq-progress">
          <div className="oq-bar" aria-hidden="true">
            <div style={{ width: `${(100 * progress.done) / Math.max(1, progress.total)}%` }} />
          </div>
          <span>
            {progress.total ? `${progress.done} of ${progress.total}` : ''}
            {progress.current ? `${progress.total ? ': ' : ''}${progress.current}` : ''}
          </span>
        </div>
      )}
      {error && (
        <p className="records-banner is-fail" role="alert">
          {error}
        </p>
      )}
      {report && (
        <>
          <p className={'records-banner ' + (report.pass ? 'is-ok' : 'is-fail')} role={report.pass ? undefined : 'alert'} data-testid="oq-result">
            {report.pass ? 'Pass' : 'Fail'}: {report.totals.passed.toLocaleString('en')} of {report.totals.checks.toLocaleString('en')} checks agree
            with their reference ({report.totals.nist_checks} on the {report.totals.nist_sets} NIST StRD data sets,{' '}
            {report.totals.oracle_checks.toLocaleString('en')} oracle comparisons) in {report.seconds.toFixed(1)} s.
          </p>
          <dl className="oq-ident">
            <dt>Build</dt>
            <dd>
              {report.product} {report.version}, {report.target}
            </dd>
            <dt>System</dt>
            <dd>{report.os}</dd>
            <dt>Fixture set</dt>
            <dd className="mono">{report.fixtures_sha256}</dd>
            <dt>Report</dt>
            <dd className="mono">{report.report_sha256}</dd>
          </dl>
          <div className="records-filters">
            <Select label="Show" value={view} options={options} onChange={setView} />
            {view !== 'all' && (
              <span className="muted">
                {rows.length > ROW_LIMIT ? `First ${ROW_LIMIT} of ${rows.length}; every check is in the CSV.` : `${rows.length} checks`}
              </span>
            )}
          </div>
          <div className="records-table-wrap">
            {view === 'all' ? (
              <table className="records-table" data-testid="oq-sections">
                <thead>
                  <tr>
                    <th>Section</th>
                    <th>Covers</th>
                    <th>Checks</th>
                    <th>Failed</th>
                    <th>Time</th>
                  </tr>
                </thead>
                <tbody>
                  {report.sections.map((s, i) => (
                    <tr key={i} className={s.failed ? 'is-fail' : undefined}>
                      <td className="mono nowrap">{s.code}</td>
                      <td>{s.title}</td>
                      <td className="num">{s.checks}</td>
                      <td className="num">{s.failed}</td>
                      <td className="num">{s.seconds.toFixed(2)} s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <table className="records-table" data-testid="oq-checks">
                <thead>
                  <tr>
                    <th>Check</th>
                    <th>Reference</th>
                    <th>Expected</th>
                    <th>Got</th>
                    <th>Tolerance</th>
                    <th>Result</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.slice(0, ROW_LIMIT).map((c) => (
                    <tr key={c.id} className={c.pass ? undefined : 'is-fail'}>
                      <td className="mono nowrap">{c.id}</td>
                      <td>
                        {c.reference}
                        {c.quantity && <span className="muted"> :: {c.quantity}</span>}
                      </td>
                      <td className="num">{numText(c.expected)}</td>
                      <td className="num">{numText(c.got)}</td>
                      <td className="nowrap">{c.tolerance}</td>
                      <td>{c.pass ? 'Pass' : 'Fail'}</td>
                    </tr>
                  ))}
                  {rows.length === 0 && (
                    <tr>
                      <td colSpan={6} className="muted">
                        No checks to show.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
      <footer className="modal-foot">
        {report && (
          <>
            <Button onClick={() => void exportOq('csv', report)} testId="oq-csv">
              Export CSV
            </Button>
            <Button onClick={() => void exportOq('pdf', report)} testId="oq-pdf">
              Export PDF...
            </Button>
          </>
        )}
        <Button variant="primary" onClick={close}>
          {progress ? 'Stop' : 'Close'}
        </Button>
      </footer>
    </div>
  );
}

/** For the PDF: the signed-in user's password to sign as executed, '' to leave the manual
 * signature block, or null when cancelled. */
function askExecutionSignature(): Promise<string | null> {
  const user = useRecords.getState().users?.current;
  if (!user) return Promise.resolve('');
  return openDialog<string | null>((close) => <SignBody name={user.name} id={user.id} close={close} />, null);
}

function SignBody({ name, id, close }: { name: string; id: string; close: (v: string | null) => void }) {
  const [password, setPassword] = useState('');
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="oq-sign"
      onSubmit={(e) => {
        e.preventDefault();
        if (password) close(password);
      }}
    >
      <DialogHeader
        title="Sign the OQ report"
        sub="The signature records your name, the time and its meaning, executed, on the PDF."
        onClose={() => close(null)}
      />
      <p className="records-manifest">
        Executed by {name} ({id})
      </p>
      <label className="field">
        <span className="field-label">Password of {name}</span>
        <TextInput type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
      </label>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button onClick={() => close('')}>Export unsigned</Button>
        <Button type="submit" variant="primary" disabled={!password}>
          Sign and export
        </Button>
      </footer>
    </form>
  );
}

async function exportOq(format: 'pdf' | 'csv', report: OqReport): Promise<void> {
  let password: string | null = '';
  if (format === 'pdf') {
    password = await askExecutionSignature();
    if (password === null) return;
  }
  const name = `${report.product} ${report.version} OQ report.${format}`;
  const command: Record<string, unknown> = { type: 'lab_oq_report', format };
  if (password) command.password = password;
  try {
    if (isDesktop()) {
      let path = await desktopSavePath(name, [{ name: format.toUpperCase(), extensions: [format] }]);
      if (!path) return;
      if (!path.toLowerCase().endsWith(`.${format}`)) path += `.${format}`;
      await withEngine((e) => labCall(e, { ...command, path }));
      notify(`Exported to ${baseName(path)}.`);
    } else {
      const r = await withEngine((e) => labCall<{ name: string; mime: string; base64: string }>(e, command));
      downloadBytes(r.name, r.mime, r.base64);
      notify(`Exported ${r.name}.`);
    }
  } catch (err) {
    notifyError(err);
  } finally {
    // A wrong password counts towards the lockout; keep the users file current.
    if (password) await useRecords.getState().syncUsers();
  }
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** The OQ runs in the desktop app only; the web build has no menu item. */
export const OQ_COMMANDS: AppCommand[] = inTauri
  ? [{ id: 'records.oq', title: 'Run OQ...', menu: 'Records', group: 'validation', keywords: ['oq', 'validation', 'qualification', 'iq', 'nist', 'gxp', 'part 11'], run: runOq }]
  : [];
