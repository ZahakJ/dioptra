// Copying the selection: tab-separated values at full precision, quoted like a spreadsheet
// when a text cell contains a tab, newline or quote.
import type { CellView } from '../engine/types';

/** The value a copied cell carries: the number at full precision, else its text. */
export function cellClipboardValue(cell: CellView | undefined): string {
  if (!cell) return '';
  if (cell.e) return '';
  if (typeof cell.v === 'number' && Number.isFinite(cell.v)) return formatFull(cell.v);
  if (cell.t !== undefined) return cell.t;
  return '';
}

/** Shortest decimal that round-trips the double (what JavaScript's `String` gives). */
export function formatFull(v: number): string {
  if (Object.is(v, -0)) return '0';
  return String(v);
}

function quote(s: string): string {
  if (/[\t\n\r"]/.test(s)) return `"${s.replace(/"/g, '""')}"`;
  return s;
}

/** Rows of cells to TSV (no trailing newline). */
export function toTsv(rows: (CellView | undefined)[][]): string {
  return rows.map((r) => r.map((c) => quote(cellClipboardValue(c))).join('\t')).join('\n');
}

/** TSV of the display strings (`d`), for pasting into places that want what is shown. */
export function toDisplayTsv(rows: (CellView | undefined)[][]): string {
  return rows.map((r) => r.map((c) => quote(c?.d ?? c?.t ?? '')).join('\t')).join('\n');
}
