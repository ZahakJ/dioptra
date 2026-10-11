// Mirrors crates/dioptra-core/src/api.rs (and the types it re-exports from table.rs,
// import.rs, project.rs, history.rs). Keep in sync by hand.
//
// Serde tagging: `Command` and `Response` are internally tagged, `{"type": "snake_case", ...}`.
// Newtype variants (`Response::Project(ProjectView)` and friends) flatten the inner struct's
// fields next to `type`. Optional Rust fields that are `skip_serializing_if` are optional here.
//
// Types owned by the analysis and graph layers (AnalysisSpec, ResultsSheet, GraphSpec,
// TransformOp) are kept opaque (`JsonObject`) at this layer; Phase 5B narrows them.

export type Id = string;

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

// ---- enums ----------------------------------------------------------------------------

export type TableKind =
  | 'xy'
  | 'column'
  | 'grouped'
  | 'contingency'
  | 'survival'
  | 'parts'
  | 'variables'
  | 'nested';

export const TABLE_KINDS: readonly TableKind[] = [
  'xy',
  'column',
  'grouped',
  'contingency',
  'survival',
  'parts',
  'variables',
  'nested',
];

/** `#[serde(tag = "kind", content = "n")]` */
export type SubcolumnFormat =
  | { kind: 'values'; n: number }
  | { kind: 'mean_sd_n' }
  | { kind: 'mean_sem_n' }
  | { kind: 'mean_ci_n' }
  | { kind: 'mean_sd' }
  | { kind: 'mean_sem' }
  | { kind: 'median_iqr' }
  | { kind: 'lower_upper' }
  | { kind: 'single' };

export type SubcolumnFormatKind = SubcolumnFormat['kind'];

export type VariableType = 'number' | 'category' | 'text' | 'date';

export type DecimalMark = 'auto' | 'point' | 'comma';
export type Separator = 'auto' | 'tab' | 'comma' | 'semicolon' | 'whitespace';

/** Every field is optional in JSON (`{}` = auto). */
export interface PasteOptions {
  decimal?: DecimalMark;
  separator?: Separator;
  header?: boolean | null;
}

/** Half-open on rows and flat columns, zero-based. */
export interface Range {
  row0: number;
  row1: number;
  col0: number;
  col1: number;
}

export type TableFormat = 'csv' | 'tsv' | 'xlsx';

/** `#[serde(tag = "format")]` */
export type GraphFormat = { format: 'svg' } | { format: 'pdf' } | { format: 'png'; dpi: number };

export interface SizeMm {
  width: number;
  height: number;
}

export type PValueStyle = 'apa' | 'nejm' | 'gp' | 'exact';

// ---- table structures (table.rs) --------------------------------------------------------

export type ColumnSource =
  | { kind: 'entered' }
  | { kind: 'pasted'; at: string }
  | { kind: 'imported'; file: string; sheet?: string; column: string }
  | { kind: 'formula'; expr: string }
  | { kind: 'derived'; table: Id; op: string };

/** Untagged: empty is `null`, a number, or text (cell formulas keep their leading `=`). */
export type CellValue = null | number | string;

export interface CellEdit {
  row: number;
  at: string;
  from: CellValue;
  to: CellValue;
}

export type ColumnRole = 'x' | 'y';

export interface ColumnMeta {
  index: number;
  role: ColumnRole;
  dataset: number | null;
  subcolumn: number;
  label: string;
  header: string;
  dataset_name: string;
  format?: SubcolumnFormat;
  formula?: string;
  cell_formulas?: boolean;
  decimals?: number;
  var_type?: VariableType;
  source: ColumnSource;
}

export interface DatasetMeta {
  name: string;
  letter: string;
  format: SubcolumnFormat;
  first_col: number;
  subcolumns: number;
  var_type?: VariableType;
}

export interface TableMeta {
  id: Id;
  title: string;
  kind: TableKind;
  rows: number;
  has_x: boolean;
  has_row_titles: boolean;
  /** Padded to `rows` only when any title is set; otherwise empty. */
  row_titles: string[];
  columns: ColumnMeta[];
  datasets: DatasetMeta[];
  derived_from?: Id;
  read_only: boolean;
}

/** What one cell looks like in a viewport. */
export interface CellView {
  /** Numeric value at full precision. */
  v?: number;
  /** Text content for non-numeric entries. */
  t?: string;
  /** Display string with the column's decimals. */
  d?: string;
  /** Excluded. */
  x?: boolean;
  /** Computed by a formula. */
  f?: boolean;
  /** Formula error message. */
  e?: string;
}

export interface CellInfo {
  table: Id;
  row: number;
  col: number;
  address: string;
  cell: CellView;
  source: ColumnSource;
  column_formula?: string;
  cell_formula?: string;
  edits: CellEdit[];
  provenance: string;
}

// ---- import.rs ---------------------------------------------------------------------------

export interface Changed {
  ids: Id[];
  tree: boolean;
  created?: Id;
}

export interface PasteReport {
  separator: Separator;
  decimal: DecimalMark;
  header_detected: boolean;
  rows: number;
  cols: number;
  numeric_cells: number;
  text_cells: number;
  missing_cells: number;
  notes: string[];
  changed: Changed;
}

// ---- project.rs --------------------------------------------------------------------------

export type ItemKind = 'table' | 'analysis' | 'graph' | 'layout';

export interface ProjectItem {
  id: Id;
  kind: ItemKind;
  title: string;
  derived_from?: Id;
  stale: boolean;
  /** Table kind for tables, analysis kind for analyses, family for graphs. */
  subtype?: string;
}

export interface ProjectView {
  title: string;
  path?: string;
  dirty: boolean;
  p_value_style: PValueStyle;
  items: ProjectItem[];
  can_undo: boolean;
  can_redo: boolean;
}

// ---- history.rs --------------------------------------------------------------------------

export interface HistoryEntry {
  at: string;
  label: string;
  command: JsonValue;
}

// ---- analysis / graph (owned by Phase 5B) -------------------------------------------------

export interface AnalysisOption {
  kind: string;
  name: string;
  fits: boolean;
  reason?: string;
  implemented: boolean;
}

export type AnalysisSpec = JsonObject;
export type ResultsSheet = JsonObject;
export type GraphSpec = JsonObject;
export type TransformOp = { op: string } & JsonObject;

export interface Target {
  id: string;
  target: string;
  bbox: { x: number; y: number; width: number; height: number };
}

// ---- Command -----------------------------------------------------------------------------

export type Command =
  | { type: 'ping' }
  // project
  | { type: 'new_project' }
  | { type: 'open_project_path'; path: string }
  | { type: 'open_project_bytes'; base64: string; name: string | null }
  | { type: 'save_project_path'; path: string }
  | { type: 'save_project_bytes' }
  | { type: 'get_project' }
  | { type: 'set_p_value_style'; style: PValueStyle }
  | { type: 'rename_object'; id: Id; title: string }
  | { type: 'delete_object'; id: Id }
  // tables
  | {
      type: 'create_table';
      kind: TableKind;
      title: string;
      datasets: number;
      format: SubcolumnFormat;
      rows: number;
    }
  | { type: 'get_table_meta'; table: Id }
  | { type: 'get_cells'; table: Id; range: Range }
  | { type: 'set_cell'; table: Id; row: number; col: number; input: string }
  | { type: 'set_cells'; table: Id; row0: number; col0: number; values: string[][] }
  | { type: 'paste'; table: Id; row0: number; col0: number; text: string; options: PasteOptions }
  | { type: 'clear_cells'; table: Id; range: Range }
  | { type: 'exclude_cells'; table: Id; range: Range; excluded: boolean }
  | { type: 'fill_down'; table: Id; range: Range }
  | { type: 'insert_rows'; table: Id; at: number; count: number }
  | { type: 'delete_rows'; table: Id; at: number; count: number }
  | { type: 'add_dataset'; table: Id; name: string | null }
  | { type: 'remove_dataset'; table: Id; dataset: number }
  | { type: 'set_dataset_name'; table: Id; dataset: number; name: string }
  | { type: 'set_subcolumn_format'; table: Id; format: SubcolumnFormat }
  | { type: 'set_column_formula'; table: Id; col: number; expr: string | null }
  | { type: 'set_column_decimals'; table: Id; col: number; decimals: number | null }
  | { type: 'set_row_title'; table: Id; row: number; title: string }
  | { type: 'set_variable_type'; table: Id; dataset: number; var_type: VariableType }
  | { type: 'transform'; table: Id; op: TransformOp }
  | { type: 'get_cell_info'; table: Id; row: number; col: number }
  // import / export
  | { type: 'import_text'; text: string; kind: TableKind; title: string | null; options: PasteOptions }
  | { type: 'import_path'; path: string; kind: TableKind | null }
  | { type: 'import_bytes'; name: string; base64: string; kind: TableKind | null }
  | { type: 'export_table'; table: Id; format: TableFormat }
  | { type: 'export_table_path'; table: Id; format: TableFormat; path: string }
  // analyses
  | { type: 'list_analyses'; table: Id; range: Range | null }
  | { type: 'default_analysis_spec'; table: Id; kind: string }
  | { type: 'create_analysis'; table: Id; spec: AnalysisSpec }
  | { type: 'update_analysis'; id: Id; spec: AnalysisSpec }
  | { type: 'get_analysis_spec'; id: Id }
  | { type: 'get_results'; id: Id }
  | { type: 'export_results'; id: Id; format: 'tsv' | 'markdown' }
  // graphs
  | { type: 'create_graph'; source: Id; family: string; title: string | null }
  | { type: 'get_graph_spec'; id: Id }
  | { type: 'update_graph'; id: Id; spec: GraphSpec }
  | { type: 'patch_graph'; id: Id; patch: JsonValue }
  | { type: 'render_graph'; id: Id }
  | { type: 'export_graph'; id: Id; format: GraphFormat; size: SizeMm | null }
  | { type: 'export_graph_path'; id: Id; format: GraphFormat; size: SizeMm | null; path: string }
  // layouts
  | { type: 'create_layout'; title: string | null; graphs: Id[]; columns: number }
  | { type: 'render_layout'; id: Id }
  | { type: 'export_layout'; id: Id; format: GraphFormat }
  // history
  | { type: 'undo' }
  | { type: 'redo' }
  | { type: 'get_history'; limit: number }
  | { type: 'recompute_all' };

export type CommandType = Command['type'];

// ---- Response ----------------------------------------------------------------------------

export type Response =
  | { type: 'pong'; version: string }
  | { type: 'ok'; changed: Changed }
  | { type: 'error'; message: string }
  | ({ type: 'project' } & ProjectView)
  | ({ type: 'table_meta' } & TableMeta)
  | { type: 'cells'; range: Range; rows: CellView[][] }
  | ({ type: 'paste_report' } & PasteReport)
  | ({ type: 'cell_info' } & CellInfo)
  | { type: 'analyses'; analyses: AnalysisOption[] }
  | ({ type: 'analysis_spec' } & AnalysisSpec)
  | ({ type: 'results' } & ResultsSheet)
  | ({ type: 'graph_spec' } & GraphSpec)
  | { type: 'svg'; svg: string; width_mm: number; height_mm: number; targets: Target[] }
  | { type: 'bytes'; name: string; mime: string; base64: string }
  | { type: 'text'; text: string }
  | { type: 'history'; entries: HistoryEntry[] };

export type ResponseType = Response['type'];
export type ResponseOf<T extends ResponseType> = Extract<Response, { type: T }>;
