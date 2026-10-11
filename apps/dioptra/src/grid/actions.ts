// Operations on the active table's selection, shared by keyboard commands, menus, the palette
// and the context menus.
import * as cmd from '../engine/commands';
import type { CellView, Id, Range, SubcolumnFormat, TableMeta, VariableType } from '../engine/types';
import { chooseDialog, promptDialog } from '../app/dialogs';
import { formatPasteReport } from '../io/report';
import { notify, notifyError, useNotices } from '../store/notices';
import { useProject } from '../store/project';
import { mutate, mutateReport } from '../store/sync';
import { cellCache, useTables } from '../store/table';
import { toTsv } from './clipboard';
import { activeRange, allRanges, clipRange, selectAll, type Selection } from './selection';
import { selectionOf, useGrid } from './store';

export function activeTableId(): Id | null {
  const { view, activeId } = useProject.getState();
  const item = view?.items.find((i) => i.id === activeId);
  return item?.kind === 'table' ? item.id : null;
}

export function activeMeta(): TableMeta | undefined {
  const id = activeTableId();
  return id ? useTables.getState().metas[id] : undefined;
}

export function hasActiveTable(): boolean {
  return !!activeMeta();
}

export function isEditableTable(): boolean {
  const m = activeMeta();
  return !!m && !m.read_only;
}

interface Ctx {
  id: Id;
  meta: TableMeta;
  sel: Selection;
}

function ctx(): Ctx | null {
  const meta = activeMeta();
  if (!meta) return null;
  return { id: meta.id, meta, sel: selectionOf(meta.id) };
}

/** Selected ranges clipped to the table's real rows and columns (empty ranges dropped). */
function dataRanges(c: Ctx): Range[] {
  return allRanges(c.sel)
    .map((r) => clipRange(r, c.meta.rows, c.meta.columns.length))
    .filter((r) => r.row1 > r.row0 && r.col1 > r.col0);
}

function readOnlyNotice(c: Ctx): boolean {
  if (c.meta.read_only) {
    notify('This table is derived from another table and is read-only. Edit the source instead.');
    return true;
  }
  return false;
}

export async function clearSelection(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  for (const r of dataRanges(c)) await mutate((e) => cmd.clearCells(e, c.id, r));
}

export async function fillDownSelection(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const r = clipRange(activeRange(c.sel), c.meta.rows, c.meta.columns.length);
  if (r.row1 - r.row0 < 2 || r.col1 <= r.col0) {
    notify('Select the cell to copy and the cells below it, then fill down.');
    return;
  }
  await mutate((e) => cmd.fillDown(e, c.id, r));
}

/** Read cells from the cache when it covers the range, else fetch them. */
export async function cellsOf(id: Id, r: Range): Promise<(CellView | undefined)[][]> {
  if (cellCache.covers(id, r)) {
    const rows: (CellView | undefined)[][] = [];
    for (let row = r.row0; row < r.row1; row++) {
      const line: (CellView | undefined)[] = [];
      for (let col = r.col0; col < r.col1; col++) line.push(cellCache.get(id, row, col));
      rows.push(line);
    }
    return rows;
  }
  return useTables.getState().fetchRange(id, r);
}

export async function toggleExclude(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const ranges = dataRanges(c);
  let allExcluded = true;
  let any = false;
  for (const r of ranges) {
    for (const row of await cellsOf(c.id, r)) {
      for (const cell of row) {
        if (!cell || (cell.v === undefined && cell.t === undefined)) continue;
        any = true;
        if (!cell.x) allExcluded = false;
      }
    }
  }
  const excluded = !(any && allExcluded);
  for (const r of ranges) await mutate((e) => cmd.excludeCells(e, c.id, r, excluded));
}

/** The active range as TSV, synchronously when the cache covers it. */
export function selectionTsvSync(): string | null {
  const c = ctx();
  if (!c) return null;
  const r = clipRange(activeRange(c.sel), c.meta.rows, c.meta.columns.length);
  if (r.row1 <= r.row0 || r.col1 <= r.col0) return '';
  if (!cellCache.covers(c.id, r)) return null;
  const rows: (CellView | undefined)[][] = [];
  for (let row = r.row0; row < r.row1; row++) {
    const line: (CellView | undefined)[] = [];
    for (let col = r.col0; col < r.col1; col++) line.push(cellCache.get(c.id, row, col));
    rows.push(line);
  }
  return toTsv(rows);
}

const MAX_COPY_CELLS = 2_000_000;

export async function selectionTsv(): Promise<string> {
  const sync = selectionTsvSync();
  if (sync !== null) return sync;
  const c = ctx();
  if (!c) return '';
  const r = clipRange(activeRange(c.sel), c.meta.rows, c.meta.columns.length);
  if ((r.row1 - r.row0) * (r.col1 - r.col0) > MAX_COPY_CELLS) {
    throw new Error('The selection is too large to copy; export the table instead.');
  }
  return toTsv(await useTables.getState().fetchRange(c.id, r));
}

async function writeClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    throw new Error('The clipboard is not available here. Use Ctrl+C with the grid focused.');
  }
}

/** Menu/palette copy (keyboard copy goes through the grid's copy event). */
export async function copySelection(): Promise<void> {
  try {
    await writeClipboard(await selectionTsv());
  } catch (err) {
    notifyError(err);
  }
}

export async function cutSelection(): Promise<void> {
  await copySelection();
  await clearSelection();
}

/** Paste delimited text at the active cell and report what the engine decided. */
export async function pasteText(text: string): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const { row, col } = c.sel.anchor;
  const report = await mutateReport((e) => cmd.paste(e, c.id, row, col, text, {}));
  if (!report) return;
  if (report.rows === 0 && report.cols === 0) {
    notify(report.notes[0] ?? 'There was nothing to paste.');
    return;
  }
  // Select what was pasted.
  const sel: Selection = {
    anchor: { row, col },
    focus: { row: row + Math.max(0, report.rows - 1), col: col + Math.max(0, report.cols - 1) },
    extra: [],
  };
  useGrid.getState().setSelection(c.id, sel);
  useNotices.getState().push({
    kind: 'report',
    text: formatPasteReport(report),
    detail: report.notes,
    action: { label: 'Undo', run: () => undoAction() },
  });
}

export async function pasteFromClipboard(): Promise<void> {
  let text: string;
  try {
    text = await navigator.clipboard.readText();
  } catch {
    notify('The clipboard cannot be read from the menu here. Press Ctrl+V with the grid focused.');
    return;
  }
  await pasteText(text);
}

export async function undoAction(): Promise<void> {
  await mutate(cmd.undo);
}

export async function redoAction(): Promise<void> {
  await mutate(cmd.redo);
}

export function selectAllCells(): void {
  const c = ctx();
  if (!c) return;
  useGrid.getState().setSelection(c.id, selectAll(Math.max(1, c.meta.rows), c.meta.columns.length));
}

export async function insertRows(where: 'above' | 'below'): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const r = activeRange(c.sel);
  const count = Math.max(1, r.row1 - r.row0);
  const at = Math.min(where === 'above' ? r.row0 : r.row1, c.meta.rows);
  await mutate((e) => cmd.insertRows(e, c.id, at, count));
}

export async function deleteRows(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const r = clipRange(activeRange(c.sel), c.meta.rows, c.meta.columns.length);
  if (r.row1 <= r.row0) return;
  await mutate((e) => cmd.deleteRows(e, c.id, r.row0, r.row1 - r.row0));
}

export async function addDataset(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  await mutate((e) => cmd.addDataset(e, c.id, null));
}

/** Dataset of the active column, or the given one. */
function datasetOf(c: Ctx, col = c.sel.anchor.col): number | null {
  return c.meta.columns[col]?.dataset ?? null;
}

export async function renameDataset(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const d = datasetOf(c, col);
  if (d === null) {
    notify('The X column is named in the inspector.');
    return;
  }
  const ds = c.meta.datasets[d]!;
  const name = await promptDialog({
    title: c.meta.kind === 'variables' ? 'Rename variable' : 'Rename data set',
    label: 'Name',
    initial: ds.name,
    confirm: 'Rename',
    validate: (v) => (v.trim() ? null : 'A name cannot be empty.'),
  });
  if (name === null || name === ds.name) return;
  await mutate((e) => cmd.setDatasetName(e, c.id, d, name.trim()));
}

export async function removeDataset(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const d = datasetOf(c, col);
  if (d === null) return;
  await mutate((e) => cmd.removeDataset(e, c.id, d));
}

export async function setDecimals(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const cols = col !== undefined ? [col] : columnsOfSelection(c);
  const first = c.meta.columns[cols[0]!];
  const v = await promptDialog({
    title: cols.length > 1 ? `Decimals for ${cols.length} columns` : `Decimals for ${first?.label ?? 'column'}`,
    label: 'Decimal places (empty for automatic)',
    initial: first?.decimals !== undefined ? String(first.decimals) : '',
    hint: 'Display only: values keep full precision.',
    confirm: 'Set',
    validate: (s) => (s.trim() === '' || /^\d{1,2}$/.test(s.trim()) && Number(s) <= 15 ? null : 'A whole number from 0 to 15.'),
  });
  if (v === null) return;
  const decimals = v.trim() === '' ? null : Number(v.trim());
  for (const col of cols) await mutate((e) => cmd.setColumnDecimals(e, c.id, col, decimals));
}

function columnsOfSelection(c: Ctx): number[] {
  const r = clipRange(activeRange(c.sel), c.meta.rows, c.meta.columns.length);
  const out: number[] = [];
  for (let col = r.col0; col < Math.max(r.col1, r.col0 + 1) && col < c.meta.columns.length; col++) out.push(col);
  return out;
}

export async function setColumnFormula(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const k = col ?? c.sel.anchor.col;
  const meta = c.meta.columns[k];
  if (!meta) return;
  const expr = await promptDialog({
    title: `Formula for column ${meta.label}`,
    label: 'Formula',
    initial: meta.formula ? `=${meta.formula}` : '=',
    hint: 'Fills every row, e.g. =log10(A) or =(A - mean(A)) / sd(A). Empty clears it.',
    mono: true,
    confirm: 'Set formula',
  });
  if (expr === null) return;
  const t = expr.trim().replace(/^=\s*/, '').trim();
  await mutate((e) => cmd.setColumnFormula(e, c.id, k, t === '' ? null : t));
}

export async function clearColumnFormula(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const k = col ?? c.sel.anchor.col;
  await mutate((e) => cmd.setColumnFormula(e, c.id, k, null));
}

export const FORMAT_CHOICES: { value: SubcolumnFormat['kind']; label: string; hint: string }[] = [
  { value: 'values', label: 'Replicate values', hint: 'Each subcolumn holds one replicate.' },
  { value: 'single', label: 'Single value', hint: 'One value per data set and row.' },
  { value: 'mean_sd_n', label: 'Mean, SD, N', hint: 'Summaries entered elsewhere.' },
  { value: 'mean_sem_n', label: 'Mean, SEM, N', hint: '' },
  { value: 'mean_ci_n', label: 'Mean, CI, N', hint: 'Lower and upper confidence limits, N.' },
  { value: 'mean_sd', label: 'Mean, SD', hint: '' },
  { value: 'mean_sem', label: 'Mean, SEM', hint: '' },
  { value: 'median_iqr', label: 'Median, quartiles', hint: 'Median, lower quartile, upper quartile.' },
  { value: 'lower_upper', label: 'Lower, upper', hint: 'A range.' },
];

export function formatLabel(f: SubcolumnFormat | undefined): string {
  if (!f) return '';
  if (f.kind === 'values') return f.n === 1 ? 'Values' : `${f.n} replicates`;
  return FORMAT_CHOICES.find((c) => c.value === f.kind)?.label ?? f.kind;
}

export async function setSubcolumnFormat(): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const current = c.meta.datasets[0]?.format ?? { kind: 'values', n: 1 };
  const kind = await chooseDialog({
    title: 'Subcolumn format',
    label: 'Every data set of this table uses',
    options: FORMAT_CHOICES,
    initial: current.kind,
    confirm: 'Next',
  });
  if (kind === null) return;
  let format: SubcolumnFormat;
  if (kind === 'values') {
    const n = await promptDialog({
      title: 'Replicates',
      label: 'Replicate subcolumns per data set',
      initial: String(current.kind === 'values' ? current.n : 3),
      confirm: 'Set format',
      validate: (s) => (/^\d+$/.test(s.trim()) && Number(s) >= 1 && Number(s) <= 100 ? null : 'A whole number from 1 to 100.'),
    });
    if (n === null) return;
    format = { kind: 'values', n: Number(n.trim()) };
  } else {
    format = { kind } as SubcolumnFormat;
  }
  await mutate((e) => cmd.setSubcolumnFormat(e, c.id, format));
}

export const VARIABLE_TYPES: { value: VariableType; label: string; hint: string }[] = [
  { value: 'number', label: 'Number', hint: 'Continuous measurements.' },
  { value: 'category', label: 'Category', hint: 'Labels or small integer codes for groups.' },
  { value: 'text', label: 'Text', hint: 'Free text, not analysed.' },
  { value: 'date', label: 'Date', hint: 'ISO dates.' },
];

export async function setVariableType(col?: number): Promise<void> {
  const c = ctx();
  if (!c || readOnlyNotice(c)) return;
  const d = datasetOf(c, col);
  if (d === null) return;
  const t = await chooseDialog({
    title: `Type of ${c.meta.datasets[d]!.name}`,
    label: 'Variable type',
    options: VARIABLE_TYPES,
    initial: c.meta.datasets[d]!.var_type ?? 'number',
    confirm: 'Set type',
  });
  if (t === null) return;
  await mutate((e) => cmd.setVariableType(e, c.id, d, t));
}
