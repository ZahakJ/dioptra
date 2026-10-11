// The command history (audit log), newest first. Loaded on demand by the history section.
import { create } from 'zustand';
import { getHistory } from '../engine/commands';
import type { HistoryEntry } from '../engine/types';
import { withEngine } from './engine';

export interface HistoryState {
  entries: HistoryEntry[];
  /** Whether anything shows the history (only then is it refreshed after mutations). */
  watched: number;
  refresh(): Promise<void>;
  watch(): () => void;
}

export const useHistory = create<HistoryState>((set, get) => ({
  entries: [],
  watched: 0,
  async refresh() {
    const entries = await withEngine((e) => getHistory(e, 200));
    set({ entries });
  },
  watch() {
    set((s) => ({ watched: s.watched + 1 }));
    void get().refresh();
    return () => set((s) => ({ watched: Math.max(0, s.watched - 1) }));
  },
}));
