import { describe, expect, it } from 'vitest';
import type { Sheet } from '../engine/objects';
import { displayTable, rangeTsv, summarizeChecks, totalN } from './sheetModel';

const table = {
  columns: ['Comparison', 'Mean diff.', 'P'],
  rows: [
    [{ text: 'A vs B' }, { number: { v: 1.23456789 } }, { p_value: { p: 0.0049 } }],
    [{ text: 'A vs C' }, { number: { v: 2, decimals: 0 } }, 'empty' as const],
  ],
};

describe('results tables', () => {
  it('formats numbers and p values, numbers right-aligned', () => {
    const t = displayTable(table, 'apa');
    expect(t.columns).toEqual(['Comparison', 'Mean diff.', 'P']);
    expect(t.numeric).toEqual([false, true, true]);
    expect(t.rows[0]!.map((c) => c.text)).toEqual(['A vs B', '1.235', '.005']);
    expect(t.rows[1]!.map((c) => c.text)).toEqual(['A vs C', '2', '']);
  });
  it('adds an asterisk column after p values in the GP style', () => {
    const t = displayTable(table, 'gp');
    expect(t.columns).toEqual(['Comparison', 'Mean diff.', 'P', 'Summary']);
    expect(t.rows[0]!.map((c) => c.text)).toEqual(['A vs B', '1.235', '0.0049', '**']);
    expect(t.rows[1]![3]!.text).toBe('');
  });
  it('copies full precision', () => {
    const t = displayTable(table, 'apa');
    expect(rangeTsv(t, { section: 0, r0: 0, r1: 1, c0: 1, c1: 2 })).toBe('1.23456789\t0.0049\n2\t');
  });
  it('summarizes assumption checks', () => {
    const s = (statuses: string[]): Sheet =>
      ({ assumptions: statuses.map((status) => ({ name: 'x', status, detail: '' })) }) as unknown as Sheet;
    expect(summarizeChecks(s(['pass', 'pass', 'pass']))).toEqual({ collapsed: true, text: '3 checks passed' });
    expect(summarizeChecks(s(['pass', 'caution']))).toEqual({ collapsed: false, text: '1 check in doubt, 1 check passed' });
    expect(summarizeChecks(s(['fail', 'not_checked'])).collapsed).toBe(false);
    expect(summarizeChecks(s(['pass', 'not_checked'])).text).toBe('1 check passed, 1 to judge from the design');
  });
  it('totals N for the method line', () => {
    const sh = { sections: [{ title: '', table: { columns: ['Data set', 'N'], rows: [[{ text: 'a' }, { number: { v: 3, decimals: 0 } }], [{ text: 'b' }, { number: { v: 4, decimals: 0 } }]] } }] } as unknown as Sheet;
    expect(totalN(sh)).toBe(7);
  });
});
