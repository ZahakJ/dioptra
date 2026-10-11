// Buttons at the right end of the table toolbar (the formula bar row). Phase 5B registers
// Analyze and Graph here; each tool is a small component given the table id.
import { useSyncExternalStore, type ComponentType } from 'react';
import type { Id } from '../engine/types';

export type TableTool = ComponentType<{ tableId: Id }>;

const tools = new Map<string, TableTool>();
const listeners = new Set<() => void>();
let version = 0;

export function registerTableTool(id: string, tool: TableTool): () => void {
  tools.set(id, tool);
  version++;
  for (const l of listeners) l();
  return () => {
    if (tools.get(id) === tool) {
      tools.delete(id);
      version++;
      for (const l of listeners) l();
    }
  };
}

export function TableTools({ tableId }: { tableId: Id }) {
  useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => version,
    () => version,
  );
  if (tools.size === 0) return null;
  return (
    <div className="table-tools">
      {[...tools.entries()].map(([id, T]) => (
        <T key={id} tableId={tableId} />
      ))}
    </div>
  );
}
