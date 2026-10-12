// Typed helpers for the Phase 5B commands: analyses, results sheets, graphs, layouts and
// transforms. The shapes mirror crates/dioptra-core/src/{analysis,results}.rs and
// crates/dioptra-plot/src/spec.rs; the opaque `JsonObject` types of types.ts are narrowed here.
import { call, expectType, type Engine } from './bridge';
import { changedOf } from './commands';
import type {
  AnalysisOption,
  Changed,
  GraphFormat,
  Id,
  JsonObject,
  JsonValue,
  Range,
  SizeMm,
  Target,
} from './types';

// ---- results sheets (results.rs) ----------------------------------------------------------

export type CheckStatus = 'pass' | 'caution' | 'fail' | 'not_checked';

export interface AssumptionCheck {
  name: string;
  status: CheckStatus;
  detail: string;
  /** What to do instead when the check is in doubt or fails. */
  alternative?: string;
}

/** Externally tagged: `{"number": {...}}`, `{"p_value": {...}}`, `{"text": "..."}`, `"empty"`. */
export type ResultCell =
  | { number: { v: number | null; decimals?: number } }
  | { p_value: { p: number | null } }
  | { text: string }
  | 'empty';

export interface ResultTable {
  columns: string[];
  rows: ResultCell[][];
}

export interface ResultSection {
  title: string;
  table: ResultTable;
}

export interface Sheet {
  title: string;
  analysis_kind: string;
  message?: string;
  sections: ResultSection[];
  assumptions: AssumptionCheck[];
  summary: string[];
  checklist: string[];
  stale: boolean;
  /** A methods sentence ready to paste (test, sidedness, correction, CI level, version). */
  methods?: string;
  /** The family a graph of this analysis gets by default. */
  graph_family?: string;
}

/** An analysis spec: `{"kind": "t_test", ...options}`. */
export type Spec = { kind: string } & JsonObject;

// ---- graph spec (dioptra-plot spec.rs) -------------------------------------------------------

export interface AxisTicks {
  major?: number;
  minor?: number;
  direction?: 'out' | 'in' | 'cross';
  length_pt?: number;
  format?: 'auto' | 'decimal' | 'scientific' | 'power';
  decimals?: number;
  label_angle?: number;
}

export interface AxisSpec {
  title?: string;
  scale?: 'linear' | 'log10' | 'log2' | 'categorical';
  min?: number;
  max?: number;
  breaks?: { from: number; to: number }[];
  ticks?: AxisTicks;
  hidden?: boolean;
}

export interface DatasetStyle {
  dataset: number;
  colour?: string;
  marker?: string;
  marker_pt?: number;
  line_style?: 'solid' | 'dashed' | 'dotted' | 'dash_dot' | 'none';
  line_pt?: number;
  label?: string;
  pattern?: 'none' | 'hatch' | 'back_hatch' | 'crosshatch' | 'horizontal' | 'vertical' | 'dots';
}

export type Annotation =
  | { kind: 'significance_bracket'; from: number; to: number; text: string; y?: number }
  | { kind: 'text_label'; text: string; x: number; y: number; size_pt?: number }
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; dashed?: boolean }
  | { kind: 'reference_line'; axis: 'x' | 'y'; value: number; label?: string; dashed?: boolean };

export type LegendPosition =
  | 'auto'
  | 'hidden'
  | 'right'
  | 'top'
  | 'bottom'
  | 'inside_top_left'
  | 'inside_top_right'
  | 'inside_bottom_left'
  | 'inside_bottom_right';

export interface GraphSpecT {
  source: Id;
  family: string;
  title?: string;
  mapping: {
    datasets?: number[];
    x?: number;
    y?: number[];
    label?: number;
    error_bars: 'none' | 'sd' | 'sem' | 'ci95' | 'range' | 'iqr';
    points: boolean;
  };
  style: {
    theme: string;
    palette?: string;
    size_mm: SizeMm;
    font_pt?: number;
    line_pt?: number;
    marker_pt?: number;
    transparent?: boolean;
    datasets?: DatasetStyle[];
  };
  axes: {
    x: AxisSpec;
    y: AxisSpec;
    frame?: 'l' | 'box' | 'none';
    origin?: 'frame' | 'zero';
    grid?: boolean;
    offset_pt?: number;
  };
  legend: { position: LegendPosition; title?: string; at?: { x: number; y: number } };
  options: JsonObject;
  annotations?: Annotation[];
}

export interface Rendered {
  svg: string;
  width_mm: number;
  height_mm: number;
  targets: Target[];
}

export interface Bytes {
  name: string;
  mime: string;
  base64: string;
}

// ---- transforms (transform.rs) --------------------------------------------------------------

export type TransformOpT =
  | { op: 'transpose' }
  | { op: 'pivot_long_to_wide'; id_col: number; key_col: number; value_col: number }
  | { op: 'pivot_wide_to_long'; id_col: number | null }
  | { op: 'filter_rows'; predicate: string }
  | { op: 'sort'; col: number; descending: boolean }
  | {
      op: 'remove_baseline';
      baseline:
        | { kind: 'first_row' }
        | { kind: 'mean_of_rows'; rows: number[] }
        | { kind: 'column'; col: number }
        | { kind: 'row_title'; title: string };
      mode: 'subtract' | 'divide' | 'percent';
    }
  | { op: 'fraction_of_total'; by: 'row' | 'column' | 'table' }
  | { op: 'prune'; keep_every: number; offset: number }
  | { op: 'average_replicates' }
  | { op: 'select_rows'; start: number; end: number }
  | { op: 'select_columns'; columns: number[] }
  | { op: 'stack'; columns: number[] }
  | { op: 'normalize'; lo: NormRef; hi: NormRef; as_percent: boolean };

export type NormRef =
  | { kind: 'min' }
  | { kind: 'max' }
  | { kind: 'first_row' }
  | { kind: 'last_row' }
  | { kind: 'value'; value: number };

const strip = <T extends { type: string }>(r: T): Omit<T, 'type'> => {
  const { type: _t, ...rest } = r;
  void _t;
  return rest;
};

// ---- analyses ----

export async function listAnalyses(e: Engine, table: Id, range: Range | null): Promise<AnalysisOption[]> {
  return (await expectType(e, { type: 'list_analyses', table, range }, 'analyses')).analyses;
}
export async function defaultAnalysisSpec(e: Engine, table: Id, kind: string): Promise<Spec> {
  return strip(await expectType(e, { type: 'default_analysis_spec', table, kind }, 'analysis_spec')) as Spec;
}
export async function createAnalysis(e: Engine, table: Id, spec: Spec): Promise<Changed> {
  return changedOf(await call(e, { type: 'create_analysis', table, spec }));
}
export async function updateAnalysis(e: Engine, id: Id, spec: Spec): Promise<Changed> {
  return changedOf(await call(e, { type: 'update_analysis', id, spec }));
}
export async function getAnalysisSpec(e: Engine, id: Id): Promise<Spec> {
  return strip(await expectType(e, { type: 'get_analysis_spec', id }, 'analysis_spec')) as Spec;
}
export async function getResults(e: Engine, id: Id): Promise<Sheet> {
  return strip(await expectType(e, { type: 'get_results', id }, 'results')) as unknown as Sheet;
}
export async function exportResults(e: Engine, id: Id, format: 'tsv' | 'markdown'): Promise<string> {
  return (await expectType(e, { type: 'export_results', id, format }, 'text')).text;
}

// ---- graphs ----

export async function createGraph(e: Engine, source: Id, family: string, title: string | null = null): Promise<Changed> {
  return changedOf(await call(e, { type: 'create_graph', source, family, title }));
}
export async function getGraphSpec(e: Engine, id: Id): Promise<GraphSpecT> {
  return strip(await expectType(e, { type: 'get_graph_spec', id }, 'graph_spec')) as unknown as GraphSpecT;
}
export async function updateGraph(e: Engine, id: Id, spec: GraphSpecT): Promise<Changed> {
  return changedOf(await call(e, { type: 'update_graph', id, spec: spec as unknown as JsonObject }));
}
/** `coalesce`: merge into the previous undo step when it patched the same fields (typing). */
export async function patchGraph(e: Engine, id: Id, patch: JsonValue, coalesce = false): Promise<Changed> {
  return changedOf(await call(e, { type: 'patch_graph', id, patch, coalesce }));
}
export async function renderGraph(e: Engine, id: Id): Promise<Rendered> {
  return strip(await expectType(e, { type: 'render_graph', id }, 'svg'));
}
export async function exportGraph(e: Engine, id: Id, format: GraphFormat, size: SizeMm | null): Promise<Bytes> {
  return strip(await expectType(e, { type: 'export_graph', id, format, size }, 'bytes'));
}
export async function exportGraphPath(
  e: Engine,
  id: Id,
  format: GraphFormat,
  size: SizeMm | null,
  path: string,
): Promise<Changed> {
  return changedOf(await call(e, { type: 'export_graph_path', id, format, size, path }));
}

// ---- layouts ----

export async function createLayout(e: Engine, title: string | null, graphs: Id[], columns: number): Promise<Changed> {
  return changedOf(await call(e, { type: 'create_layout', title, graphs, columns }));
}
export async function renderLayout(e: Engine, id: Id): Promise<Rendered> {
  return strip(await expectType(e, { type: 'render_layout', id }, 'svg'));
}
export async function exportLayout(e: Engine, id: Id, format: GraphFormat): Promise<Bytes> {
  return strip(await expectType(e, { type: 'export_layout', id, format }, 'bytes'));
}
export async function exportLayoutPath(e: Engine, id: Id, format: GraphFormat, path: string): Promise<Changed> {
  return changedOf(await call(e, { type: 'export_layout_path', id, format, path }));
}
export interface LayoutInfo {
  id: Id;
  title: string;
  graphs: Id[];
  columns: number;
}
export async function getLayout(e: Engine, id: Id): Promise<LayoutInfo> {
  return strip(await expectType(e, { type: 'get_layout', id }, 'layout'));
}
export async function updateLayout(e: Engine, id: Id, graphs: Id[], columns: number): Promise<Changed> {
  return changedOf(await call(e, { type: 'update_layout', id, graphs, columns }));
}

// ---- transforms ----

export async function transform(e: Engine, table: Id, op: TransformOpT): Promise<Changed> {
  return changedOf(await call(e, { type: 'transform', table, op: op as unknown as { op: string } & JsonObject }));
}
