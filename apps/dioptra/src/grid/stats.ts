// The status line's selection summary, computed from cells in hand.
import type { CellView, Range } from '../engine/types';

export interface SelectionSummary {
  cells: number;
  /** Numeric, non-excluded values. */
  n: number;
  sum: number;
  mean: number | null;
  sd: number | null;
}

/** Summarise numeric, non-excluded cells over ranges (overlaps are counted once). */
export function summarize(ranges: Range[], get: (row: number, col: number) => CellView | undefined): SelectionSummary {
  let n = 0;
  let mean = 0;
  let m2 = 0;
  let sum = 0;
  let cells = 0;
  const seen = ranges.length > 1 ? new Set<string>() : null;
  for (const r of ranges) {
    for (let row = r.row0; row < r.row1; row++) {
      for (let col = r.col0; col < r.col1; col++) {
        if (seen) {
          const k = `${row}:${col}`;
          if (seen.has(k)) continue;
          seen.add(k);
        }
        cells++;
        const c = get(row, col);
        if (!c || c.x || c.e || typeof c.v !== 'number' || !Number.isFinite(c.v)) continue;
        // Welford's update keeps the SD accurate for large offsets.
        n++;
        sum += c.v;
        const d = c.v - mean;
        mean += d / n;
        m2 += d * (c.v - mean);
      }
    }
  }
  return { cells, n, sum, mean: n ? mean : null, sd: n > 1 ? Math.sqrt(m2 / (n - 1)) : null };
}

/** Compact number for the status line: up to 6 significant digits. */
export function formatStat(v: number | null): string {
  if (v === null || !Number.isFinite(v)) return '–';
  if (v === 0) return '0';
  const a = Math.abs(v);
  if (a >= 1e9 || a < 1e-4) return v.toExponential(4).replace(/\.?0+e/, 'e');
  return String(Number(v.toPrecision(6)));
}
