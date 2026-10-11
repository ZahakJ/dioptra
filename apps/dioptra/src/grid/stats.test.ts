import { describe, expect, it } from 'vitest';
import type { CellView } from '../engine/types';
import { formatStat, summarize } from './stats';

const grid: CellView[][] = [
  [{ v: 2 }, { t: 'x' }],
  [{ v: 4 }, { v: 100, x: true }],
  [{ v: 4 }, {}],
  [{ v: 5 }, { e: 'bad', f: true }],
  [{ v: 5 }, { v: 7 }],
];
const get = (r: number, c: number) => grid[r]?.[c];

describe('selection summary', () => {
  it('counts numeric non-excluded cells with mean, SD and sum', () => {
    const s = summarize([{ row0: 0, row1: 5, col0: 0, col1: 1 }], get);
    expect(s.cells).toBe(5);
    expect(s.n).toBe(5);
    expect(s.sum).toBe(20);
    expect(s.mean).toBeCloseTo(4, 12);
    expect(s.sd).toBeCloseTo(Math.sqrt(1.5), 12);
  });

  it('skips text, excluded, empty and error cells', () => {
    const s = summarize([{ row0: 0, row1: 5, col0: 1, col1: 2 }], get);
    expect(s.cells).toBe(5);
    expect(s.n).toBe(1);
    expect(s.sd).toBeNull();
  });

  it('counts overlapping ranges once', () => {
    const s = summarize(
      [
        { row0: 0, row1: 2, col0: 0, col1: 1 },
        { row0: 1, row1: 3, col0: 0, col1: 1 },
      ],
      get,
    );
    expect(s.cells).toBe(3);
    expect(s.sum).toBe(10);
  });

  it('is accurate for large offsets (Welford)', () => {
    const vals = [1e9 + 4, 1e9 + 7, 1e9 + 13, 1e9 + 16];
    const s = summarize([{ row0: 0, row1: 4, col0: 0, col1: 1 }], (r) => ({ v: vals[r] }));
    expect(s.sd).toBeCloseTo(Math.sqrt(30), 6);
  });

  it('formats compactly', () => {
    expect(formatStat(null)).toBe('–');
    expect(formatStat(2.25)).toBe('2.25');
    expect(formatStat(1 / 3)).toBe('0.333333');
    expect(formatStat(1.5e-7)).toBe('1.5e-7');
  });
});
