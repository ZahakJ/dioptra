import { describe, expect, it } from 'vitest';
import { applyMergePatch, combinePatches, getPath, patchAt, pathKeys } from './patch';

describe('merge-patch builder', () => {
  it('builds the patch for a JSON pointer or a dot path', () => {
    expect(patchAt('/axes/y/title', 'Response')).toEqual({ axes: { y: { title: 'Response' } } });
    expect(patchAt('style.theme', 'night')).toEqual({ style: { theme: 'night' } });
    expect(patchAt(['legend', 'at'], { x: 0.1, y: 0.2 })).toEqual({ legend: { at: { x: 0.1, y: 0.2 } } });
  });

  it('removes a field with null (or undefined)', () => {
    expect(patchAt('/axes/y/min', null)).toEqual({ axes: { y: { min: null } } });
    expect(patchAt('/axes/y/min', undefined)).toEqual({ axes: { y: { min: null } } });
  });

  it('decodes JSON pointer escapes', () => {
    expect(pathKeys('/a~1b/c~0d')).toEqual(['a/b', 'c~d']);
    expect(() => patchAt('', 1)).toThrow();
  });

  it('applies like RFC 7396 (arrays replaced whole)', () => {
    const spec = { family: 'column', axes: { y: { title: 'A', min: 0 } }, annotations: [{ kind: 'x' }] };
    expect(applyMergePatch(spec, { axes: { y: { min: null, max: 10 } } })).toEqual({ family: 'column', axes: { y: { title: 'A', max: 10 } }, annotations: [{ kind: 'x' }] });
    expect(applyMergePatch(spec, { annotations: [] })).toEqual({ ...spec, annotations: [] });
    expect(applyMergePatch(spec, { family: 'box' })).toMatchObject({ family: 'box' });
    // Spec examples from RFC 7396 appendix A.
    expect(applyMergePatch({ a: 'b' }, { a: 'c' })).toEqual({ a: 'c' });
    expect(applyMergePatch({ a: 'b' }, { b: 'c' })).toEqual({ a: 'b', b: 'c' });
    expect(applyMergePatch({ a: 'b' }, { a: null })).toEqual({});
    expect(applyMergePatch({ a: ['b'] }, { a: 'c' })).toEqual({ a: 'c' });
    expect(applyMergePatch({ a: 'c' }, { a: ['b'] })).toEqual({ a: ['b'] });
    expect(applyMergePatch({ a: { b: 'c' } }, { a: { b: 'd', c: null } })).toEqual({ a: { b: 'd' } });
    expect(applyMergePatch({ a: [{ b: 'c' }] }, { a: [1] })).toEqual({ a: [1] });
    expect(applyMergePatch(['a', 'b'], ['c', 'd'])).toEqual(['c', 'd']);
    expect(applyMergePatch({ a: 'b' }, ['c'])).toEqual(['c']);
    expect(applyMergePatch({ e: null }, { a: 1 })).toEqual({ e: null, a: 1 });
    expect(applyMergePatch([1, 2], { a: 'b', c: null })).toEqual({ a: 'b' });
    expect(applyMergePatch({}, { a: { bb: { ccc: null } } })).toEqual({ a: { bb: {} } });
  });

  it('combines debounced patches, later wins', () => {
    const a = patchAt('/axes/y/title', 'R');
    const b = patchAt('/axes/y/title', 'Re');
    const c = patchAt('/axes/x/title', 'Dose');
    expect(combinePatches(combinePatches(a, b), c)).toEqual({ axes: { y: { title: 'Re' }, x: { title: 'Dose' } } });
  });

  it('reads values at a path, through arrays', () => {
    const spec = { annotations: [{ text: 'p = .01' }], axes: { y: { title: 'A' } } };
    expect(getPath(spec, '/annotations/0/text')).toBe('p = .01');
    expect(getPath(spec, '/axes/y/title')).toBe('A');
    expect(getPath(spec, '/axes/x/title')).toBeUndefined();
  });
});
