import { describe, expect, it } from 'vitest';
import { dataEdge, navigate, type NavContext } from './navigation';
import { extendTo, selectCell } from './selection';

const ctx: NavContext = { rows: 100, cols: 10, pageRows: 20, dataRows: 50 };
const k = (key: string, shift = false, mod = false) => ({ key, shift, mod });

describe('navigate', () => {
  it('moves with arrows and clamps at the edges', () => {
    const s = selectCell({ row: 0, col: 0 });
    expect(navigate(s, k('ArrowDown'), ctx)!.anchor).toEqual({ row: 1, col: 0 });
    expect(navigate(s, k('ArrowUp'), ctx)!.anchor).toEqual({ row: 0, col: 0 });
    expect(navigate(s, k('ArrowLeft'), ctx)!.anchor).toEqual({ row: 0, col: 0 });
    expect(navigate(selectCell({ row: 0, col: 9 }), k('ArrowRight'), ctx)!.anchor).toEqual({ row: 0, col: 9 });
  });

  it('extends with shift, moving the focus corner only', () => {
    const s = navigate(selectCell({ row: 3, col: 3 }), k('ArrowDown', true), ctx)!;
    expect(s.anchor).toEqual({ row: 3, col: 3 });
    expect(s.focus).toEqual({ row: 4, col: 3 });
    const t = navigate(s, k('ArrowRight', true), ctx)!;
    expect(t.focus).toEqual({ row: 4, col: 4 });
  });

  it('Tab moves right and wraps; Shift-Tab moves left', () => {
    expect(navigate(selectCell({ row: 0, col: 2 }), k('Tab'), ctx)!.anchor).toEqual({ row: 0, col: 3 });
    expect(navigate(selectCell({ row: 0, col: 9 }), k('Tab'), ctx)!.anchor).toEqual({ row: 1, col: 0 });
    expect(navigate(selectCell({ row: 1, col: 0 }), k('Tab', true), ctx)!.anchor).toEqual({ row: 0, col: 9 });
  });

  it('Enter and Shift-Enter move down and up, collapsing a range', () => {
    const range = extendTo(selectCell({ row: 2, col: 1 }), { row: 5, col: 4 });
    const d = navigate(range, k('Enter'), ctx)!;
    expect(d.anchor).toEqual({ row: 3, col: 1 });
    expect(d.focus).toEqual(d.anchor);
    expect(navigate(selectCell({ row: 2, col: 1 }), k('Enter', true), ctx)!.anchor).toEqual({ row: 1, col: 1 });
  });

  it('Home, End, Ctrl-Home, Ctrl-End', () => {
    const s = selectCell({ row: 7, col: 5 });
    expect(navigate(s, k('Home'), ctx)!.anchor).toEqual({ row: 7, col: 0 });
    expect(navigate(s, k('End'), ctx)!.anchor).toEqual({ row: 7, col: 9 });
    expect(navigate(s, k('Home', false, true), ctx)!.anchor).toEqual({ row: 0, col: 0 });
    expect(navigate(s, k('End', false, true), ctx)!.anchor).toEqual({ row: 49, col: 9 });
  });

  it('PageUp and PageDown move by a page', () => {
    expect(navigate(selectCell({ row: 30, col: 2 }), k('PageDown'), ctx)!.anchor).toEqual({ row: 50, col: 2 });
    expect(navigate(selectCell({ row: 10, col: 2 }), k('PageUp'), ctx)!.anchor).toEqual({ row: 0, col: 2 });
    expect(navigate(selectCell({ row: 95, col: 2 }), k('PageDown'), ctx)!.anchor).toEqual({ row: 99, col: 2 });
  });

  it('ignores other keys', () => {
    expect(navigate(selectCell({ row: 0, col: 0 }), k('a'), ctx)).toBeNull();
  });
});

describe('dataEdge (Ctrl-arrows)', () => {
  // Column 0: rows 0..4 filled, 5..9 empty, 10..12 filled, rest empty.
  const filled = (r: number, c: number) => c === 0 && ((r >= 0 && r <= 4) || (r >= 10 && r <= 12));
  const c: NavContext = { rows: 20, cols: 3, pageRows: 5, filled };

  it('runs to the end of a filled block', () => {
    expect(dataEdge({ row: 0, col: 0 }, 1, 0, c)).toEqual({ row: 4, col: 0 });
  });
  it('jumps over empties to the next filled cell', () => {
    expect(dataEdge({ row: 4, col: 0 }, 1, 0, c)).toEqual({ row: 10, col: 0 });
  });
  it('goes to the boundary when nothing is filled ahead', () => {
    expect(dataEdge({ row: 12, col: 0 }, 1, 0, c)).toEqual({ row: 19, col: 0 });
    expect(dataEdge({ row: 0, col: 1 }, 0, 1, c)).toEqual({ row: 0, col: 2 });
  });
  it('stops at the boundary for unknown cells', () => {
    const unknown: NavContext = { ...c, filled: (r) => (r < 3 ? true : undefined) };
    expect(dataEdge({ row: 0, col: 0 }, 1, 0, unknown)).toEqual({ row: 19, col: 0 });
  });
  it('is used by navigate with mod held, and extends with shift', () => {
    const s = navigate(selectCell({ row: 0, col: 0 }), k('ArrowDown', true, true), c)!;
    expect(s.anchor).toEqual({ row: 0, col: 0 });
    expect(s.focus).toEqual({ row: 4, col: 0 });
  });
});
