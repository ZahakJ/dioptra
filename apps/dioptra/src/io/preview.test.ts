import { describe, expect, it } from 'vitest';
import { formatPasteReport } from './report';
import { parsePreview, suggestKind } from './preview';

describe('import preview', () => {
  it('detects tabs, a header and point decimals', () => {
    const p = parsePreview('Dose\tResp\n1\t2.5\n2\t3.5\n4\t4.0\n');
    expect(p.separator).toBe('tab');
    expect(p.header).toBe(true);
    expect(p.decimal).toBe('point');
    expect(p.totalRows).toBe(3);
    expect(p.cols).toBe(2);
  });

  it('detects decimal commas with semicolons', () => {
    const p = parsePreview('1,5;2,25\n3,0;4,75');
    expect(p.separator).toBe('semicolon');
    expect(p.decimal).toBe('comma');
    expect(p.header).toBe(false);
  });

  it('honours explicit options and shows at most 10 data rows', () => {
    const text = Array.from({ length: 30 }, (_, i) => `${i},${i * 2}`).join('\n');
    const p = parsePreview(text, { separator: 'comma', header: false });
    expect(p.rows).toHaveLength(10);
    expect(p.totalRows).toBe(30);
  });

  it('handles quoted fields', () => {
    const p = parsePreview('"a,b",2\n"say ""x""",3', { separator: 'comma', header: false });
    expect(p.rows[0]).toEqual(['a,b', '2']);
    expect(p.rows[1]![0]).toBe('say "x"');
  });

  it('suggests a kind from the shape', () => {
    expect(suggestKind(parsePreview('X\tY\n1\t2\n2\t4\n3\t9')).kind).toBe('xy');
    expect(suggestKind(parsePreview('A\tB\n5\t2\n3\t4\n9\t1')).kind).toBe('column');
    expect(suggestKind(parsePreview('\tMale\tFemale\nWT\t1\t2\nKO\t3\t4')).kind).toBe('grouped');
    expect(suggestKind(parsePreview('Age\tSex\tBMI\n54\tF\t23\n61\tM\t27')).kind).toBe('variables');
    expect(suggestKind(parsePreview('')).kind).toBe('variables');
  });
});

describe('paste report sentence', () => {
  it('reads like the spec', () => {
    const s = formatPasteReport({
      separator: 'tab',
      decimal: 'comma',
      header_detected: false,
      rows: 120,
      cols: 3,
      numeric_cells: 358,
      text_cells: 2,
      missing_cells: 0,
      notes: [],
      changed: { ids: [], tree: false },
    });
    expect(s).toBe('Pasted 120 × 3; tab-separated; decimal comma; 2 text cells kept as text');
  });
});
