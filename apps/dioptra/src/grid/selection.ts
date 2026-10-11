// The grid's selection model: an anchor (the active cell, where editing happens), a focus (the
// moving corner of the active range) and any extra ranges added with Ctrl/Cmd-click.
import type { Range } from '../engine/types';

export interface CellPos {
  row: number;
  col: number;
}

export interface Selection {
  anchor: CellPos;
  focus: CellPos;
  extra: Range[];
}

export const ORIGIN: Selection = { anchor: { row: 0, col: 0 }, focus: { row: 0, col: 0 }, extra: [] };

export function rangeOf(a: CellPos, b: CellPos): Range {
  return {
    row0: Math.min(a.row, b.row),
    row1: Math.max(a.row, b.row) + 1,
    col0: Math.min(a.col, b.col),
    col1: Math.max(a.col, b.col) + 1,
  };
}

/** The active (primary) range. */
export function activeRange(sel: Selection): Range {
  return rangeOf(sel.anchor, sel.focus);
}

/** All selected ranges, active last. */
export function allRanges(sel: Selection): Range[] {
  return [...sel.extra, activeRange(sel)];
}

export function selectCell(pos: CellPos): Selection {
  return { anchor: { ...pos }, focus: { ...pos }, extra: [] };
}

/** Shift-click / Shift-arrow: move the focus corner, keep the anchor. */
export function extendTo(sel: Selection, pos: CellPos): Selection {
  return { ...sel, focus: { ...pos } };
}

/** Ctrl/Cmd-click: keep the current ranges and start a new one at `pos`. */
export function addCell(sel: Selection, pos: CellPos): Selection {
  return { anchor: { ...pos }, focus: { ...pos }, extra: [...sel.extra, activeRange(sel)] };
}

export function selectColumns(c0: number, c1: number, rows: number): Selection {
  const last = Math.max(0, rows - 1);
  return { anchor: { row: 0, col: c0 }, focus: { row: last, col: c1 }, extra: [] };
}

export function selectRows(r0: number, r1: number, cols: number): Selection {
  const last = Math.max(0, cols - 1);
  return { anchor: { row: r0, col: 0 }, focus: { row: r1, col: last }, extra: [] };
}

export function selectAll(rows: number, cols: number): Selection {
  return {
    anchor: { row: 0, col: 0 },
    focus: { row: Math.max(0, rows - 1), col: Math.max(0, cols - 1) },
    extra: [],
  };
}

export function inRange(r: Range, row: number, col: number): boolean {
  return row >= r.row0 && row < r.row1 && col >= r.col0 && col < r.col1;
}

export function contains(sel: Selection, row: number, col: number): boolean {
  return allRanges(sel).some((r) => inRange(r, row, col));
}

export function isSingleCell(sel: Selection): boolean {
  return sel.extra.length === 0 && sel.anchor.row === sel.focus.row && sel.anchor.col === sel.focus.col;
}

export function cellCount(sel: Selection): number {
  return allRanges(sel).reduce((n, r) => n + (r.row1 - r.row0) * (r.col1 - r.col0), 0);
}

/** Clip a selection to a table of `rows` x `cols` (after rows or columns are removed). */
export function clampSelection(sel: Selection, rows: number, cols: number): Selection {
  const c = (p: CellPos): CellPos => ({
    row: Math.max(0, Math.min(p.row, Math.max(0, rows - 1))),
    col: Math.max(0, Math.min(p.col, Math.max(0, cols - 1))),
  });
  return { anchor: c(sel.anchor), focus: c(sel.focus), extra: [] };
}

/** Clip a range to the table's real extent (the grid shows spare rows below the data). */
export function clipRange(r: Range, rows: number, cols: number): Range {
  return {
    row0: Math.min(r.row0, rows),
    row1: Math.min(r.row1, rows),
    col0: Math.min(r.col0, cols),
    col1: Math.min(r.col1, cols),
  };
}

/** Spreadsheet-style address such as `B3` or `B3:D10` (columns by flat index). */
export function columnLetters(index: number): string {
  let s = '';
  let n = index + 1;
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}
