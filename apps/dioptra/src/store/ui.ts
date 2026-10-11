// UI preferences and transient UI state. Theme, panel sizes and visibility, collapsed
// sections and column widths persist in localStorage (the window's own size and position are
// kept by the desktop shell); everything else is per session.
import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';
import type { Id } from '../engine/types';

export type ThemePref = 'light' | 'dark' | 'system';

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
  paletteOpen: boolean;
  shortcutsOpen: boolean;
  setTheme(t: ThemePref): void;
  setRailWidth(w: number): void;
  setInspectorWidth(w: number): void;
  toggleRail(): void;
  toggleInspector(): void;
  setColWidth(table: Id, col: number, width: number): void;
  setCollapsed(key: string, collapsed: boolean): void;
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
      setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
      setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
    }),
    {
      name: 'dioptra-ui',
      version: 2,
      migrate: (old) => ({ collapsed: {}, ...(old as object) }) as UiState,
      storage: safeStorage,
      partialize: (s) => ({
        theme: s.theme,
        railWidth: s.railWidth,
        inspectorWidth: s.inspectorWidth,
        railVisible: s.railVisible,
        inspectorVisible: s.inspectorVisible,
        colWidths: s.colWidths,
        collapsed: s.collapsed,
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
