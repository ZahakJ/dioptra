// The core inspector sections: cell provenance, column properties, table, history, project.
import { useEffect } from 'react';
import * as cmd from '../engine/commands';
import type { ColumnSource, PValueStyle, SubcolumnFormat, VariableType } from '../engine/types';
import { formatLabel, FORMAT_CHOICES, redoAction, undoAction, VARIABLE_TYPES } from '../grid/actions';
import { formatFull } from '../grid/clipboard';
import { useGrid, useSelection } from '../grid/store';
import { useHistory } from '../store/history';
import { useProject } from '../store/project';
import { mutate } from '../store/sync';
import { useTableMeta } from '../store/table';
import { Button, NumberField, Select } from '../ui';
import { CommitField, FieldRow, KV } from './fields';
import { registerInspectorSection, type InspectorContext } from './inspector';
import { kindInfo } from './kinds';

export function describeSource(s: ColumnSource): string {
  switch (s.kind) {
    case 'entered':
      return 'Typed by hand';
    case 'pasted':
      return `Pasted at ${s.at}`;
    case 'imported':
      return `Imported from ${s.file}${s.sheet ? `, sheet ${s.sheet}` : ''}, column ${s.column}`;
    case 'formula':
      return `Formula =${s.expr}`;
    case 'derived':
      return `Derived (${s.op})`;
  }
}

function CellSection({ ctx }: { ctx: InspectorContext }) {
  const info = useGrid((s) => s.activeInfo);
  const sel = useSelection(ctx.tableId);
  if (!info || info.table !== ctx.tableId || info.row !== sel.anchor.row || info.col !== sel.anchor.col) {
    return <p className="muted">Select a cell to see where its value came from.</p>;
  }
  const c = info.cell;
  const value =
    typeof c.v === 'number' ? formatFull(c.v) : c.t !== undefined ? `"${c.t}"` : c.e ? 'error' : 'empty';
  const states = [c.x && 'excluded', c.f && 'computed', c.e && 'error'].filter(Boolean).join(', ');
  return (
    <div className="kv-list" data-testid="cell-info">
      <KV label="Cell" mono>
        {info.address}
      </KV>
      <KV label="Value" mono>
        {value}
      </KV>
      {c.d && c.d !== value && (
        <KV label="Shown as" mono>
          {c.d}
        </KV>
      )}
      {states && <KV label="State">{states}</KV>}
      {c.e && (
        <KV label="Error">
          <span className="text-error">{c.e}</span>
        </KV>
      )}
      {info.cell_formula && (
        <KV label="Cell formula" mono>
          ={info.cell_formula.replace(/^=/, '')}
        </KV>
      )}
      {info.column_formula && (
        <KV label="Column formula" mono>
          ={info.column_formula}
        </KV>
      )}
      <p className="provenance">{info.provenance}</p>
      {info.edits.length > 0 && (
        <details className="edits">
          <summary>
            {info.edits.length} hand {info.edits.length === 1 ? 'edit' : 'edits'}
          </summary>
          <ol>
            {info.edits
              .slice(-8)
              .reverse()
              .map((e, i) => (
                <li key={i}>
                  <span className="mono">{String(e.from ?? 'empty')}</span> to <span className="mono">{String(e.to ?? 'empty')}</span>
                  <span className="muted"> at {e.at}</span>
                </li>
              ))}
          </ol>
        </details>
      )}
    </div>
  );
}

function ColumnSection({ ctx }: { ctx: InspectorContext }) {
  const meta = useTableMeta(ctx.tableId);
  const sel = useSelection(ctx.tableId);
  if (!meta || !ctx.tableId) return null;
  const col = meta.columns[sel.anchor.col];
  if (!col) return <p className="muted">No column selected.</p>;
  const id = ctx.tableId;
  const ro = meta.read_only;
  const ds = col.dataset;
  const format: SubcolumnFormat | undefined = col.format;
  return (
    <div className="kv-list" data-testid="column-props">
      <KV label="Column" mono>
        {col.label}
      </KV>
      {ds !== null ? (
        <CommitField
          label={meta.kind === 'variables' ? 'Variable' : 'Data set'}
          value={col.dataset_name}
          disabled={ro}
          testId="insp-dataset-name"
          validate={(v) => (v.trim() ? null : 'A name cannot be empty.')}
          onCommit={(v) => mutate((e) => cmd.setDatasetName(e, id, ds, v.trim()))}
        />
      ) : (
        <KV label="Title">{col.dataset_name}</KV>
      )}
      {meta.kind === 'variables' && ds !== null && (
        <FieldRow label="Type">
          {(lid) => (
            <Select
              labelledBy={lid}
              value={col.var_type ?? 'number'}
              disabled={ro}
              testId="insp-var-type"
              options={VARIABLE_TYPES.map((t) => ({ value: t.value, label: t.label }))}
              onChange={(v) => void mutate((en) => cmd.setVariableType(en, id, ds, v as VariableType))}
            />
          )}
        </FieldRow>
      )}
      {format && kindInfo(meta.kind)?.formats && (
        <FieldRow label="Subcolumns">
          {(lid) => (
            <Select
              labelledBy={lid}
              value={format.kind}
              disabled={ro}
              testId="insp-subcolumns"
              tipText="Applies to every data set of the table"
              options={FORMAT_CHOICES.map((f) => ({ value: f.value, label: f.label }))}
              onChange={(k) => {
                const f = k === 'values' ? { kind: 'values' as const, n: 3 } : ({ kind: k } as SubcolumnFormat);
                void mutate((en) => cmd.setSubcolumnFormat(en, id, f));
              }}
            />
          )}
        </FieldRow>
      )}
      {format?.kind === 'values' && kindInfo(meta.kind)?.formats && (
        <FieldRow label="Replicates">
          {(lid) => (
            <NumberField
              labelledBy={lid}
              value={format.n}
              min={1}
              max={100}
              integer
              nullable={false}
              disabled={ro}
              testId="insp-replicates"
              onCommit={(v) => v !== null && void mutate((e) => cmd.setSubcolumnFormat(e, id, { kind: 'values', n: v }))}
            />
          )}
        </FieldRow>
      )}
      {format && !kindInfo(meta.kind)?.formats && <KV label="Format">{formatLabel(format)}</KV>}
      <FieldRow label="Decimals">
        {(lid) => (
          <NumberField
            labelledBy={lid}
            value={col.decimals ?? null}
            min={0}
            max={15}
            integer
            placeholder="Automatic"
            disabled={ro}
            testId="insp-decimals"
            onCommit={(v) => void mutate((e) => cmd.setColumnDecimals(e, id, col.index, v))}
          />
        )}
      </FieldRow>
      <CommitField
        label="Formula"
        value={col.formula ? `=${col.formula}` : ''}
        placeholder="None"
        mono
        disabled={ro}
        testId="insp-formula"
        onCommit={(v) => {
          const t = v.trim().replace(/^=\s*/, '').trim();
          return mutate((e) => cmd.setColumnFormula(e, id, col.index, t === '' ? null : t));
        }}
      />
      <KV label="Source">{describeSource(col.source)}</KV>
      {col.cell_formulas && <KV label="Cell formulas">Some cells carry their own formulas</KV>}
    </div>
  );
}

function TableSection({ ctx }: { ctx: InspectorContext }) {
  const meta = useTableMeta(ctx.tableId);
  if (!meta || !ctx.tableId) return null;
  const id = ctx.tableId;
  const info = kindInfo(meta.kind);
  return (
    <div className="kv-list">
      <CommitField
        label="Title"
        value={meta.title}
        validate={(v) => (v.trim() ? null : 'A title cannot be empty.')}
        onCommit={(v) => mutate((e) => cmd.renameObject(e, id, v.trim()))}
      />
      <KV label="Kind">{info?.name ?? meta.kind}</KV>
      {info && <p className="muted small">{info.sentence}</p>}
      <KV label="Rows" mono>
        {meta.rows.toLocaleString('en-US')}
      </KV>
      <KV label={meta.kind === 'variables' ? 'Variables' : 'Data sets'} mono>
        {meta.datasets.length.toLocaleString('en-US')}
      </KV>
      <KV label="Columns" mono>
        {meta.columns.length.toLocaleString('en-US')}
      </KV>
      {meta.read_only && <KV label="Read-only">Derived from another table</KV>}
    </div>
  );
}

function HistorySection() {
  const entries = useHistory((s) => s.entries);
  const view = useProject((s) => s.view);
  useEffect(() => useHistory.getState().watch(), []);
  return (
    <div>
      <div className="btn-row">
        <Button size="sm" disabled={!view?.can_undo} onClick={() => void undoAction()} testId="insp-undo" shortcut="Mod+Z" tipText="Undo">
          Undo
        </Button>
        <Button size="sm" disabled={!view?.can_redo} onClick={() => void redoAction()} testId="insp-redo" shortcut="Mod+Shift+Z" tipText="Redo">
          Redo
        </Button>
      </div>
      {entries.length === 0 ? (
        <p className="muted">Nothing done yet.</p>
      ) : (
        <ol className="history-list">
          {entries.slice(0, 50).map((e, i) => (
            <li key={i} data-tip={e.at}>
              {e.label}
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

const P_STYLES: { value: PValueStyle; label: string; secondary: string }[] = [
  { value: 'apa', label: 'APA', secondary: 'p = .005' },
  { value: 'nejm', label: 'NEJM', secondary: 'p = 0.005' },
  { value: 'gp', label: 'Asterisks', secondary: '**' },
  { value: 'exact', label: 'Exact', secondary: 'p = 0.0049' },
];

function ProjectSection() {
  const view = useProject((s) => s.view);
  if (!view) return null;
  return (
    <div className="kv-list">
      <KV label="Title">{view.title}</KV>
      {view.path && (
        <KV label="File" mono>
          {view.path}
        </KV>
      )}
      <KV label="State">{view.dirty ? 'Unsaved changes' : 'Saved'}</KV>
      <FieldRow label="P values">
        {(lid) => (
          <Select
            labelledBy={lid}
            value={view.p_value_style}
            testId="insp-pstyle"
            options={P_STYLES}
            onChange={(v) => void mutate((en) => cmd.setPValueStyle(en, v))}
          />
        )}
      </FieldRow>
    </div>
  );
}

const isTable = (c: InspectorContext) => !!c.tableId;

export function registerCoreInspectorSections(): void {
  registerInspectorSection({ id: 'cell', title: 'Cell', order: 10, when: isTable, component: CellSection });
  registerInspectorSection({ id: 'column', title: 'Column', order: 20, when: isTable, component: ColumnSection });
  registerInspectorSection({ id: 'table', title: 'Table', order: 30, when: isTable, component: TableSection });
  registerInspectorSection({ id: 'history', title: 'History', order: 90, when: () => true, component: HistorySection, collapsed: true });
  registerInspectorSection({ id: 'project', title: 'Project', order: 100, when: () => true, component: ProjectSection });
}
