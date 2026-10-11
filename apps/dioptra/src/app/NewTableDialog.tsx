import { useState } from 'react';
import type { SubcolumnFormat, TableKind } from '../engine/types';
import { Button, Field, NumberField, Select, TextInput } from '../ui';
import { DialogHeader } from './dialogs';
import { KIND_ICONS } from './icons';
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
            <span className="kind-glyph" aria-hidden="true">
              {(() => {
                const G = KIND_ICONS[k.kind];
                return <G />;
              })()}
            </span>
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
      <DialogHeader title="New table" sub="The kind decides which analyses and graphs fit. You can paste data in any kind." onClose={() => close(null)} />
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
        <Field label="Title">
          {(lid) => <TextInput aria-labelledby={lid} value={title} placeholder="Automatic" onChange={(e) => setTitle(e.target.value)} testId="new-table-title" />}
        </Field>
        <Field label={info.datasetWord} className="field-num">
          {(lid) => (
            <NumberField key={`ds-${kind}`} size="md" labelledBy={lid} value={Number(nDatasets)} min={1} max={500} integer nullable={false} onText={(t) => setDatasets(t)} testId="new-table-datasets" />
          )}
        </Field>
        <Field label="Rows" className="field-num">
          {(lid) => <NumberField size="md" labelledBy={lid} value={Number(rows)} min={0} max={1_000_000} integer nullable={false} step={10} onText={(t) => setRows(t)} testId="new-table-rows" />}
        </Field>
        {info.formats && (
          <Field label="Subcolumns">
            {(lid) => (
              <Select
                size="md"
                labelledBy={lid}
                value={fk}
                testId="new-table-format"
                options={FORMAT_CHOICES.map((f) => ({ value: f.value, label: f.label }))}
                onChange={(v) => setFormatKind(v)}
              />
            )}
          </Field>
        )}
        {info.formats && fk === 'values' && (
          <Field label="Replicates" className="field-num">
            {(lid) => (
              <NumberField key={`rep-${kind}`} size="md" labelledBy={lid} value={Number(reps)} min={1} max={100} integer nullable={false} onText={(t) => setReplicates(t)} testId="new-table-replicates" />
            )}
          </Field>
        )}
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!valid} testId="new-table-create">
          Create table
        </Button>
      </footer>
    </form>
  );
}
