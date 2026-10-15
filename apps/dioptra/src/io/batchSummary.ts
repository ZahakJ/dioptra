// Batch analysis results as the UI assembles them, one input at a time: the summary table
// (one row per file, one column per headline number) and its CSV. Mirrors
// `dioptra_lab::batch::summarise` and `summary_csv`.

export interface Headline {
  name: string;
  value: number | null;
  p?: boolean;
}

export interface OutputFile {
  name: string;
  mime: string;
  base64: string;
}

export interface BatchItem {
  name: string;
  error?: string;
  notes: string[];
  headlines: Headline[];
  files: OutputFile[];
  written?: string[];
}

export type SummaryCell = string | number | null;

export interface Summary {
  columns: string[];
  rows: SummaryCell[][];
}

/** `File`, every headline name in first-seen order, `Error`. */
export function summarise(items: BatchItem[]): Summary {
  const names: string[] = [];
  const seen = new Set<string>();
  for (const it of items)
    for (const h of it.headlines)
      if (!seen.has(h.name)) {
        seen.add(h.name);
        names.push(h.name);
      }
  const rows = items.map((it) => {
    const by = new Map(it.headlines.map((h) => [h.name, h.value] as const));
    return [it.name, ...names.map((n) => by.get(n) ?? null), it.error ?? null];
  });
  return { columns: ['File', ...names, 'Error'], rows };
}

/** Whether a summary column holds p values. */
export function pColumns(items: BatchItem[]): Set<string> {
  const out = new Set<string>();
  for (const it of items) for (const h of it.headlines) if (h.p) out.add(h.name);
  return out;
}

function field(s: string): string {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** CSV with full-precision numbers and CRLF line ends. */
export function summaryCsv(s: Summary): string {
  const line = (cells: string[]) => cells.join(',') + '\r\n';
  let out = line(s.columns.map(field));
  for (const r of s.rows) out += line(r.map((v) => (v === null ? '' : typeof v === 'number' ? String(v) : field(v))));
  return out;
}

/** Four significant digits for the on-screen table. */
export function formatSummaryCell(v: SummaryCell): string {
  if (v === null) return '';
  if (typeof v === 'string') return v;
  if (!Number.isFinite(v)) return '';
  if (v === 0) return '0';
  const a = Math.abs(v);
  if (a >= 1e6 || a < 1e-4) return v.toExponential(3).replace(/\.?0+e/, 'e');
  return String(Number(v.toPrecision(4)));
}

/** The name of a data file without its folder. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Extensions a batch reads. */
export const BATCH_EXTENSIONS = ['csv', 'tsv', 'txt', 'xlsx', 'xls', 'ods', 'pzfx', 'prism'];
