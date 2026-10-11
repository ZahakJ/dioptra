import { useState } from 'react';
import type { SubcolumnFormat, TableKind } from '../engine/types';
import { DialogHeader } from './dialogs';
import { KINDS, kindInfo, type KindInfo } from './kinds';
import { FORMAT_CHOICES } from '../grid/actions';

export interface NewTableSpec {
  kind: TableKind;
  title: string;
  datasets: number;
  format: SubcolumnFormat;
  rows: number;
}

export function KindSketch({ info }: { info: KindInfo }) {
  return (
    <table className="kind-sketch" aria-hidden="true">
      <tbody>
        {info.sketch.map((row, i) => (
          <tr key={i} className={i === 0 ? 'ks-head' : undefined}>
            {row.map((c, j) => (
              <td key={j}>{c}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function KindPicker({
  value,
  onChange,
  suggested,
  compact,
}: {
  value: TableKind;
  onChange: (k: TableKind) => void;
  suggested?: TableKind;
  /** Smaller cards: name, sketch and sentence in less height. */
  compact?: boolean;
}) {
  return (
    <div className={'kind-grid' + (compact ? ' is-compact' : '')} role="radiogroup" aria-label="Table kind">
      {KINDS.map((k) => (
        <button
          key={k.kind}
          type="button"
          role="radio"
          aria-checked={value === k.kind}
          className="kind-card"
          data-testid={`kind-${k.kind}`}
          onClick={() => onChange(k.kind)}
          onKeyDown={(e) => {
            const i = KINDS.findIndex((x) => x.kind === value);
            const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
            if (d) {
              e.preventDefault();
              const next = KINDS[(i + d + KINDS.length) % KINDS.length]!;
              onChange(next.kind);
              (e.currentTarget.parentElement?.querySelector(`[data-testid="kind-${next.kind}"]`) as HTMLElement | null)?.focus();
            }
          }}
        >
          <span className="kind-name">
            <span className="kind-radio" aria-hidden="true">{value === k.kind ? '●' : '○'}</span>
            {k.name}
            {suggested === k.kind && <span className="tag">Suggested</span>}
          </span>
          <KindSketch info={k} />
          <span className="kind-sentence">{k.sentence}</span>
        </button>
      ))}
    </div>
  );
}

export function NewTableDialog({ close }: { close: (spec: NewTableSpec | null) => void }) {
  const [kind, setKind] = useState<TableKind>('column');
  const info = kindInfo(kind)!;
  const [title, setTitle] = useState('');
  const [datasets, setDatasets] = useState<string>('');
  const [rows, setRows] = useState('20');
  const [formatKind, setFormatKind] = useState<SubcolumnFormat['kind'] | ''>('');
  const [replicates, setReplicates] = useState('');
  const fk = formatKind || info.defaultFormat.kind;
  const reps = replicates || String(info.defaultFormat.kind === 'values' ? info.defaultFormat.n : 3);
  const nDatasets = datasets || String(info.defaultDatasets);
  const valid = /^\d+$/.test(nDatasets) && Number(nDatasets) >= 1 && Number(nDatasets) <= 500 && /^\d+$/.test(rows) && Number(rows) <= 1_000_000 && (fk !== 'values' || (/^\d+$/.test(reps) && Number(reps) >= 1 && Number(reps) <= 100));
  const submit = () => {
    if (!valid) return;
    const format: SubcolumnFormat = !info.formats
      ? info.defaultFormat
      : fk === 'values'
        ? { kind: 'values', n: Number(reps) }
        : ({ kind: fk } as SubcolumnFormat);
    close({ kind, title: title.trim(), datasets: Number(nDatasets), format, rows: Number(rows) });
  };
  return (
    <form
      className="modal-body modal-wide"
      data-testid="new-table-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <DialogHeader title="New table" sub="The kind decides which analyses and graphs fit. You can paste data in any kind." />
      <KindPicker
        value={kind}
        onChange={(k) => {
          setKind(k);
          setDatasets('');
          setFormatKind('');
          setReplicates('');
        }}
      />
      <div className="form-row">
        <label className="field">
          <span className="field-label">Title</span>
          <input className="input" value={title} placeholder="Automatic" onChange={(e) => setTitle(e.target.value)} data-testid="new-table-title" />
        </label>
        <label className="field field-num">
          <span className="field-label">{info.datasetWord}</span>
          <input className="input num" inputMode="numeric" value={nDatasets} onChange={(e) => setDatasets(e.target.value)} />
        </label>
        <label className="field field-num">
          <span className="field-label">Rows</span>
          <input className="input num" inputMode="numeric" value={rows} onChange={(e) => setRows(e.target.value)} />
        </label>
        {info.formats && (
          <label className="field">
            <span className="field-label">Subcolumns</span>
            <select className="input" value={fk} onChange={(e) => setFormatKind(e.target.value as SubcolumnFormat['kind'])}>
              {FORMAT_CHOICES.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </select>
          </label>
        )}
        {info.formats && fk === 'values' && (
          <label className="field field-num">
            <span className="field-label">Replicates</span>
            <input className="input num" inputMode="numeric" value={reps} onChange={(e) => setReplicates(e.target.value)} />
          </label>
        )}
      </div>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!valid} data-testid="new-table-create">
          Create table
        </button>
      </footer>
    </form>
  );
}
