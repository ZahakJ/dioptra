// A client-side parse of pasted text, for the import preview only. The engine does the real
// parse; this shows the user roughly what they are about to get and suggests a table kind.
import type { DecimalMark, Separator, TableKind } from '../engine/types';

export interface Preview {
  separator: Exclude<Separator, 'auto'>;
  decimal: Exclude<DecimalMark, 'auto'>;
  header: boolean;
  rows: string[][];
  totalRows: number;
  cols: number;
}

const NUM_POINT = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;
const NUM_COMMA = /^[+-]?\d+,\d+$/;
const MISSING = new Set(['', '-', '.', 'NA', 'N/A', 'na', 'n/a', 'NaN', 'nan']);

export function detectSeparator(lines: string[]): Exclude<Separator, 'auto'> {
  const sample = lines.slice(0, 20);
  const count = (re: RegExp) => sample.reduce((n, l) => n + (l.match(re)?.length ?? 0), 0);
  if (count(/\t/g) > 0) return 'tab';
  if (count(/;/g) >= sample.length && sample.length > 0) return 'semicolon';
  if (count(/,/g) > 0) return 'comma';
  return 'whitespace';
}

function split(line: string, sep: Exclude<Separator, 'auto'>): string[] {
  if (sep === 'whitespace') return line.trim().split(/\s+/);
  const ch = sep === 'tab' ? '\t' : sep === 'comma' ? ',' : ';';
  // Minimal quote handling for the preview.
  const out: string[] = [];
  let cur = '';
  let q = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i]!;
    if (q) {
      if (c === '"' && line[i + 1] === '"') {
        cur += '"';
        i++;
      } else if (c === '"') q = false;
      else cur += c;
    } else if (c === '"' && cur === '') q = true;
    else if (c === ch) {
      out.push(cur);
      cur = '';
    } else cur += c;
  }
  out.push(cur);
  return out.map((s) => s.trim());
}

export function isNumeric(s: string, decimal: 'point' | 'comma'): boolean {
  return decimal === 'comma' ? NUM_COMMA.test(s) || /^[+-]?\d+$/.test(s) : NUM_POINT.test(s);
}

export function parsePreview(
  text: string,
  opts: { separator?: Separator; decimal?: DecimalMark; header?: boolean | null } = {},
  maxRows = 10,
): Preview {
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  while (lines.length && lines[lines.length - 1]!.trim() === '') lines.pop();
  const separator = !opts.separator || opts.separator === 'auto' ? detectSeparator(lines) : opts.separator;
  const all = lines.map((l) => split(l, separator));
  const cells = all.flat().filter((s) => !MISSING.has(s));
  let decimal: 'point' | 'comma';
  if (opts.decimal && opts.decimal !== 'auto') decimal = opts.decimal;
  else decimal = separator !== 'comma' && cells.length > 0 && cells.some((s) => NUM_COMMA.test(s)) && !cells.some((s) => /^\d+\.\d+$/.test(s)) ? 'comma' : 'point';
  let header: boolean;
  if (opts.header === true || opts.header === false) header = opts.header;
  else {
    const first = all[0] ?? [];
    const rest = all.slice(1, 20).flat().filter((s) => !MISSING.has(s));
    header =
      first.length > 0 &&
      first.some((s) => s !== '' && !isNumeric(s, decimal)) &&
      rest.length > 0 &&
      rest.filter((s) => isNumeric(s, decimal)).length / rest.length > 0.5;
  }
  const cols = all.reduce((m, r) => Math.max(m, r.length), 0);
  return {
    separator,
    decimal,
    header,
    rows: all.slice(0, maxRows + (header ? 1 : 0)),
    totalRows: Math.max(0, all.length - (header ? 1 : 0)),
    cols,
  };
}

/**
 * Suggest a table kind from the data's shape: a text first column with numbers beside it is
 * grouped; an increasing numeric first column is XY; all-numeric columns are a column table;
 * anything with text columns is a variables table (the safe default).
 */
export function suggestKind(p: Preview): { kind: TableKind; why: string } {
  const body = p.rows.slice(p.header ? 1 : 0);
  if (!body.length || p.cols === 0) return { kind: 'variables', why: 'Variables is the safe default.' };
  const colVals = (c: number) => body.map((r) => r[c] ?? '').filter((s) => !MISSING.has(s));
  const numericCol = (c: number) => {
    const v = colVals(c);
    return v.length > 0 && v.every((s) => isNumeric(s, p.decimal));
  };
  const textCols = Array.from({ length: p.cols }, (_, c) => c).filter((c) => !numericCol(c) && colVals(c).length > 0);
  if (textCols.length === 0) {
    const first = colVals(0).map((s) => Number(s.replace(',', '.')));
    const increasing = first.length > 2 && first.every((v, i) => i === 0 || v > first[i - 1]!);
    const headerFirst = p.header ? (p.rows[0]?.[0] ?? '').toLowerCase() : '';
    if (p.cols >= 2 && increasing && (headerFirst === '' || /^(x|time|t|dose|conc|concentration|day|days|hour|hours|min)/.test(headerFirst))) {
      return { kind: 'xy', why: 'The first column increases steadily, like an X axis.' };
    }
    return { kind: 'column', why: 'Every column is numeric, so each is a group of values.' };
  }
  if (textCols.length === 1 && textCols[0] === 0 && p.cols >= 2) {
    return { kind: 'grouped', why: 'The first column holds labels and the rest are numbers.' };
  }
  return { kind: 'variables', why: 'Some columns hold text, so each column is a variable.' };
}
