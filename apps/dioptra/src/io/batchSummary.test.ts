import { describe, expect, it } from 'vitest';
import { fileName, formatSummaryCell, pColumns, summarise, summaryCsv, type BatchItem } from './batchSummary';

const item = (name: string, headlines: [string, number | null, boolean?][], error?: string): BatchItem => ({
  name,
  notes: [],
  files: [],
  headlines: headlines.map(([n, value, p]) => ({ name: n, value, p })),
  ...(error ? { error } : {}),
});

describe('batch summary', () => {
  it('has one row per file and one column per headline, in first-seen order', () => {
    const s = summarise([
      item('a.csv', [
        ['Fit · LogEC50', -7],
        ['Fit · Top', 100],
      ]),
      item('b.csv', [
        ['Fit · Top', 98],
        ['Test · P value', 0.01, true],
      ]),
      item('c.csv', [], 'no table called "X"'),
    ]);
    expect(s.columns).toEqual(['File', 'Fit · LogEC50', 'Fit · Top', 'Test · P value', 'Error']);
    expect(s.rows).toEqual([
      ['a.csv', -7, 100, null, null],
      ['b.csv', null, 98, 0.01, null],
      ['c.csv', null, null, null, 'no table called "X"'],
    ]);
  });
  it('writes CSV with quoting and empty cells', () => {
    const s = summarise([item('a, b.csv', [['x', 1.5]]), item('c.csv', [], 'bad "file"')]);
    expect(summaryCsv(s)).toBe('File,x,Error\r\n"a, b.csv",1.5,\r\nc.csv,,"bad ""file"""\r\n');
  });
  it('knows its p-value columns', () => {
    expect([...pColumns([item('a', [['p', 0.2, true], ['t', 2]])])]).toEqual(['p']);
  });
  it('formats numbers to four significant digits', () => {
    expect(formatSummaryCell(1.23456)).toBe('1.235');
    expect(formatSummaryCell(-7.000001)).toBe('-7');
    expect(formatSummaryCell(1e-7)).toBe('1e-7');
    expect(formatSummaryCell(null)).toBe('');
    expect(formatSummaryCell('x')).toBe('x');
    expect(fileName('C:\\data\\plate1.txt')).toBe('plate1.txt');
  });
});
