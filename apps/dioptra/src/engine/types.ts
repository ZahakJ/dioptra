// Mirrors crates/dioptra-core/src/api.rs. Keep in sync by hand; the serde tagging is
// `{"type": "snake_case_variant", ...fields}` for Command and Response.
export type Id = string;

export type TableKind =
  | 'xy' | 'column' | 'grouped' | 'contingency' | 'survival' | 'parts' | 'variables' | 'nested';

export type SubcolumnFormat =
  | { kind: 'values'; n: number }
  | { kind: 'mean_sd_n' } | { kind: 'mean_sem_n' } | { kind: 'mean_ci_n' }
  | { kind: 'mean_sd' } | { kind: 'mean_sem' } | { kind: 'median_iqr' }
  | { kind: 'lower_upper' } | { kind: 'single' };

export interface Range { row0: number; row1: number; col0: number; col1: number }

export interface PasteOptions {
  decimal: 'auto' | 'point' | 'comma';
  separator: 'auto' | 'tab' | 'comma' | 'semicolon' | 'whitespace';
  header: boolean | null;
}

export interface CellView { v?: number; t?: string; d?: string; x?: boolean; f?: boolean; e?: string }

export interface Changed { ids: Id[]; tree: boolean; created?: Id }

// The command union is deliberately loose at this layer: `type` plus fields. Typed helpers
// live in engine/commands.ts.
export type Command = { type: string } & Record<string, unknown>;
export type Response = { type: string } & Record<string, unknown>;
