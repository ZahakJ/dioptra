// Viewport cell cache: blocks of BLOCK_ROWS x BLOCK_COLS cells per table, fetched on demand
// with `GetCells` and invalidated (marked stale, kept for display until refetched) when the
// engine reports a table in `Changed.ids`.
import type { CellView, Id, Range } from '../engine/types';

export const BLOCK_ROWS = 64;
export const BLOCK_COLS = 32;

export interface BlockKey {
  br: number;
  bc: number;
}

interface Block {
  /** Row-major cells relative to the block origin; may be smaller than a full block. */
  cells: CellView[][];
  /** Table generation the cells were fetched at. */
  gen: number;
}

const keyOf = (br: number, bc: number) => `${br}:${bc}`;

export function blockRange(br: number, bc: number): Range {
  return {
    row0: br * BLOCK_ROWS,
    row1: (br + 1) * BLOCK_ROWS,
    col0: bc * BLOCK_COLS,
    col1: (bc + 1) * BLOCK_COLS,
  };
}

/** Every block that intersects `range` (half-open). */
export function blocksFor(range: Range): BlockKey[] {
  if (range.row1 <= range.row0 || range.col1 <= range.col0) return [];
  const out: BlockKey[] = [];
  const br0 = Math.floor(range.row0 / BLOCK_ROWS);
  const br1 = Math.floor((range.row1 - 1) / BLOCK_ROWS);
  const bc0 = Math.floor(range.col0 / BLOCK_COLS);
  const bc1 = Math.floor((range.col1 - 1) / BLOCK_COLS);
  for (let br = br0; br <= br1; br++) for (let bc = bc0; bc <= bc1; bc++) out.push({ br, bc });
  return out;
}

export class BlockCache {
  private tables = new Map<Id, Map<string, Block>>();
  private gens = new Map<Id, number>();
  private inflight = new Map<Id, Map<string, number>>();
  private size = 0;

  constructor(public readonly maxBlocks = 600) {}

  generation(table: Id): number {
    return this.gens.get(table) ?? 0;
  }

  /** A cell, fresh or stale; `undefined` when its block was never fetched. */
  get(table: Id, row: number, col: number): CellView | undefined {
    const blocks = this.tables.get(table);
    if (!blocks) return undefined;
    const block = blocks.get(keyOf(Math.floor(row / BLOCK_ROWS), Math.floor(col / BLOCK_COLS)));
    if (!block) return undefined;
    const r = block.cells[row % BLOCK_ROWS];
    if (!r) return EMPTY;
    return r[col % BLOCK_COLS] ?? EMPTY;
  }

  /** Whether every cell of `range` is cached (stale counts as cached for display). */
  covers(table: Id, range: Range): boolean {
    const blocks = this.tables.get(table);
    if (!blocks) return blocksFor(range).length === 0;
    return blocksFor(range).every(({ br, bc }) => blocks.has(keyOf(br, bc)));
  }

  /** Blocks of `range` that are absent or stale and not already being fetched. */
  missing(table: Id, range: Range): BlockKey[] {
    const blocks = this.tables.get(table);
    const gen = this.generation(table);
    const flying = this.inflight.get(table);
    return blocksFor(range).filter(({ br, bc }) => {
      const k = keyOf(br, bc);
      if (flying?.get(k) === gen) return false;
      const b = blocks?.get(k);
      return !b || b.gen !== gen;
    });
  }

  /** Record that a block fetch started at the current generation. */
  begin(table: Id, key: BlockKey): number {
    const gen = this.generation(table);
    let m = this.inflight.get(table);
    if (!m) this.inflight.set(table, (m = new Map()));
    m.set(keyOf(key.br, key.bc), gen);
    return gen;
  }

  /** A fetch finished (or failed); forget it as in flight. */
  end(table: Id, key: BlockKey, gen: number): void {
    const m = this.inflight.get(table);
    const k = keyOf(key.br, key.bc);
    if (m?.get(k) === gen) m.delete(k);
  }

  /** Store a fetched block. `cells` start at the block origin. */
  put(table: Id, key: BlockKey, cells: CellView[][], gen = this.generation(table)): void {
    let blocks = this.tables.get(table);
    if (!blocks) this.tables.set(table, (blocks = new Map()));
    const k = keyOf(key.br, key.bc);
    const existing = blocks.get(k);
    // Never overwrite newer data with an older fetch.
    if (existing && existing.gen > gen) return;
    if (existing) blocks.delete(k);
    else this.size++;
    blocks.set(k, { cells, gen });
    this.evict(table, k);
  }

  /** Mark every block of a table stale (kept for display until refetched). */
  invalidate(table: Id): void {
    this.gens.set(table, this.generation(table) + 1);
  }

  /** Forget a table entirely. */
  drop(table: Id): void {
    const blocks = this.tables.get(table);
    if (blocks) this.size -= blocks.size;
    this.tables.delete(table);
    this.inflight.delete(table);
  }

  get blockCount(): number {
    return this.size;
  }

  private evict(keepTable: Id, keepKey: string): void {
    // Map iteration is insertion order, and `put` re-inserts, so the first entries are the
    // least recently fetched.
    while (this.size > this.maxBlocks) {
      let evicted = false;
      for (const [t, blocks] of this.tables) {
        for (const k of blocks.keys()) {
          if (t === keepTable && k === keepKey) continue;
          blocks.delete(k);
          this.size--;
          evicted = true;
          break;
        }
        if (evicted) break;
      }
      if (!evicted) break;
    }
  }
}

const EMPTY: CellView = Object.freeze({}) as CellView;
