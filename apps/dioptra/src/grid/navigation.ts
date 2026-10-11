// Keyboard navigation as a pure reducer over the selection.
import { extendTo, selectCell, type CellPos, type Selection } from './selection';

export interface NavKey {
  key: string;
  shift: boolean;
  /** Ctrl on Linux/Windows, Cmd on macOS. */
  mod: boolean;
}

export interface NavContext {
  /** Navigable rows (table rows plus the spare rows the grid shows). */
  rows: number;
  cols: number;
  /** Rows per page for PageUp/PageDown. */
  pageRows: number;
  /** Whether a cell holds data; `undefined` when unknown (not cached). */
  filled?: (row: number, col: number) => boolean | undefined;
  /** Rows that hold data (Ctrl-End target). Defaults to `rows`. */
  dataRows?: number;
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

/**
 * Ctrl-arrow: from `pos`, move in (dr, dc) to the edge of the data, like a spreadsheet:
 * inside a run of filled cells go to its last cell; otherwise jump to the next filled cell
 * (or the boundary when there is none). Unknown cells end the walk at the boundary.
 */
export function dataEdge(pos: CellPos, dr: number, dc: number, ctx: NavContext): CellPos {
  const maxR = ctx.rows - 1;
  const maxC = ctx.cols - 1;
  const boundary = { row: dr ? (dr > 0 ? maxR : 0) : pos.row, col: dc ? (dc > 0 ? maxC : 0) : pos.col };
  const filled = ctx.filled;
  if (!filled) return boundary;
  const inside = (r: number, c: number) => r >= 0 && r <= maxR && c >= 0 && c <= maxC;
  let r = pos.row;
  let c = pos.col;
  const nr = r + dr;
  const nc = c + dc;
  if (!inside(nr, nc)) return { row: r, col: c };
  const here = filled(r, c);
  const next = filled(nr, nc);
  if (here === undefined || next === undefined) return boundary;
  let steps = 0;
  if (here && next) {
    // Run to the last filled cell.
    while (inside(r + dr, c + dc) && steps++ < 2_000_000) {
      const f = filled(r + dr, c + dc);
      if (f === undefined) return boundary;
      if (!f) break;
      r += dr;
      c += dc;
    }
    return { row: r, col: c };
  }
  // Skip empties to the next filled cell.
  while (inside(r + dr, c + dc) && steps++ < 2_000_000) {
    r += dr;
    c += dc;
    const f = filled(r, c);
    if (f === undefined) return boundary;
    if (f) return { row: r, col: c };
  }
  return { row: r, col: c };
}

/**
 * Apply one navigation key. Returns the new selection, or `null` when the key is not a
 * navigation key. Shift extends the active range (moves the focus corner); Tab and Enter
 * always collapse to one cell.
 */
export function navigate(sel: Selection, k: NavKey, ctx: NavContext): Selection | null {
  const maxR = Math.max(0, ctx.rows - 1);
  const maxC = Math.max(0, ctx.cols - 1);
  const from = k.shift ? sel.focus : sel.anchor;
  let to: CellPos;
  let extend = k.shift;
  const step = (dr: number, dc: number): CellPos =>
    k.mod ? dataEdge(from, dr, dc, ctx) : { row: clamp(from.row + dr, 0, maxR), col: clamp(from.col + dc, 0, maxC) };
  switch (k.key) {
    case 'ArrowUp':
      to = step(-1, 0);
      break;
    case 'ArrowDown':
      to = step(1, 0);
      break;
    case 'ArrowLeft':
      to = step(0, -1);
      break;
    case 'ArrowRight':
      to = step(0, 1);
      break;
    case 'Tab': {
      extend = false;
      const a = sel.anchor;
      if (k.shift) {
        to = a.col > 0 ? { row: a.row, col: a.col - 1 } : a.row > 0 ? { row: a.row - 1, col: maxC } : a;
      } else {
        to = a.col < maxC ? { row: a.row, col: a.col + 1 } : a.row < maxR ? { row: a.row + 1, col: 0 } : a;
      }
      break;
    }
    case 'Enter': {
      extend = false;
      const a = sel.anchor;
      to = { row: clamp(a.row + (k.shift ? -1 : 1), 0, maxR), col: a.col };
      break;
    }
    case 'Home':
      to = k.mod ? { row: 0, col: 0 } : { row: from.row, col: 0 };
      break;
    case 'End': {
      const lastData = Math.max(0, Math.min(maxR, (ctx.dataRows ?? ctx.rows) - 1));
      to = k.mod ? { row: lastData, col: maxC } : { row: from.row, col: maxC };
      break;
    }
    case 'PageUp':
      to = { row: clamp(from.row - Math.max(1, ctx.pageRows), 0, maxR), col: from.col };
      break;
    case 'PageDown':
      to = { row: clamp(from.row + Math.max(1, ctx.pageRows), 0, maxR), col: from.col };
      break;
    default:
      return null;
  }
  return extend ? extendTo(sel, to) : selectCell(to);
}
