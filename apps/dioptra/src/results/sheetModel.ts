// Pure helpers for showing and copying results tables.
import type { PValueStyle } from '../engine/types';
import type { ResultCell, ResultTable, Sheet } from '../engine/objects';
import { formatPValue, formatSig, stars } from './format';

export type CellKind = 'number' | 'p' | 'text' | 'empty' | 'stars';

export interface DisplayCell {
  kind: CellKind;
  /** What the sheet shows. */
  text: string;
  /** What Mod+C copies: full precision for numbers and p values. */
  raw: string;
}

export interface DisplayTable {
  columns: string[];
  /** Per column: numbers right-aligned. */
  numeric: boolean[];
  rows: DisplayCell[][];
}

export function cellKind(c: ResultCell): Exclude<CellKind, 'stars'> {
  if (c === 'empty' || c === null || c === undefined) return 'empty';
  if ('number' in c) return c.number.v === null || c.number.v === undefined ? 'empty' : 'number';
  if ('p_value' in c) return c.p_value.p === null || c.p_value.p === undefined ? 'empty' : 'p';
  if ('text' in c) return 'text';
  return 'empty';
}

const full = (v: number) => (Object.is(v, -0) ? '0' : String(v));

export function displayCell(c: ResultCell, style: PValueStyle): DisplayCell {
  const k = cellKind(c);
  if (k === 'empty') return { kind: 'empty', text: '', raw: '' };
  if (k === 'text') {
    const t = (c as { text: string }).text;
    return { kind: 'text', text: t, raw: t };
  }
  if (k === 'number') {
    const { v, decimals } = (c as { number: { v: number; decimals?: number } }).number;
    return { kind: 'number', text: formatSig(v, decimals ?? undefined, 4), raw: full(v) };
  }
  const p = (c as { p_value: { p: number } }).p_value.p;
  return { kind: 'p', text: formatPValue(p, style), raw: full(p) };
}

/**
 * A results table as shown: p values formatted by the style; with the GP style an asterisk
 * column ("Summary") follows every column that holds p values.
 */
export function displayTable(t: ResultTable, style: PValueStyle): DisplayTable {
  const ncol = t.columns.length;
  const pCols = new Set<number>();
  for (const r of t.rows) r.forEach((c, i) => cellKind(c) === 'p' && pCols.add(i));
  const columns: string[] = [];
  const numeric: boolean[] = [];
  const map: { src: number; stars: boolean }[] = [];
  for (let i = 0; i < ncol; i++) {
    columns.push(t.columns[i] ?? '');
    const kinds = t.rows.map((r) => (r[i] ? cellKind(r[i]!) : 'empty'));
    numeric.push(kinds.some((k) => k === 'number' || k === 'p') && !kinds.some((k) => k === 'text'));
    map.push({ src: i, stars: false });
    if (style === 'gp' && pCols.has(i)) {
      columns.push('Summary');
      numeric.push(false);
      map.push({ src: i, stars: true });
    }
  }
  const rows = t.rows.map((r) =>
    map.map(({ src, stars: isStars }) => {
      const c = r[src] ?? 'empty';
      if (!isStars) return displayCell(c, style);
      if (cellKind(c) !== 'p') return { kind: 'empty' as const, text: '', raw: '' };
      const s = stars((c as { p_value: { p: number } }).p_value.p);
      return { kind: 'stars' as const, text: s, raw: s };
    }),
  );
  return { columns, numeric, rows };
}

export interface CellRange {
  section: number;
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

export function normRange(section: number, a: { r: number; c: number }, b: { r: number; c: number }): CellRange {
  return {
    section,
    r0: Math.min(a.r, b.r),
    r1: Math.max(a.r, b.r),
    c0: Math.min(a.c, b.c),
    c1: Math.max(a.c, b.c),
  };
}

/** TSV of a range at full precision (inclusive bounds). */
export function rangeTsv(t: DisplayTable, r: CellRange): string {
  const out: string[] = [];
  for (let i = r.r0; i <= r.r1; i++) {
    const row = t.rows[i];
    if (!row) continue;
    const cells: string[] = [];
    for (let j = r.c0; j <= r.c1; j++) cells.push(row[j]?.raw ?? '');
    out.push(cells.join('\t'));
  }
  return out.join('\n');
}

/** Total N for the method line: the sum of a column headed N, if the first section has one. */
export function totalN(sheet: Sheet): number | null {
  const t = sheet.sections[0]?.table;
  if (!t) return null;
  const i = t.columns.findIndex((c) => /^(n|N|Total N|n total)$/.test(c.trim()));
  if (i < 0) return null;
  let n = 0;
  let any = false;
  for (const r of t.rows) {
    const c = r[i];
    if (c && cellKind(c) === 'number') {
      n += (c as { number: { v: number } }).number.v;
      any = true;
    }
  }
  return any ? n : null;
}

export type CheckSummary = { collapsed: boolean; text: string };

/** Assumptions are collapsed when none is in doubt; the summary line says what was found. */
export function summarizeChecks(sheet: Sheet): CheckSummary {
  const a = sheet.assumptions;
  const count = (s: string) => a.filter((x) => x.status === s).length;
  const pass = count('pass');
  const caution = count('caution');
  const fail = count('fail');
  const judge = count('not_checked');
  const parts: string[] = [];
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  if (fail) parts.push(plural(fail, 'check failed', 'checks failed'));
  if (caution) parts.push(plural(caution, 'check in doubt', 'checks in doubt'));
  if (pass) parts.push(plural(pass, 'check passed', 'checks passed'));
  if (judge) parts.push(`${judge} to judge from the design`);
  return { collapsed: fail + caution === 0, text: parts.join(', ') || 'No checks' };
}
