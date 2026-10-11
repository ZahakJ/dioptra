import { beforeEach, describe, expect, it } from 'vitest';
import type { Engine } from '../engine/bridge';
import { setEngineForTests } from '../engine/engine';
import type { Command, Range, Response, TableMeta } from '../engine/types';
import { cellCache, useTables } from './table';

const meta: TableMeta = {
  id: 't1',
  title: 'T',
  kind: 'column',
  rows: 200,
  has_x: false,
  has_row_titles: false,
  row_titles: [],
  columns: Array.from({ length: 40 }, (_, i) => ({
    index: i,
    role: 'y' as const,
    dataset: i,
    subcolumn: 0,
    label: String(i),
    header: String(i),
    dataset_name: String(i),
    source: { kind: 'entered' as const },
  })),
  datasets: [],
  read_only: false,
};

let calls: Command[] = [];
let value = 1;

const fake: Engine = {
  backend: 'wasm',
  async dispatch(c: Command): Promise<Response> {
    calls.push(c);
    if (c.type === 'ping') return { type: 'pong', version: 'test' };
    if (c.type === 'get_table_meta') return { type: 'table_meta', ...meta };
    if (c.type === 'get_cells') {
      const r: Range = c.range;
      return {
        type: 'cells',
        range: r,
        rows: Array.from({ length: r.row1 - r.row0 }, (_, i) =>
          Array.from({ length: r.col1 - r.col0 }, (_, j) => ({ v: value * 1e6 + (r.row0 + i) * 1000 + r.col0 + j, d: '' })),
        ),
      };
    }
    return { type: 'error', message: 'unexpected' };
  },
};

const settle = () => new Promise((r) => setTimeout(r, 10));

describe('table store and viewport cache', () => {
  beforeEach(async () => {
    setEngineForTests(fake);
    calls = [];
    value = 1;
    cellCache.drop('t1');
    await useTables.getState().loadMeta('t1');
    calls = [];
  });

  it('fetches only the blocks a viewport needs, clipped to the table', async () => {
    useTables.getState().ensureCells('t1', { row0: 0, row1: 40, col0: 0, col1: 20 });
    await settle();
    const gets = calls.filter((c) => c.type === 'get_cells');
    expect(gets).toHaveLength(1);
    expect(gets[0]).toMatchObject({ range: { row0: 0, row1: 64, col0: 0, col1: 32 } });
    expect(cellCache.get('t1', 5, 7)!.v).toBe(1e6 + 5007);
    useTables.getState().ensureCells('t1', { row0: 10, row1: 50, col0: 0, col1: 20 });
    await settle();
    expect(calls.filter((c) => c.type === 'get_cells')).toHaveLength(1);
    useTables.getState().ensureCells('t1', { row0: 150, row1: 260, col0: 30, col1: 50 });
    await settle();
    const more = calls.filter((c) => c.type === 'get_cells').slice(1);
    expect(more.map((c) => (c as { range: Range }).range)).toEqual([
      { row0: 128, row1: 192, col0: 0, col1: 32 },
      { row0: 128, row1: 192, col0: 32, col1: 40 },
      { row0: 192, row1: 200, col0: 0, col1: 32 },
      { row0: 192, row1: 200, col0: 32, col1: 40 },
    ]);
  });

  it('refetches blocks of a table named in Changed.ids', async () => {
    useTables.getState().ensureCells('t1', { row0: 0, row1: 10, col0: 0, col1: 10 });
    await settle();
    value = 2;
    await useTables.getState().invalidate(['t1', 'other']);
    expect(calls.some((c) => c.type === 'get_table_meta')).toBe(true);
    expect(cellCache.get('t1', 0, 0)!.v).toBe(1e6); // stale value still shown
    useTables.getState().ensureCells('t1', { row0: 0, row1: 10, col0: 0, col1: 10 });
    await settle();
    expect(cellCache.get('t1', 0, 0)!.v).toBe(2e6);
  });
});
