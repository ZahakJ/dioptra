import { describe, expect, it } from 'vitest';
import { fuzzyFilter, fuzzyMatch } from './fuzzy';

describe('fuzzyMatch', () => {
  it('matches subsequences and rejects others', () => {
    expect(fuzzyMatch('nt', 'New table')).not.toBeNull();
    expect(fuzzyMatch('tn', 'New table')).toBeNull();
    expect(fuzzyMatch('xyz', 'New table')).toBeNull();
  });

  it('returns positions of matched characters', () => {
    const m = fuzzyMatch('nt', 'New table')!;
    expect(m.positions).toEqual([0, 4]);
  });

  it('prefers word starts and prefixes', () => {
    const a = fuzzyMatch('sa', 'Save as')!;
    const b = fuzzyMatch('sa', 'Paste as new table')!;
    expect(a.score).toBeGreaterThan(b.score);
  });

  it('empty query matches everything with score 0', () => {
    expect(fuzzyMatch('', 'anything')).toEqual({ score: 0, positions: [] });
  });
});

describe('fuzzyFilter', () => {
  const items = ['New table', 'New project', 'Open', 'Save', 'Save as', 'Toggle dark theme', 'Fill down'];
  it('ranks the best match first', () => {
    const r = fuzzyFilter('new tab', items, (s) => [s]);
    expect(r[0]!.item).toBe('New table');
  });
  it('keeps order for an empty query', () => {
    expect(fuzzyFilter('', items, (s) => [s]).map((r) => r.item)).toEqual(items);
  });
  it('matches keywords as secondary fields', () => {
    const r = fuzzyFilter('dark', [{ t: 'Toggle theme', k: ['dark mode'] }, { t: 'Save', k: [] }], (x) => [x.t, ...x.k]);
    expect(r.map((x) => x.item.t)).toEqual(['Toggle theme']);
  });
  it('is case-insensitive', () => {
    expect(fuzzyFilter('FILL', items, (s) => [s])[0]!.item).toBe('Fill down');
  });
});
