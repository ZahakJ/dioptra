import { describe, expect, it } from 'vitest';
import type { GraphSpecT } from '../engine/objects';
import { connectingOf, isCategorical } from './connecting';
import { familiesFor, familyForAnalysis, PRESETS } from './families';
import { patchAt } from './patch';
import { describeTarget } from './targets';

const spec = (family: string, options: Record<string, unknown> = {}) => ({ family, options }) as unknown as GraphSpecT;
const target = (id: string) => ({ id, target: '/options/connecting', bbox: { x: 0, y: 0, width: 1, height: 1 } });

describe('connecting lines in the editor', () => {
  it('belong to the categorical families only', () => {
    for (const f of ['column', 'strip', 'box', 'violin']) expect(isCategorical(spec(f))).toBe(true);
    for (const f of ['scatter', 'histogram', 'pie']) expect(isCategorical(spec(f))).toBe(false);
  });
  it('read the spec with defaults', () => {
    expect(connectingOf(spec('column')).connect).toBeUndefined();
    expect(connectingOf(spec('column', { connecting: { connect: 'both' } })).connect).toBe('both');
  });
  it('patch one field of the options', () => {
    expect(patchAt('/options/connecting/connect', 'means')).toEqual({ options: { connecting: { connect: 'means' } } });
  });
  it('name subject and summary lines', () => {
    expect(describeTarget(target('subject-3'))).toBe('Subject line 3');
    expect(describeTarget(target('subject-g1-2'))).toBe('Subject line 2, group 2');
    expect(describeTarget(target('subject-s0-5'))).toBe('Subject line 5, data set 1');
    expect(describeTarget(target('connect-summary'))).toBe('Summary line');
  });
  it('paired analyses default to bars with connecting lines', () => {
    for (const k of ['wilcoxon', 'repeated_measures_anova', 'friedman']) expect(familyForAnalysis(k)).toBe('paired');
    expect(familiesFor('column')).toContain('paired');
    expect(PRESETS).toContain('before_after');
  });
});
