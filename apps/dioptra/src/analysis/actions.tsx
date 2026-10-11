// Analyze (new analysis from the active table and selection) and re-running with new options.
import { openDialog } from '../app/dialogs';
import * as obj from '../engine/objects';
import type { Id, Range, TableMeta } from '../engine/types';
import { activeRange } from '../grid/selection';
import { selectionOf } from '../grid/store';
import { notify } from '../store/notices';
import { itemById, useProject } from '../store/project';
import { mutate, query } from '../store/sync';
import { useTables } from '../store/table';
import { AnalyzeDialog, OptionsStep, kindTitle, type AnalyzeResult } from './AnalyzeDialog';
import { withSelection } from './schema';
import { useState } from 'react';

/** The table an Analyze command applies to: the active table, or the source of the active object. */
export function analyzableTable(): Id | null {
  const { activeId, view } = useProject.getState();
  const item = view?.items.find((i) => i.id === activeId);
  if (!item) return view?.items.find((i) => i.kind === 'table')?.id ?? null;
  if (item.kind === 'table') return item.id;
  if (item.kind === 'analysis' && item.derived_from) return item.derived_from;
  return view?.items.find((i) => i.kind === 'table')?.id ?? null;
}

async function metaOf(id: Id): Promise<TableMeta | undefined> {
  return useTables.getState().metas[id] ?? (await useTables.getState().loadMeta(id));
}

/** Data sets touched by a range (whole table when the selection is one cell). */
function selectedDatasets(meta: TableMeta, r: Range | null): number[] {
  if (!r) return [];
  const out: number[] = [];
  for (let c = r.col0; c < r.col1; c++) {
    const d = meta.columns[c]?.dataset;
    if (d !== null && d !== undefined && !out.includes(d)) out.push(d);
  }
  return out;
}

function scopeText(meta: TableMeta, ds: number[]): string {
  if (!ds.length || ds.length === meta.datasets.length) return `Data: ${meta.title}`;
  const letters = ds.map((d) => meta.datasets[d]?.letter ?? '?');
  return `Data: ${meta.title}, data set${ds.length > 1 ? 's' : ''} ${letters.join(', ')}`;
}

export async function analyze(tableId: Id | null = analyzableTable()): Promise<void> {
  if (!tableId) {
    notify('Create or open a table first; analyses start from data.');
    return;
  }
  const meta = await metaOf(tableId);
  if (!meta) return;
  const sel = selectionOf(tableId);
  const r = activeRange(sel);
  const multi = useProject.getState().activeId === tableId && (r.row1 - r.row0 > 1 || r.col1 - r.col0 > 1);
  const range = multi ? r : null;
  const ds = selectedDatasets(meta, range);
  const options = await query((e) => obj.listAnalyses(e, tableId, range));
  if (!options) return;
  const scope = scopeText(meta, ds);
  const result = await openDialog<AnalyzeResult>(
    (close) => (
      <AnalyzeDialog
        options={options}
        meta={meta}
        scope={scope}
        close={close}
        loadSpec={async (kind) => {
          const s = await query((e) => obj.defaultAnalysisSpec(e, tableId, kind));
          return s ? withSelection(s, meta, ds) : undefined;
        }}
      />
    ),
    null,
  );
  if (!result) return;
  await mutate((e) => obj.createAnalysis(e, tableId, result.spec));
}

/** Reopen the options panel of an analysis and update it. */
export async function editAnalysisOptions(id: Id): Promise<void> {
  const item = itemById(id);
  if (!item || item.kind !== 'analysis') return;
  const spec = await query((e) => obj.getAnalysisSpec(e, id));
  if (!spec) return;
  const tableId = item.derived_from;
  const meta = tableId ? await metaOf(tableId) : undefined;
  const result = await openDialog<AnalyzeResult>(
    (close) => <EditOptions initial={spec} meta={meta} scope={meta ? `Data: ${meta.title}` : ''} close={close} />,
    null,
  );
  if (!result) return;
  await mutate((e) => obj.updateAnalysis(e, id, result.spec));
}

function EditOptions({
  initial,
  meta,
  scope,
  close,
}: {
  initial: obj.Spec;
  meta: TableMeta | undefined;
  scope: string;
  close: (r: AnalyzeResult) => void;
}) {
  const [spec, setSpec] = useState(initial);
  return <OptionsStep spec={spec} meta={meta} scope={scope} onSpec={setSpec} close={close} confirm="Update" name={kindTitle(spec.kind)} />;
}
