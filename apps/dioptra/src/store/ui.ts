// UI preferences and transient UI state. Theme, panel sizes and visibility, collapsed
// sections and column widths persist in localStorage (the window's own size and position are
// kept by the desktop shell); everything else is per session.
import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { Id } from '../engine/types';

export type ThemePref = 'light' | 'dark' | 'system';
/** Grid row height: comfortable 28 px, compact 22 px (DESIGN.md §12.6). */
export type Density = 'comfortable' | 'compact';
/** What the graph editor shows behind the figure: white paper, the app's sheet colour, or a
 *  transparency checkerboard. */
export type Paper = 'white' | 'theme' | 'checker';
export const PAPERS: Paper[] = ['white', 'theme', 'checker'];

export interface UiState {
  theme: ThemePref;
  railWidth: number;
  inspectorWidth: number;
  railVisible: boolean;
  inspectorVisible: boolean;
  /** Column widths in px per table, by flat column index. */
  colWidths: Record<Id, Record<number, number>>;
  /** Collapsed state of named sections: `insp:<section id>`, `rail:<kind>`. */
  collapsed: Record<string, boolean>;
  density: Density;
  /** Paper per graph; white when unset. */
  paper: Record<Id, Paper>;
  paletteOpen: boolean;
  shortcutsOpen: boolean;
  setTheme(t: ThemePref): void;
  setRailWidth(w: number): void;
  setInspectorWidth(w: number): void;
  toggleRail(): void;
  toggleInspector(): void;
  setColWidth(table: Id, col: number, width: number): void;
  setCollapsed(key: string, collapsed: boolean): void;
  setDensity(d: Density): void;
  setPaper(graph: Id, p: Paper): void;
  setPaletteOpen(open: boolean): void;
  setShortcutsOpen(open: boolean): void;
}

const safeStorage = createJSONStorage(() => {
  try {
    const s = window.localStorage;
    s.getItem('dioptra-probe');
    return s;
  } catch {
    const mem = new Map<string, string>();
    return {
      getItem: (k: string) => mem.get(k) ?? null,
      setItem: (k: string, v: string) => void mem.set(k, v),
      removeItem: (k: string) => void mem.delete(k),
    };
  }
});

export const RAIL_MIN = 160;
export const RAIL_MAX = 480;
export const INSPECTOR_MIN = 220;
export const INSPECTOR_MAX = 560;

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, Math.round(v)));

export const useUi = create<UiState>()(
  persist(
    (set) => ({
      theme: 'system',
      railWidth: 232,
      inspectorWidth: 288,
      railVisible: true,
      inspectorVisible: true,
      colWidths: {},
      collapsed: {},
      density: 'comfortable',
      paper: {},
      paletteOpen: false,
      shortcutsOpen: false,
      setTheme: (theme) => set({ theme }),
      setRailWidth: (w) => set({ railWidth: clamp(w, RAIL_MIN, RAIL_MAX) }),
      setInspectorWidth: (w) => set({ inspectorWidth: clamp(w, INSPECTOR_MIN, INSPECTOR_MAX) }),
      toggleRail: () => set((s) => ({ railVisible: !s.railVisible })),
      toggleInspector: () => set((s) => ({ inspectorVisible: !s.inspectorVisible })),
      setColWidth: (table, col, width) =>
        set((s) => ({
          colWidths: { ...s.colWidths, [table]: { ...s.colWidths[table], [col]: clamp(width, 36, 640) } },
        })),
      setCollapsed: (key, c) => set((s) => ({ collapsed: { ...s.collapsed, [key]: c } })),
      setDensity: (density) => set({ density: density === 'compact' ? 'compact' : 'comfortable' }),
      setPaper: (graph, p) => set((s) => ({ paper: { ...s.paper, [graph]: PAPERS.includes(p) ? p : 'white' } })),
      setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
      setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
    }),
    {
      name: 'dioptra-ui',
      version: 3,
      migrate: (old) => ({ collapsed: {}, density: 'comfortable', paper: {}, ...(old as object) }) as UiState,
      storage: safeStorage,
      partialize: (s) => ({
        theme: s.theme,
        railWidth: s.railWidth,
        inspectorWidth: s.inspectorWidth,
        railVisible: s.railVisible,
        inspectorVisible: s.inspectorVisible,
        colWidths: s.colWidths,
        collapsed: s.collapsed,
        density: s.density,
        paper: s.paper,
      }),
    },
  ),
);

/** The theme actually applied: `system` resolves through the OS preference. */
export function resolveTheme(pref: ThemePref, systemDark: boolean): 'light' | 'dark' {
  return pref === 'system' ? (systemDark ? 'dark' : 'light') : pref;
}

/** Light -> dark -> system -> light. */
export function nextTheme(pref: ThemePref): ThemePref {
  return pref === 'light' ? 'dark' : pref === 'dark' ? 'system' : 'light';
}

/** The paper a graph is shown on: white unless the user chose otherwise. */
export function paperOf(paper: Record<Id, Paper>, graph: Id): Paper {
  const p = paper[graph];
  return p && PAPERS.includes(p) ? p : 'white';
}
