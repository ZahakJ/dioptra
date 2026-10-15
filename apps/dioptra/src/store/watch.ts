// Desktop: the folder being watched for new instrument exports (Dioptra Lab).
import { create } from 'zustand';
import type { BatchItem } from '../io/batchSummary';
import { startWatch, stopWatch, type BatchOptions, type Template, type WatchEvent } from '../io/batch';
import { useNotices } from './notices';

interface WatchState {
  dir: string | null;
  table: string | null;
  items: BatchItem[];
  unlisten: (() => void) | null;
  start(dir: string, template: Template, table: string, options: BatchOptions): Promise<void>;
  stop(): Promise<void>;
}

export const useWatch = create<WatchState>((set, get) => ({
  dir: null,
  table: null,
  items: [],
  unlisten: null,
  async start(dir, template, table, options) {
    await get().stop();
    const unlisten = await startWatch(dir, template, table, options, (e: WatchEvent) => onEvent(e));
    set({ dir, table, items: [], unlisten });
  },
  async stop() {
    const u = get().unlisten;
    if (!u) return;
    await stopWatch().catch(() => undefined);
    u();
    set({ dir: null, unlisten: null });
  },
}));

function onEvent(e: WatchEvent): void {
  const notices = useNotices.getState();
  if (e.item) {
    const it = e.item;
    useWatch.setState((s) => ({ items: [...s.items, it] }));
    notices.push(
      it.error
        ? { kind: 'error', text: `${it.name}: ${it.error}` }
        : { kind: 'info', text: `Analysed ${it.name}: ${it.written?.length ?? 0} files in the analysed folder.` },
    );
  }
  if (e.error) notices.push({ kind: 'error', text: `Watch folder: ${e.error}` });
  if (e.stopped) {
    useWatch.getState().unlisten?.();
    useWatch.setState({ dir: null, unlisten: null });
  }
}
