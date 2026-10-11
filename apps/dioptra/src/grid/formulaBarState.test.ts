import { describe, expect, it } from 'vitest';
import { commitOf, formulaBarReducer, INITIAL_BAR, rawCellInput, type FormulaBarState } from './formulaBarState';

const sync = (s: FormulaBarState, cellText: string, columnText = '') =>
  formulaBarReducer(s, { type: 'sync', cellText, columnText });

describe('rawCellInput', () => {
  it('shows numbers at full precision, text as typed, cell formulas with =', () => {
    expect(rawCellInput({ v: 2 / 3, d: '0.67' })).toBe('0.6666666666666666');
    expect(rawCellInput({ t: '<LOD', d: '<LOD' })).toBe('<LOD');
    expect(rawCellInput({ v: 6, d: '6', f: true }, 'A3 * 2')).toBe('=A3 * 2');
    expect(rawCellInput({ v: 6, d: '6', f: true }, '=A3 * 2')).toBe('=A3 * 2');
    expect(rawCellInput(undefined)).toBe('');
    expect(rawCellInput({})).toBe('');
  });
});

describe('formula bar reducer', () => {
  it('syncs to the active cell when not editing', () => {
    const s = sync(INITIAL_BAR, '1.5');
    expect(s.text).toBe('1.5');
    expect(commitOf(s)).toBeNull();
  });

  it('keeps the user edit across syncs and commits a cell input', () => {
    let s = sync(INITIAL_BAR, '1.5');
    s = formulaBarReducer(s, { type: 'input', text: '2.5' });
    s = sync(s, '1.5');
    expect(s.text).toBe('2.5');
    expect(commitOf(s)).toEqual({ kind: 'cell', input: '2.5' });
    s = formulaBarReducer(s, { type: 'committed' });
    expect(s.editing).toBe(false);
  });

  it('cancel reverts to the synced value', () => {
    let s = sync(INITIAL_BAR, 'abc');
    s = formulaBarReducer(s, { type: 'input', text: 'xyz' });
    s = formulaBarReducer(s, { type: 'cancel' });
    expect(s.text).toBe('abc');
    expect(s.editing).toBe(false);
  });

  it('column mode shows and commits the column formula without the =', () => {
    let s = sync(INITIAL_BAR, '0.176', '=log10(A)');
    s = formulaBarReducer(s, { type: 'mode', mode: 'column' });
    expect(s.text).toBe('=log10(A)');
    s = formulaBarReducer(s, { type: 'input', text: '= ln(A) ' });
    expect(commitOf(s)).toEqual({ kind: 'column', expr: 'ln(A)' });
  });

  it('an emptied column formula clears it', () => {
    let s = formulaBarReducer(sync(INITIAL_BAR, '', '=A*2'), { type: 'mode', mode: 'column' });
    s = formulaBarReducer(s, { type: 'input', text: '' });
    expect(commitOf(s)).toEqual({ kind: 'column', expr: null });
  });

  it('switching mode drops an uncommitted edit', () => {
    let s = sync(INITIAL_BAR, '1');
    s = formulaBarReducer(s, { type: 'input', text: '99' });
    s = formulaBarReducer(s, { type: 'mode', mode: 'column' });
    s = formulaBarReducer(s, { type: 'mode', mode: 'cell' });
    expect(s.text).toBe('1');
  });
});
