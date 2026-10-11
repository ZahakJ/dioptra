// Per-table structure (TableMeta) and the viewport cell cache.
import { create } from 'zustand';
import { getCells, getTableMeta } from '../engine/commands';
import type { CellView, Id, Range, TableMeta } from '../engine/types';
import { BlockCache, blockRange } from './blockCache';
import { withEngine } from './engine';

/** The shared cell cache. Components read it directly and subscribe to `version`. */
export const cellCache = new BlockCache();

export interface TableState {
  metas: Record<Id, TableMeta>;
  /** Bumped per table whenever its cached cells or meta change. */
  version: Record<Id, number>;
  loadMeta(id: Id): Promise<TableMeta | undefined>;
  /** Fetch any missing or stale blocks covering `range` (clipped to the table). */
  ensureCells(id: Id, range: Range): void;
  /** Fetch a range now (bypassing nothing; fills the cache and returns the cells). */
  fetchRange(id: Id, range: Range): Promise<CellView[][]>;
  /** Called with `Changed.ids`: refetch metas of loaded tables and mark their cells stale. */
  invalidate(ids: Id[]): Promise<void>;
  forget(id: Id): void;
}

const bump = (v: Record<Id, number>, id: Id) => ({ ...v, [id]: (v[id] ?? 0) + 1 });

export const useTables = create<TableState>((set, get) => ({
  metas: {},
  version: {},
  async loadMeta(id) {
    try {
      const meta = await withEngine((e) => getTableMeta(e, id));
      set((s) => ({ metas: { ...s.metas, [id]: meta }, version: bump(s.version, id) }));
      return meta;
    } catch {
      get().forget(id);
      return undefined;
    }
  },
  ensureCells(id, range) {
    const meta = get().metas[id];
    if (!meta) return;
    const cols = meta.columns.length;
    const clipped: Range = {
      row0: Math.max(0, range.row0),
      row1: Math.min(range.row1, meta.rows),
      col0: Math.max(0, range.col0),
      col1: Math.min(range.col1, cols),
    };
    if (clipped.row1 <= clipped.row0 || clipped.col1 <= clipped.col0) return;
    for (const key of cellCache.missing(id, clipped)) {
      const gen = cellCache.begin(id, key);
      const r = blockRange(key.br, key.bc);
      const want: Range = { row0: r.row0, row1: Math.min(r.row1, meta.rows), col0: r.col0, col1: Math.min(r.col1, cols) };
      withEngine((e) => getCells(e, id, want))
        .then((res) => {
          cellCache.put(id, key, res.rows, gen);
          set((s) => ({ version: bump(s.version, id) }));
        })
        .catch(() => undefined)
        .finally(() => cellCache.end(id, key, gen));
    }
  },
  async fetchRange(id, range) {
    const res = await withEngine((e) => getCells(e, id, range));
    return res.rows;
  },
  async invalidate(ids) {
    const loaded = ids.filter((id) => get().metas[id]);
    for (const id of loaded) cellCache.invalidate(id);
    await Promise.all(loaded.map((id) => get().loadMeta(id)));
  },
  forget(id) {
    cellCache.drop(id);
    set((s) => {
      const metas = { ...s.metas };
      delete metas[id];
      return { metas, version: bump(s.version, id) };
    });
  },
}));

export function useTableMeta(id: Id | null | undefined): TableMeta | undefined {
  return useTables((s) => (id ? s.metas[id] : undefined));
}

export function useTableVersion(id: Id | null | undefined): number {
  return useTables((s) => (id ? (s.version[id] ?? 0) : 0));
}
