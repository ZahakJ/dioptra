import { describe, expect, it } from 'vitest';
import type { Annotation, Sheet } from '../engine/objects';
import { bracketsFromSheet, bracketText, comparisonsFromSheet, nextFreePair, placeBracket, splitPair } from './brackets';

const sheet = (kind: string, rows: Sheet['sections'][number]['table']['rows'], columns = ['Comparison', 'Mean diff.', 'Adjusted P']): Sheet => ({
  title: '',
  analysis_kind: kind,
  sections: [{ title: 'Multiple comparisons', table: { columns, rows } }],
  assumptions: [],
  summary: [],
  checklist: [],
  stale: false,
});

describe('bracket placement', () => {
  it('adds a bracket between two categories in order', () => {
    const a = placeBracket([], 2, 0, '**', 3);
    expect(a).toEqual([{ kind: 'significance_bracket', from: 0, to: 2, text: '**' }]);
  });
  it('updates the text of an existing pair instead of stacking a duplicate', () => {
    const a = placeBracket([], 0, 1, '*', 3);
    const b = placeBracket(a, 1, 0, 'ns', 3);
    expect(b).toHaveLength(1);
    expect((b[0] as { text: string }).text).toBe('ns');
  });
  it('rejects invalid pairs', () => {
    const none: Annotation[] = [];
    expect(placeBracket(none, 1, 1, '*', 3)).toBe(none);
    expect(placeBracket(none, 0, 3, '*', 3)).toBe(none);
    expect(placeBracket(none, -1, 1, '*', 3)).toBe(none);
  });
  it('keeps other annotations', () => {
    const t: Annotation = { kind: 'text_label', text: 'n = 6', x: 1, y: 2 };
    expect(placeBracket([t], 0, 1, '*', 2)).toEqual([t, { kind: 'significance_bracket', from: 0, to: 1, text: '*' }]);
  });
  it('suggests the next free pair, neighbours first', () => {
    expect(nextFreePair([], 3)).toEqual([0, 1]);
    let a = placeBracket([], 0, 1, '*', 3);
    expect(nextFreePair(a, 3)).toEqual([1, 2]);
    a = placeBracket(a, 1, 2, '*', 3);
    expect(nextFreePair(a, 3)).toEqual([0, 2]);
    a = placeBracket(a, 0, 2, '*', 3);
    expect(nextFreePair(a, 3)).toBeNull();
  });
  it('labels follow the p style', () => {
    expect(bracketText(0.004, 'gp')).toBe('**');
    expect(bracketText(0.2, 'gp')).toBe('ns');
    expect(bracketText(0.004, 'apa')).toBe('p = .004');
    expect(bracketText(0.0004, 'apa')).toBe('p < .001');
    expect(bracketText(0.004, 'nejm')).toBe('P=0.004');
  });
});

describe('brackets from an analysis', () => {
  it('splits comparison labels', () => {
    expect(splitPair('Control vs. High dose')).toEqual(['Control', 'High dose']);
    expect(splitPair('A - B')).toEqual(['A', 'B']);
    expect(splitPair('A–B')).toBeNull();
  });
  it('reads a multiple-comparisons table by category names', () => {
    const s = sheet('one_way_anova', [
      [{ text: 'Control vs Low' }, { number: { v: -4 } }, { p_value: { p: 0.03 } }],
      [{ text: 'Control vs High' }, { number: { v: -10 } }, { p_value: { p: 0.00001 } }],
      [{ text: 'Low vs High' }, { number: { v: -6 } }, { p_value: { p: 0.2 } }],
    ]);
    expect(comparisonsFromSheet(s, ['Control', 'Low', 'High'])).toEqual([
      { from: 0, to: 1, p: 0.03 },
      { from: 0, to: 2, p: 0.00001 },
      { from: 1, to: 2, p: 0.2 },
    ]);
    const ann = bracketsFromSheet([], s, ['Control', 'Low', 'High'], 'gp');
    expect(ann.map((a) => (a as { text: string }).text)).toEqual(['*', '****', 'ns']);
  });
  it('uses the adjusted p of an engine comparisons table, not the unadjusted one', () => {
    const s = sheet(
      'one_way_anova',
      [
        [{ text: 'Vehicle vs. Low dose' }, { number: { v: -0.8 } }, { p_value: { p: 0.04 } }, { p_value: { p: 0.21 } }],
        [{ text: 'Vehicle vs. High dose' }, { number: { v: -18 } }, { p_value: { p: 1e-9 } }, { p_value: { p: 4e-9 } }],
      ],
      ['Comparison', 'Mean diff.', 'P (unadjusted)', 'Adjusted P'],
    );
    expect(comparisonsFromSheet(s, ['Vehicle', 'Low dose', 'High dose'])).toEqual([
      { from: 0, to: 1, p: 0.21 },
      { from: 0, to: 2, p: 4e-9 },
    ]);
  });
  it('reads two group-name columns', () => {
    const s = sheet('one_way_anova', [[{ text: 'High' }, { text: 'Control' }, { p_value: { p: 0.01 } }]], ['Group 1', 'Group 2', 'P']);
    expect(comparisonsFromSheet(s, ['Control', 'Low', 'High'])).toEqual([{ from: 0, to: 2, p: 0.01 }]);
  });
  it('a two-group test gives one bracket', () => {
    const s = sheet('t_test', [[{ text: 't' }, { number: { v: 3.2 } }, { p_value: { p: 0.005 } }]], ['', 't', 'P']);
    expect(comparisonsFromSheet(s, ['A', 'B'])).toEqual([{ from: 0, to: 1, p: 0.005 }]);
    expect(comparisonsFromSheet(s, ['A', 'B', 'C'])).toEqual([]);
  });
  it('ignores rows that name categories not plotted', () => {
    const s = sheet('one_way_anova', [[{ text: 'X vs Y' }, { number: { v: 1 } }, { p_value: { p: 0.01 } }]]);
    expect(comparisonsFromSheet(s, ['A', 'B'])).toEqual([]);
  });
});
