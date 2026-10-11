// Typed helpers over the bridge: one function per command the UI uses. Every helper takes the
// engine explicitly so stores and tests can pass a fake.
import { call, expectType, type Engine } from './bridge';
import type {
  CellInfo,
  CellView,
  Changed,
  HistoryEntry,
  Id,
  PasteOptions,
  PasteReport,
  ProjectView,
  PValueStyle,
  Range,
  Response,
  SubcolumnFormat,
  TableFormat,
  TableKind,
  TableMeta,
  VariableType,
} from './types';

/** A mutation's effect: `Ok.changed` or `PasteReport.changed`. */
export function changedOf(response: Response): Changed {
  if (response.type === 'ok' || response.type === 'paste_report') return response.changed;
  return { ids: [], tree: false };
}

const strip = <T extends { type: string }>(r: T): Omit<T, 'type'> => {
  const { type: _t, ...rest } = r;
  void _t;
  return rest;
};

export async function ping(e: Engine): Promise<string> {
  return (await expectType(e, { type: 'ping' }, 'pong')).version;
}

// ---- project ----

export async function getProject(e: Engine): Promise<ProjectView> {
  return strip(await expectType(e, { type: 'get_project' }, 'project'));
}
export async function newProject(e: Engine): Promise<Changed> {
  return changedOf(await call(e, { type: 'new_project' }));
}
export async function openProjectBytes(e: Engine, base64: string, name: string | null): Promise<Changed> {
  return changedOf(await call(e, { type: 'open_project_bytes', base64, name }));
}
export async function openProjectPath(e: Engine, path: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'open_project_path', path }));
}
export async function saveProjectBytes(e: Engine): Promise<{ name: string; mime: string; base64: string }> {
  return strip(await expectType(e, { type: 'save_project_bytes' }, 'bytes'));
}
export async function saveProjectPath(e: Engine, path: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'save_project_path', path }));
}
export async function setPValueStyle(e: Engine, style: PValueStyle): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_p_value_style', style }));
}
export async function setProjectTitle(e: Engine, title: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_project_title', title }));
}
export async function renameObject(e: Engine, id: Id, title: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'rename_object', id, title }));
}
export async function deleteObject(e: Engine, id: Id): Promise<Changed> {
  return changedOf(await call(e, { type: 'delete_object', id }));
}

// ---- tables ----

export interface CreateTableArgs {
  kind: TableKind;
  title?: string;
  datasets?: number;
  format?: SubcolumnFormat;
  rows?: number;
}
export async function createTable(e: Engine, a: CreateTableArgs): Promise<Changed> {
  return changedOf(
    await call(e, {
      type: 'create_table',
      kind: a.kind,
      title: a.title ?? '',
      datasets: a.datasets ?? 3,
      format: a.format ?? { kind: 'values', n: 1 },
      rows: a.rows ?? 10,
    }),
  );
}
export async function getTableMeta(e: Engine, table: Id): Promise<TableMeta> {
  return strip(await expectType(e, { type: 'get_table_meta', table }, 'table_meta'));
}
export async function getCells(e: Engine, table: Id, range: Range): Promise<{ range: Range; rows: CellView[][] }> {
  const r = await expectType(e, { type: 'get_cells', table, range }, 'cells');
  return { range: r.range, rows: r.rows };
}
export async function setCell(e: Engine, table: Id, row: number, col: number, input: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_cell', table, row, col, input }));
}
export async function setCells(e: Engine, table: Id, row0: number, col0: number, values: string[][]): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_cells', table, row0, col0, values }));
}
export async function paste(
  e: Engine,
  table: Id,
  row0: number,
  col0: number,
  text: string,
  options: PasteOptions = {},
): Promise<PasteReport> {
  return strip(await expectType(e, { type: 'paste', table, row0, col0, text, options }, 'paste_report'));
}
export async function clearCells(e: Engine, table: Id, range: Range): Promise<Changed> {
  return changedOf(await call(e, { type: 'clear_cells', table, range }));
}
export async function excludeCells(e: Engine, table: Id, range: Range, excluded: boolean): Promise<Changed> {
  return changedOf(await call(e, { type: 'exclude_cells', table, range, excluded }));
}
export async function fillDown(e: Engine, table: Id, range: Range): Promise<Changed> {
  return changedOf(await call(e, { type: 'fill_down', table, range }));
}
export async function insertRows(e: Engine, table: Id, at: number, count: number): Promise<Changed> {
  return changedOf(await call(e, { type: 'insert_rows', table, at, count }));
}
export async function deleteRows(e: Engine, table: Id, at: number, count: number): Promise<Changed> {
  return changedOf(await call(e, { type: 'delete_rows', table, at, count }));
}
export async function addDataset(e: Engine, table: Id, name: string | null = null): Promise<Changed> {
  return changedOf(await call(e, { type: 'add_dataset', table, name }));
}
export async function removeDataset(e: Engine, table: Id, dataset: number): Promise<Changed> {
  return changedOf(await call(e, { type: 'remove_dataset', table, dataset }));
}
export async function setDatasetName(e: Engine, table: Id, dataset: number, name: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_dataset_name', table, dataset, name }));
}
export async function setSubcolumnFormat(e: Engine, table: Id, format: SubcolumnFormat): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_subcolumn_format', table, format }));
}
export async function setColumnFormula(e: Engine, table: Id, col: number, expr: string | null): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_column_formula', table, col, expr }));
}
export async function setColumnDecimals(e: Engine, table: Id, col: number, decimals: number | null): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_column_decimals', table, col, decimals }));
}
export async function setRowTitle(e: Engine, table: Id, row: number, title: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_row_title', table, row, title }));
}
export async function setVariableType(e: Engine, table: Id, dataset: number, varType: VariableType): Promise<Changed> {
  return changedOf(await call(e, { type: 'set_variable_type', table, dataset, var_type: varType }));
}
export async function getCellInfo(e: Engine, table: Id, row: number, col: number): Promise<CellInfo> {
  return strip(await expectType(e, { type: 'get_cell_info', table, row, col }, 'cell_info'));
}

// ---- import / export ----

export async function importText(
  e: Engine,
  text: string,
  kind: TableKind,
  title: string | null = null,
  options: PasteOptions = {},
): Promise<PasteReport> {
  return strip(await expectType(e, { type: 'import_text', text, kind, title, options }, 'paste_report'));
}
export async function importBytes(e: Engine, name: string, base64: string, kind: TableKind | null): Promise<PasteReport> {
  return strip(await expectType(e, { type: 'import_bytes', name, base64, kind }, 'paste_report'));
}
export async function importPath(e: Engine, path: string, kind: TableKind | null): Promise<PasteReport> {
  return strip(await expectType(e, { type: 'import_path', path, kind }, 'paste_report'));
}
export async function exportTable(e: Engine, table: Id, format: TableFormat): Promise<{ name: string; mime: string; base64: string }> {
  return strip(await expectType(e, { type: 'export_table', table, format }, 'bytes'));
}
export async function exportTablePath(e: Engine, table: Id, format: TableFormat, path: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'export_table_path', table, format, path }));
}

// ---- history ----

export async function undo(e: Engine): Promise<Changed> {
  return changedOf(await call(e, { type: 'undo' }));
}
export async function redo(e: Engine): Promise<Changed> {
  return changedOf(await call(e, { type: 'redo' }));
}
export async function getHistory(e: Engine, limit = 200): Promise<HistoryEntry[]> {
  return (await expectType(e, { type: 'get_history', limit }, 'history')).entries;
}
export async function recomputeAll(e: Engine): Promise<Changed> {
  return changedOf(await call(e, { type: 'recompute_all' }));
}
