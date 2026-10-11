// Plain-English one-line summary of a PasteReport.
import type { PasteReport } from '../engine/types';

const SEP: Record<string, string> = {
  tab: 'tab-separated',
  comma: 'comma-separated',
  semicolon: 'semicolon-separated',
  whitespace: 'space-separated',
  auto: 'separator detected',
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

export function formatPasteReport(r: PasteReport, verb = 'Pasted'): string {
  const parts = [`${verb} ${r.rows} × ${r.cols}`, SEP[r.separator] ?? r.separator];
  parts.push(r.decimal === 'comma' ? 'decimal comma' : 'decimal point');
  if (r.header_detected) parts.push('first row read as a header');
  if (r.text_cells) parts.push(`${plural(r.text_cells, 'text cell')} kept as text`);
  if (r.missing_cells) parts.push(plural(r.missing_cells, 'empty cell'));
  return parts.join('; ');
}
