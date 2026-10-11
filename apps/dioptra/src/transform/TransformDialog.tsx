// Data → Transform: whole-table operations that create a derived, read-only table which
// remembers its recipe and rebuilds when the source changes (transform.rs).
import { useState } from 'react';
import { DialogHeader, openDialog } from '../app/dialogs';
import { Segmented, Switch } from '../analysis/OptionsPanel';
import * as obj from '../engine/objects';
import type { NormRef, TransformOpT } from '../engine/objects';
import type { Id, TableMeta } from '../engine/types';
import { notify } from '../store/notices';
import { useProject } from '../store/project';
import { mutate } from '../store/sync';
import { useTables } from '../store/table';

type OpName = TransformOpT['op'];

export const OPS: { op: OpName; label: string; hint: string }[] = [
  { op: 'transpose', label: 'Transpose', hint: 'Rows become data sets and data sets become rows.' },
  { op: 'sort', label: 'Sort rows', hint: 'Order the rows by one column.' },
  { op: 'filter_rows', label: 'Filter rows', hint: 'Keep the rows where a formula is true, e.g. A > 0 && !isna(B).' },
  { op: 'remove_baseline', label: 'Remove baseline', hint: 'Subtract or divide by a baseline value in each column.' },
  { op: 'normalize', label: 'Normalize', hint: 'Scale each column so one value maps to 0 and another to 1 (or 100%).' },
  { op: 'fraction_of_total', label: 'Fraction of total', hint: 'Each value as a fraction of its row, column or table total.' },
  { op: 'average_replicates', label: 'Average replicates', hint: 'Replicate subcolumns become mean, SD and N.' },
  { op: 'prune', label: 'Prune rows', hint: 'Keep one row in every k, e.g. to thin a long time course.' },
  { op: 'pivot_long_to_wide', label: 'Pivot long to wide', hint: 'One row per identifier and one data set per key.' },
  { op: 'pivot_wide_to_long', label: 'Pivot wide to long', hint: 'A variables table of identifier, variable and value.' },
  { op: 'stack', label: 'Stack columns', hint: 'Columns to a long table of group and value; empty cells are dropped.' },
];

function activeTable(): Id | null {
  const { activeId, view } = useProject.getState();
  const item = view?.items.find((i) => i.id === activeId);
  if (item?.kind === 'table') return item.id;
  if (item?.derived_from && view?.items.find((i) => i.id === item.derived_from)?.kind === 'table') return item.derived_from;
  return null;
}

export async function transformTable(initial?: OpName): Promise<void> {
  const id = activeTable();
  if (!id) {
    notify('Open a table first; a transform makes a new table from it.');
    return;
  }
  const meta = useTables.getState().metas[id] ?? (await useTables.getState().loadMeta(id));
  if (!meta) return;
  const op = await openDialog<TransformOpT | null>((close) => <TransformDialog meta={meta} initial={initial} close={close} />, null);
  if (!op) return;
  await mutate((e) => obj.transform(e, id, op));
}

function ColSelect({
  meta,
  value,
  onChange,
  label,
  allowNone,
  testId,
}: {
  meta: TableMeta;
  value: number | null;
  onChange: (v: number | null) => void;
  label: string;
  allowNone?: string;
  testId?: string;
}) {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      <select className="input" value={value ?? ''} data-testid={testId} onChange={(e) => onChange(e.target.value === '' ? null : Number(e.target.value))}>
        {allowNone && <option value="">{allowNone}</option>}
        {meta.columns.map((c) => (
          <option key={c.index} value={c.index}>
            {c.label}: {c.header}
          </option>
        ))}
      </select>
    </label>
  );
}

function NormRefField({ label, value, onChange }: { label: string; value: NormRef; onChange: (v: NormRef) => void }) {
  const [num, setNum] = useState(value.kind === 'value' ? String(value.value) : '0');
  return (
    <div className="form-row">
      <label className="field">
        <span className="field-label">{label}</span>
        <select
          className="input"
          value={value.kind}
          onChange={(e) => {
            const k = e.target.value as NormRef['kind'];
            onChange(k === 'value' ? { kind: 'value', value: Number(num) || 0 } : ({ kind: k } as NormRef));
          }}
        >
          <option value="min">Smallest value</option>
          <option value="max">Largest value</option>
          <option value="first_row">First row</option>
          <option value="last_row">Last row</option>
          <option value="value">A number</option>
        </select>
      </label>
      {value.kind === 'value' && (
        <label className="field field-num">
          <span className="field-label">Value</span>
          <input
            className="input num"
            value={num}
            onChange={(e) => {
              setNum(e.target.value);
              const v = Number(e.target.value);
              if (Number.isFinite(v)) onChange({ kind: 'value', value: v });
            }}
          />
        </label>
      )}
    </div>
  );
}

function TransformDialog({ meta, initial, close }: { meta: TableMeta; initial?: OpName; close: (op: TransformOpT | null) => void }) {
  const firstY = meta.columns.find((c) => c.role === 'y')?.index ?? 0;
  const [op, setOp] = useState<OpName>(initial ?? 'sort');
  const [col, setCol] = useState<number>(firstY);
  const [desc, setDesc] = useState(false);
  const [predicate, setPredicate] = useState('');
  const [baseKind, setBaseKind] = useState<'first_row' | 'mean_of_rows' | 'column' | 'row_title'>('first_row');
  const [baseRows, setBaseRows] = useState('1');
  const [baseCol, setBaseCol] = useState<number>(firstY);
  const [baseTitle, setBaseTitle] = useState('');
  const [mode, setMode] = useState<'subtract' | 'divide' | 'percent'>('subtract');
  const [by, setBy] = useState<'row' | 'column' | 'table'>('column');
  const [keep, setKeep] = useState('2');
  const [offset, setOffset] = useState('1');
  const [idCol, setIdCol] = useState<number | null>(meta.columns[0]?.index ?? 0);
  const [keyCol, setKeyCol] = useState<number | null>(meta.columns[1]?.index ?? 0);
  const [valCol, setValCol] = useState<number | null>(meta.columns[2]?.index ?? 0);
  const [longId, setLongId] = useState<number | null>(null);
  const [lo, setLo] = useState<NormRef>({ kind: 'min' });
  const [hi, setHi] = useState<NormRef>({ kind: 'max' });
  const [pct, setPct] = useState(true);
  const [stackCols, setStackCols] = useState<number[]>([]);

  const rows1 = baseRows
    .split(/[,\s]+/)
    .filter(Boolean)
    .map(Number);
  const build = (): TransformOpT | string => {
    switch (op) {
      case 'transpose':
        return { op };
      case 'average_replicates':
        return { op };
      case 'sort':
        return { op, col, descending: desc };
      case 'filter_rows':
        return predicate.trim() ? { op, predicate: predicate.trim().replace(/^=\s*/, '') } : 'Write the condition rows must meet.';
      case 'remove_baseline': {
        if (baseKind === 'mean_of_rows') {
          if (!rows1.length || rows1.some((r) => !Number.isInteger(r) || r < 1 || r > meta.rows)) return `Rows are numbers from 1 to ${meta.rows}.`;
          return { op, baseline: { kind: 'mean_of_rows', rows: rows1.map((r) => r - 1) }, mode };
        }
        if (baseKind === 'column') return { op, baseline: { kind: 'column', col: baseCol }, mode };
        if (baseKind === 'row_title') return baseTitle.trim() ? { op, baseline: { kind: 'row_title', title: baseTitle.trim() }, mode } : 'Type the row title of the baseline row.';
        return { op, baseline: { kind: 'first_row' }, mode };
      }
      case 'fraction_of_total':
        return { op, by };
      case 'prune': {
        const k = Number(keep);
        const o = Number(offset);
        if (!Number.isInteger(k) || k < 1) return 'Keep one row in a whole number of rows (1 or more).';
        if (!Number.isInteger(o) || o < 1) return 'The first row kept is a row number from 1.';
        return { op, keep_every: k, offset: o - 1 };
      }
      case 'pivot_long_to_wide':
        if (idCol === null || keyCol === null || valCol === null) return 'Choose the three columns.';
        if (new Set([idCol, keyCol, valCol]).size < 3) return 'The identifier, key and value must be three different columns.';
        return { op, id_col: idCol, key_col: keyCol, value_col: valCol };
      case 'pivot_wide_to_long':
        return { op, id_col: longId };
      case 'normalize':
        return { op, lo, hi, as_percent: pct };
      case 'stack':
        return { op, columns: stackCols };
      default:
        return 'Choose an operation.';
    }
  };
  const result = build();
  const ok = typeof result !== 'string';
  const info = OPS.find((o) => o.op === op)!;
  return (
    <form
      className="modal-body transform-dialog"
      data-testid="transform-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) close(result);
      }}
    >
      <DialogHeader title="Transform" sub={`A new table from "${meta.title}"; the original stays as it is and the new one follows its changes.`} />
      <div className="tf-split">
        <div className="tf-ops" role="listbox" aria-label="Operation">
          {OPS.map((o) => (
            <button
              key={o.op}
              type="button"
              role="option"
              aria-selected={o.op === op}
              className={'tf-op' + (o.op === op ? ' is-active' : '')}
              data-testid={`tf-${o.op}`}
              onClick={() => setOp(o.op)}
            >
              {o.label}
            </button>
          ))}
        </div>
        <div className="tf-fields">
          <p className="tf-hint">{info.hint}</p>
          {op === 'sort' && (
            <>
              <ColSelect meta={meta} label="By column" value={col} onChange={(v) => setCol(v ?? 0)} testId="tf-sort-col" />
              <Segmented
                label="Order"
                choices={[
                  { value: 'asc', label: 'Smallest first' },
                  { value: 'desc', label: 'Largest first' },
                ]}
                value={desc ? 'desc' : 'asc'}
                onChange={(v) => setDesc(v === 'desc')}
              />
            </>
          )}
          {op === 'filter_rows' && (
            <label className="field">
              <span className="field-label">Keep rows where</span>
              <input className="input mono" value={predicate} placeholder="A > 0 && !isna(B)" onChange={(e) => setPredicate(e.target.value)} data-testid="tf-predicate" spellCheck={false} />
              <span className="field-hint">Data set letters name the values in the row; the formula language of the formula bar.</span>
            </label>
          )}
          {op === 'remove_baseline' && (
            <>
              <label className="field">
                <span className="field-label">Baseline</span>
                <select className="input" value={baseKind} onChange={(e) => setBaseKind(e.target.value as typeof baseKind)}>
                  <option value="first_row">The first row of each column</option>
                  <option value="mean_of_rows">The mean of chosen rows</option>
                  <option value="column">Row by row, another column</option>
                  <option value="row_title">The row with a title</option>
                </select>
              </label>
              {baseKind === 'mean_of_rows' && (
                <label className="field">
                  <span className="field-label">Rows</span>
                  <input className="input" value={baseRows} onChange={(e) => setBaseRows(e.target.value)} placeholder="1, 2, 3" />
                </label>
              )}
              {baseKind === 'column' && <ColSelect meta={meta} label="Baseline column" value={baseCol} onChange={(v) => setBaseCol(v ?? 0)} />}
              {baseKind === 'row_title' && (
                <label className="field">
                  <span className="field-label">Row title</span>
                  <input className="input" value={baseTitle} onChange={(e) => setBaseTitle(e.target.value)} placeholder="Blank" />
                </label>
              )}
              <Segmented
                label="Remove by"
                choices={[
                  { value: 'subtract', label: 'Subtract' },
                  { value: 'divide', label: 'Divide' },
                  { value: 'percent', label: 'Percent of' },
                ]}
                value={mode}
                onChange={(v) => setMode(v as typeof mode)}
              />
            </>
          )}
          {op === 'fraction_of_total' && (
            <Segmented
              label="Total of"
              choices={[
                { value: 'column', label: 'Each column' },
                { value: 'row', label: 'Each row' },
                { value: 'table', label: 'The whole table' },
              ]}
              value={by}
              onChange={(v) => setBy(v as typeof by)}
            />
          )}
          {op === 'prune' && (
            <div className="form-row">
              <label className="field field-num">
                <span className="field-label">Keep 1 row in</span>
                <input className="input num" value={keep} onChange={(e) => setKeep(e.target.value)} />
              </label>
              <label className="field field-num">
                <span className="field-label">Starting at row</span>
                <input className="input num" value={offset} onChange={(e) => setOffset(e.target.value)} />
              </label>
            </div>
          )}
          {op === 'pivot_long_to_wide' && (
            <>
              <ColSelect meta={meta} label="Identifier (one row each)" value={idCol} onChange={setIdCol} />
              <ColSelect meta={meta} label="Key (one data set each)" value={keyCol} onChange={setKeyCol} />
              <ColSelect meta={meta} label="Value" value={valCol} onChange={setValCol} />
            </>
          )}
          {op === 'pivot_wide_to_long' && <ColSelect meta={meta} label="Identifier" value={longId} onChange={setLongId} allowNone="Row titles, or row numbers" />}
          {op === 'normalize' && (
            <>
              <NormRefField label="Maps to 0" value={lo} onChange={setLo} />
              <NormRefField label="Maps to 1" value={hi} onChange={setHi} />
              <div className="ex-row">
                <Switch checked={pct} onChange={setPct} label="As percent" />
                <span className="small">Show as percent (0 to 100)</span>
              </div>
            </>
          )}
          {op === 'stack' && (
            <fieldset className="choice-list">
              <legend className="field-label">Columns (none picked: every Y column)</legend>
              {meta.columns
                .filter((c) => c.role === 'y')
                .map((c) => (
                  <label key={c.index} className="opt-check">
                    <input
                      type="checkbox"
                      checked={stackCols.includes(c.index)}
                      onChange={(e) => setStackCols(e.target.checked ? [...stackCols, c.index].sort((a, b) => a - b) : stackCols.filter((x) => x !== c.index))}
                    />
                    {c.label}: {c.header}
                  </label>
                ))}
            </fieldset>
          )}
          {(op === 'transpose' || op === 'average_replicates') && <p className="muted small">No options.</p>}
          {!ok && <p className="field-error">{result}</p>}
        </div>
      </div>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!ok} data-testid="tf-create">
          Create table
        </button>
      </footer>
    </form>
  );
}
