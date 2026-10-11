import { describe, expect, it } from 'vitest';
import {
  activeRange,
  addCell,
  allRanges,
  cellCount,
  clampSelection,
  clipRange,
  columnLetters,
  contains,
  extendTo,
  isSingleCell,
  rangeOf,
  selectAll,
  selectCell,
  selectColumns,
  selectRows,
} from './selection';

describe('selection model', () => {
  it('normalises ranges from any two corners', () => {
    expect(rangeOf({ row: 5, col: 3 }, { row: 2, col: 1 })).toEqual({ row0: 2, row1: 6, col0: 1, col1: 4 });
  });

  it('selects a single cell and extends with shift', () => {
    const s = selectCell({ row: 2, col: 2 });
    expect(isSingleCell(s)).toBe(true);
    const e = extendTo(s, { row: 4, col: 0 });
    expect(e.anchor).toEqual({ row: 2, col: 2 });
    expect(activeRange(e)).toEqual({ row0: 2, row1: 5, col0: 0, col1: 3 });
    expect(cellCount(e)).toBe(9);
  });

  it('adds ranges with ctrl-click', () => {
    const s = addCell(extendTo(selectCell({ row: 0, col: 0 }), { row: 1, col: 1 }), { row: 5, col: 5 });
    expect(allRanges(s)).toHaveLength(2);
    expect(contains(s, 1, 1)).toBe(true);
    expect(contains(s, 5, 5)).toBe(true);
    expect(contains(s, 3, 3)).toBe(false);
    expect(isSingleCell(s)).toBe(false);
  });

  it('selects whole columns, rows and everything', () => {
    expect(activeRange(selectColumns(1, 2, 10))).toEqual({ row0: 0, row1: 10, col0: 1, col1: 3 });
    expect(activeRange(selectRows(3, 4, 5))).toEqual({ row0: 3, row1: 5, col0: 0, col1: 5 });
    expect(activeRange(selectAll(7, 3))).toEqual({ row0: 0, row1: 7, col0: 0, col1: 3 });
    expect(activeRange(selectAll(0, 0))).toEqual({ row0: 0, row1: 1, col0: 0, col1: 1 });
  });

  it('clamps and clips to the table', () => {
    const s = clampSelection(extendTo(selectCell({ row: 9, col: 9 }), { row: 20, col: 1 }), 5, 3);
    expect(s.anchor).toEqual({ row: 4, col: 2 });
    expect(s.focus).toEqual({ row: 4, col: 1 });
    expect(clipRange({ row0: 2, row1: 50, col0: 0, col1: 9 }, 10, 4)).toEqual({ row0: 2, row1: 10, col0: 0, col1: 4 });
  });

  it('spells column letters', () => {
    expect(columnLetters(0)).toBe('A');
    expect(columnLetters(25)).toBe('Z');
    expect(columnLetters(26)).toBe('AA');
    expect(columnLetters(701)).toBe('ZZ');
  });
});
