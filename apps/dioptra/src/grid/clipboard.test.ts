import { describe, expect, it } from 'vitest';
import { cellClipboardValue, formatFull, toDisplayTsv, toTsv } from './clipboard';

describe('TSV clipboard serialisation', () => {
  it('writes numbers at full precision, not the display string', () => {
    expect(cellClipboardValue({ v: 0.1 + 0.2, d: '0.30' })).toBe('0.30000000000000004');
    expect(cellClipboardValue({ v: 1 / 3, d: '0.333' })).toBe('0.3333333333333333');
    expect(cellClipboardValue({ v: 1e-12, d: '0' })).toBe('1e-12');
    expect(formatFull(-0)).toBe('0');
  });

  it('keeps text and writes empty for empties and errors', () => {
    expect(cellClipboardValue({ t: 'Control', d: 'Control' })).toBe('Control');
    expect(cellClipboardValue({})).toBe('');
    expect(cellClipboardValue(undefined)).toBe('');
    expect(cellClipboardValue({ d: '#ERROR', e: 'sqrt: negative number', f: true })).toBe('');
  });

  it('copies excluded values (exclusion is a mark, not a deletion)', () => {
    expect(cellClipboardValue({ v: 5, d: '5', x: true })).toBe('5');
  });

  it('joins rows with newlines and cells with tabs', () => {
    expect(
      toTsv([
        [{ v: 1, d: '1' }, { t: 'a', d: 'a' }],
        [{}, { v: 2.5, d: '2.5' }],
      ]),
    ).toBe('1\ta\n\t2.5');
  });

  it('quotes text that holds tabs, newlines or quotes', () => {
    expect(toTsv([[{ t: 'a\tb' }, { t: 'say "hi"' }, { t: 'two\nlines' }]])).toBe('"a\tb"\t"say ""hi"""\t"two\nlines"');
  });

  it('can also write the displayed strings', () => {
    expect(toDisplayTsv([[{ v: 1 / 3, d: '0.333' }, { t: 'x', d: 'x' }]])).toBe('0.333\tx');
  });
});
