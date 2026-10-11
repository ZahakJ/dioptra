// The results sheet: title and method line, assumption checks first, the summary sentences,
// the typed tables, then the checklist. Opens beside its table when both are open.
import { useEffect, useMemo, useRef, useState } from 'react';
import { create } from 'zustand';
import { showContextMenu } from '../app/ContextMenu';
import type { WorkAreaProps } from '../app/workArea';
import * as obj from '../engine/objects';
import type { Sheet } from '../engine/objects';
import type { Id, PValueStyle } from '../engine/types';
import { TableView } from '../grid/TableView';
import { editAnalysisOptions } from '../analysis/actions';
import { makeGraphFromAnalysis } from '../graph/actions';
import { withEngine } from '../store/engine';
import { notify, notifyError } from '../store/notices';
import { itemById, usePValueStyle, useProject } from '../store/project';
import { applyChanged, onChanged } from '../store/sync';
import { kindTitle } from '../analysis/AnalyzeDialog';
import { displayTable, normRange, rangeTsv, summarizeChecks, totalN, type CellRange, type DisplayTable } from './sheetModel';

/** Per-session: whether the source table is hidden beside a sheet. */
export const useSplit = create<{ hidden: Record<Id, boolean>; toggle(id: Id): void }>((set) => ({
  hidden: {},
  toggle: (id) => set((s) => ({ hidden: { ...s.hidden, [id]: !s.hidden[id] } })),
}));

/** Sheets shared by the work area and the inspector: one fetch per change. */
const useSheets = create<{ sheets: Record<Id, Sheet>; errors: Record<Id, string> }>(() => ({ sheets: {}, errors: {} }));
const inflight = new Map<Id, Promise<void>>();
const sheetSeq: Record<Id, number> = {};

function loadSheet(id: Id, force = false): Promise<void> {
  const running = inflight.get(id);
  if (running && !force) return running;
  const n = (sheetSeq[id] = (sheetSeq[id] ?? 0) + 1);
  const p = withEngine((e) => obj.getResults(e, id))
    .then((s) => {
      if (n !== sheetSeq[id]) return;
      useSheets.setState((st) => {
        const errors = { ...st.errors };
        delete errors[id];
        return { sheets: { ...st.sheets, [id]: s }, errors };
      });
      // Fetching recomputed a stale sheet: let the rail and tabs drop their stale marks.
      if (itemById(id)?.stale) void useProject.getState().refresh();
    })
    .catch((err: unknown) => {
      if (n !== sheetSeq[id]) return;
      useSheets.setState((st) => ({ errors: { ...st.errors, [id]: err instanceof Error ? err.message : String(err) } }));
    })
    .finally(() => {
      if (inflight.get(id) === p) inflight.delete(id);
    });
  inflight.set(id, p);
  return p;
}

let listening = false;
function listen(): void {
  if (listening) return;
  listening = true;
  onChanged((c) => {
    const loaded = useSheets.getState().sheets;
    for (const id of c.ids) if (loaded[id] || useSheets.getState().errors[id]) void loadSheet(id, true);
  });
}

/** Load a sheet and keep it fresh: refetch when a change touches the analysis or the p style changes. */
export function useSheet(id: Id): { sheet: Sheet | null; error: string | null } {
  const sheet = useSheets((s) => s.sheets[id] ?? null);
  const error = useSheets((s) => s.errors[id] ?? null);
  const style = usePValueStyle();
  const first = useRef(true);
  useEffect(() => {
    listen();
    void loadSheet(id, !first.current);
    first.current = false;
  }, [id, style]);
  return { sheet, error };
}

export function ResultsSheetView({ item }: WorkAreaProps) {
  const tabs = useProject((s) => s.tabs);
  const hidden = useSplit((s) => !!s.hidden[item.id]);
  const table = item.derived_from ? itemById(item.derived_from) : undefined;
  const split = !!table && tabs.includes(table.id) && !hidden;
  const sheet = <SheetPane id={item.id} title={item.title} tableTitle={table?.title} splitAvailable={!!table && tabs.includes(table.id)} />;
  if (!split || !table) return sheet;
  return (
    <div className="split" data-testid="split-view">
      <div className="split-pane split-table">
        <TableView item={table} />
      </div>
      <div className="split-pane split-sheet">{sheet}</div>
    </div>
  );
}

function SheetPane({
  id,
  title,
  tableTitle,
  splitAvailable,
}: {
  id: Id;
  title: string;
  tableTitle?: string;
  splitAvailable: boolean;
}) {
  const { sheet, error } = useSheet(id);
  const style = usePValueStyle();
  const hidden = useSplit((s) => !!s.hidden[id]);
  if (error) return <div className="sheet-wrap"><p className="text-error sheet-error">{error}</p></div>;
  if (!sheet) return <div className="loading-line">Computing {title}…</div>;
  const n = totalN(sheet);
  const method = [kindTitle(sheet.analysis_kind), tableTitle && `data from ${tableTitle}`, n !== null && `n = ${n}`]
    .filter(Boolean)
    .join(' · ');

  const copySheet = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const copy = async (format: 'tsv' | 'markdown') => {
      try {
        const text = await withEngine((en) => obj.exportResults(en, id, format));
        await navigator.clipboard.writeText(text);
        notify(format === 'tsv' ? 'Copied the sheet as TSV.' : 'Copied the sheet as Markdown.');
      } catch (err) {
        notifyError(err);
      }
    };
    showContextMenu(r.left, r.bottom + 4, [
      { label: 'Copy as TSV (spreadsheets)', testId: 'copy-tsv', run: () => copy('tsv') },
      { label: 'Copy as Markdown (notes, papers)', testId: 'copy-markdown', run: () => copy('markdown') },
    ], 'Copy sheet');
  };

  return (
    <div className="sheet-wrap" data-testid="results-sheet">
      <div className="sheet-bar">
        {splitAvailable && (
          <button type="button" className="btn btn-small btn-ghost" onClick={() => useSplit.getState().toggle(id)} data-testid="sheet-split">
            {hidden ? 'Show table beside' : 'Hide table'}
          </button>
        )}
        <span className="sheet-bar-gap" />
        <button type="button" className="btn btn-small" onClick={() => void editAnalysisOptions(id)} data-testid="sheet-options">
          Options
        </button>
        <button type="button" className="btn btn-small" onClick={copySheet} data-testid="sheet-copy">
          Copy sheet
        </button>
        <button type="button" className="btn btn-small btn-primary" onClick={() => void makeGraphFromAnalysis(id)} data-testid="sheet-make-graph">
          Make graph
        </button>
      </div>
      <article className="sheet">
        <header className="sheet-head">
          <h1 data-testid="sheet-title">{sheet.title || title}</h1>
          <p className="sheet-method">{method}</p>
          {sheet.methods && (
            <p className="sheet-methods" data-testid="sheet-methods">
              {sheet.methods}
            </p>
          )}
        </header>
        {sheet.stale && (
          <div className="sheet-stale" role="status" data-testid="sheet-stale">
            <span>The data changed since these results were computed.</span>
            <button
              type="button"
              className="btn btn-small"
              data-testid="sheet-recompute"
              onClick={() =>
                void withEngine((e) => obj.getResults(e, id))
                  .then(() => applyChanged({ ids: [id], tree: true }))
                  .catch(notifyError)
              }
            >
              Recompute
            </button>
          </div>
        )}
        {sheet.message && (
          <p className="sheet-message" data-testid="sheet-message">
            {sheet.message}
          </p>
        )}
        {sheet.assumptions.length > 0 && <Assumptions sheet={sheet} />}
        {sheet.summary.length > 0 && (
          <section className="sheet-summary" aria-label="Summary" data-testid="sheet-summary">
            {sheet.summary.map((s, i) => (
              <p key={i}>{s}</p>
            ))}
          </section>
        )}
        <Sections sheet={sheet} style={style} />
        {sheet.checklist.length > 0 && (
          <section className="sheet-checklist" aria-label="Checklist">
            <h2>Before you report this</h2>
            <ul>
              {sheet.checklist.map((c, i) => (
                <li key={i}>{c}</li>
              ))}
            </ul>
          </section>
        )}
      </article>
    </div>
  );
}

function Assumptions({ sheet }: { sheet: Sheet }) {
  const sum = summarizeChecks(sheet);
  const [open, setOpen] = useState(!sum.collapsed);
  useEffect(() => setOpen(!sum.collapsed), [sum.collapsed]);
  return (
    <section className={'sheet-checks' + (sum.collapsed ? '' : ' has-doubt')} aria-label="Assumptions" data-testid="sheet-assumptions">
      <button type="button" className="checks-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span className="checks-title">Assumptions</span>
        <span className="checks-sum">{sum.text}</span>
        <span className="checks-toggle">{open ? 'Hide' : 'Show'}</span>
      </button>
      {open && (
        <ul className="checks-list">
          {sheet.assumptions.map((a, i) => (
            <li key={i} className={`check check-${a.status}`} data-status={a.status}>
              <CheckGlyph status={a.status} />
              <div>
                <div className="check-name">
                  {a.name}
                  <span className="check-status">{STATUS_LABEL[a.status]}</span>
                </div>
                <p className="check-detail">{a.detail}</p>
                {a.alternative && (
                  <p className="check-alt" data-testid="check-alternative">
                    {a.alternative}
                  </p>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

const STATUS_LABEL: Record<string, string> = { pass: 'passed', caution: 'in doubt', fail: 'failed', not_checked: 'not checkable from the data' };

function CheckGlyph({ status }: { status: string }) {
  return (
    <svg className="check-glyph" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.4">
      {status === 'pass' && <path d="M3 7.5l2.5 2.5L11 4.5" />}
      {status === 'caution' && (
        <>
          <path d="M7 1.8l5.6 10H1.4z" strokeLinejoin="round" />
          <path d="M7 5.5v3M7 10v.4" />
        </>
      )}
      {status === 'fail' && (
        <>
          <circle cx="7" cy="7" r="5.5" />
          <path d="M4.8 4.8l4.4 4.4M9.2 4.8l-4.4 4.4" />
        </>
      )}
      {status === 'not_checked' && <path d="M3.5 7h7" />}
    </svg>
  );
}

function Sections({ sheet, style }: { sheet: Sheet; style: PValueStyle }) {
  const tables = useMemo(() => sheet.sections.map((s) => displayTable(s.table, style)), [sheet, style]);
  const [sel, setSel] = useState<{ range: CellRange; anchor: { r: number; c: number } } | null>(null);
  const dragging = useRef(false);
  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);
  const copy = (e: React.ClipboardEvent) => {
    if (!sel) return;
    const t = tables[sel.range.section];
    if (!t) return;
    const text = window.getSelection()?.toString();
    if (text && text.length > 0 && !(e.target as HTMLElement).closest('.rt')) return; // a text selection elsewhere wins
    e.preventDefault();
    e.clipboardData.setData('text/plain', rangeTsv(t, sel.range));
  };
  return (
    <div className="sheet-sections" onCopy={copy}>
      {sheet.sections.map((s, si) => (
        <section key={si} className="sheet-section" aria-label={s.title}>
          <h2>{s.title}</h2>
          <ResultTableView
            t={tables[si]!}
            section={si}
            sel={sel?.range.section === si ? sel.range : null}
            onDown={(r, c, shift) => {
              dragging.current = true;
              window.getSelection()?.removeAllRanges();
              if (shift && sel && sel.range.section === si) setSel({ anchor: sel.anchor, range: normRange(si, sel.anchor, { r, c }) });
              else setSel({ anchor: { r, c }, range: normRange(si, { r, c }, { r, c }) });
            }}
            onEnter={(r, c) => {
              if (dragging.current && sel && sel.range.section === si) setSel({ anchor: sel.anchor, range: normRange(si, sel.anchor, { r, c }) });
            }}
            onKey={(e) => {
              if (!sel) return;
              if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
                const t = tables[sel.range.section];
                if (t && navigator.clipboard) {
                  void navigator.clipboard.writeText(rangeTsv(t, sel.range)).catch(() => undefined);
                }
              } else if (e.key === 'Escape') setSel(null);
            }}
          />
        </section>
      ))}
    </div>
  );
}

function ResultTableView({
  t,
  section,
  sel,
  onDown,
  onEnter,
  onKey,
}: {
  t: DisplayTable;
  section: number;
  sel: CellRange | null;
  onDown: (r: number, c: number, shift: boolean) => void;
  onEnter: (r: number, c: number) => void;
  onKey: (e: React.KeyboardEvent) => void;
}) {
  const inSel = (r: number, c: number) => !!sel && r >= sel.r0 && r <= sel.r1 && c >= sel.c0 && c <= sel.c1;
  return (
    <div className="rt-scroll">
      <table className="rt" tabIndex={0} data-testid={`results-table-${section}`} onKeyDown={onKey}>
        <thead>
          <tr>
            {t.columns.map((c, i) => (
              <th key={i} className={t.numeric[i] ? 'is-num' : undefined} scope="col">
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {t.rows.map((row, r) => (
            <tr key={r}>
              {row.map((cell, c) => (
                <td
                  key={c}
                  className={
                    'rt-' + cell.kind + (t.numeric[c] ? ' is-num' : '') + (inSel(r, c) ? ' is-sel' : '')
                  }
                  data-raw={cell.raw}
                  title={cell.kind === 'number' || cell.kind === 'p' ? cell.raw : undefined}
                  onMouseDown={(e) => {
                    if (e.button !== 0) return;
                    e.preventDefault();
                    (e.currentTarget.closest('table') as HTMLElement | null)?.focus();
                    onDown(r, c, e.shiftKey);
                  }}
                  onMouseEnter={() => onEnter(r, c)}
                >
                  {cell.text}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
