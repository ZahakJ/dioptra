import { describe, expect, it } from 'vitest';
import type { CellView } from '../engine/types';
import { BLOCK_COLS, BLOCK_ROWS, BlockCache, blockRange, blocksFor } from './blockCache';

const block = (v: number, rows = BLOCK_ROWS, cols = BLOCK_COLS): CellView[][] =>
  Array.from({ length: rows }, (_, r) => Array.from({ length: cols }, (_, c) => ({ v: v + r * 1000 + c, d: '' })));

describe('blocks', () => {
  it('covers a range with 64 x 32 blocks', () => {
    expect(blocksFor({ row0: 0, row1: 64, col0: 0, col1: 32 })).toEqual([{ br: 0, bc: 0 }]);
    expect(blocksFor({ row0: 60, row1: 70, col0: 30, col1: 33 })).toEqual([
      { br: 0, bc: 0 },
      { br: 0, bc: 1 },
      { br: 1, bc: 0 },
      { br: 1, bc: 1 },
    ]);
    expect(blocksFor({ row0: 5, row1: 5, col0: 0, col1: 3 })).toEqual([]);
    expect(blockRange(2, 1)).toEqual({ row0: 128, row1: 192, col0: 32, col1: 64 });
  });
});

describe('BlockCache', () => {
  it('reports missing blocks and serves cells once put', () => {
    const c = new BlockCache();
    const r = { row0: 0, row1: 100, col0: 0, col1: 10 };
    expect(c.missing('t', r)).toHaveLength(2);
    expect(c.get('t', 3, 4)).toBeUndefined();
    c.put('t', { br: 0, bc: 0 }, block(0));
    expect(c.get('t', 3, 4)!.v).toBe(3004);
    expect(c.missing('t', r)).toEqual([{ br: 1, bc: 0 }]);
    expect(c.covers('t', { row0: 0, row1: 64, col0: 0, col1: 32 })).toBe(true);
    expect(c.covers('t', r)).toBe(false);
  });

  it('serves empty cells inside a short (clipped) block', () => {
    const c = new BlockCache();
    c.put('t', { br: 0, bc: 0 }, block(0, 3, 2));
    expect(c.get('t', 1, 1)!.v).toBe(1001);
    expect(c.get('t', 10, 10)).toEqual({});
  });

  it('keeps stale data for display but asks for a refetch after invalidation', () => {
    const c = new BlockCache();
    c.put('t', { br: 0, bc: 0 }, block(0));
    c.invalidate('t');
    expect(c.get('t', 0, 0)!.v).toBe(0);
    expect(c.missing('t', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([{ br: 0, bc: 0 }]);
    c.put('t', { br: 0, bc: 0 }, block(7));
    expect(c.missing('t', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([]);
    expect(c.get('t', 0, 0)!.v).toBe(7);
  });

  it('does not refetch blocks already in flight, and ignores results older than a change', () => {
    const c = new BlockCache();
    const key = { br: 0, bc: 0 };
    const gen = c.begin('t', key);
    expect(c.missing('t', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([]);
    c.invalidate('t'); // the table changed while the fetch was in flight
    expect(c.missing('t', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([key]);
    const gen2 = c.begin('t', key);
    c.put('t', key, block(2), gen2);
    c.put('t', key, block(1), gen); // late, older answer
    c.end('t', key, gen);
    c.end('t', key, gen2);
    expect(c.get('t', 0, 0)!.v).toBe(2);
    expect(c.missing('t', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([]);
  });

  it('keeps tables separate and drops them', () => {
    const c = new BlockCache();
    c.put('a', { br: 0, bc: 0 }, block(1));
    c.put('b', { br: 0, bc: 0 }, block(2));
    c.invalidate('a');
    expect(c.missing('b', { row0: 0, row1: 1, col0: 0, col1: 1 })).toEqual([]);
    c.drop('a');
    expect(c.get('a', 0, 0)).toBeUndefined();
    expect(c.get('b', 0, 0)!.v).toBe(2);
    expect(c.blockCount).toBe(1);
  });

  it('evicts the least recently fetched blocks beyond its limit', () => {
    const c = new BlockCache(3);
    for (let i = 0; i < 5; i++) c.put('t', { br: i, bc: 0 }, block(i, 1, 1));
    expect(c.blockCount).toBe(3);
    expect(c.get('t', 0, 0)).toBeUndefined();
    expect(c.get('t', 4 * BLOCK_ROWS, 0)!.v).toBe(4);
  });
});
