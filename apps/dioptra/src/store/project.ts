// The project tree, the open tabs and the active object.
import { create } from 'zustand';
import { getProject } from '../engine/commands';
import type { Id, ItemKind, ProjectItem, ProjectView, PValueStyle } from '../engine/types';
import { withEngine } from './engine';

export interface ProjectState {
  view: ProjectView | null;
  /** Objects open as tabs in the work area, in tab order. */
  tabs: Id[];
  activeId: Id | null;
  refresh(): Promise<ProjectView | null>;
  open(id: Id): void;
  close(id: Id): void;
  /** Forget tabs whose objects no longer exist. */
  prune(): void;
  reset(): void;
}

export const useProject = create<ProjectState>((set, get) => ({
  view: null,
  tabs: [],
  activeId: null,
  async refresh() {
    const view = await withEngine(getProject);
    set({ view });
    get().prune();
    return view;
  },
  open(id) {
    set((s) => ({ tabs: s.tabs.includes(id) ? s.tabs : [...s.tabs, id], activeId: id }));
  },
  close(id) {
    set((s) => {
      const i = s.tabs.indexOf(id);
      const tabs = s.tabs.filter((t) => t !== id);
      let activeId = s.activeId;
      if (activeId === id) activeId = tabs[Math.min(i, tabs.length - 1)] ?? null;
      return { tabs, activeId };
    });
  },
  prune() {
    const { view, tabs, activeId } = get();
    if (!view) return;
    const ids = new Set(view.items.map((i) => i.id));
    const kept = tabs.filter((t) => ids.has(t));
    let active = activeId && ids.has(activeId) ? activeId : (kept[kept.length - 1] ?? null);
    if (!active && view.items.length) active = view.items[0]!.id;
    if (active && !kept.includes(active)) kept.push(active);
    if (kept.length !== tabs.length || active !== activeId || kept.some((t, i) => t !== tabs[i])) {
      set({ tabs: kept, activeId: active });
    }
  },
  reset() {
    set({ tabs: [], activeId: null });
  },
}));

export function itemById(id: Id | null | undefined): ProjectItem | undefined {
  if (!id) return undefined;
  return useProject.getState().view?.items.find((i) => i.id === id);
}

export function useActiveItem(): ProjectItem | undefined {
  return useProject((s) => s.view?.items.find((i) => i.id === s.activeId));
}

export function usePValueStyle(): PValueStyle {
  return useProject((s) => s.view?.p_value_style ?? 'apa');
}

export const KIND_GROUPS: { kind: ItemKind; label: string }[] = [
  { kind: 'table', label: 'Tables' },
  { kind: 'analysis', label: 'Analyses' },
  { kind: 'graph', label: 'Graphs' },
  { kind: 'layout', label: 'Layouts' },
];
