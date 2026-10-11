// Grid state shared with the status line, inspector and commands: the selection per table,
// the active cell's provenance, and which table's grid has focus.
import { create } from 'zustand';
import type { CellInfo, Id } from '../engine/types';
import { ORIGIN, type Selection } from './selection';
import type { FormulaMode } from './formulaBar';

export interface GridState {
  selections: Record<Id, Selection>;
  /** Provenance of the active cell of the active table (fetched lazily). */
  activeInfo: CellInfo | null;
  /** Formula bar mode per table. */
  formulaMode: Record<Id, FormulaMode>;
  /** Bumped to ask a grid to start editing its active cell (menu "Edit cell"). */
  editRequest: number;
  setSelection(table: Id, sel: Selection): void;
  setActiveInfo(info: CellInfo | null): void;
  setFormulaMode(table: Id, mode: FormulaMode): void;
  requestEdit(): void;
}

export const useGrid = create<GridState>((set) => ({
  selections: {},
  activeInfo: null,
  formulaMode: {},
  editRequest: 0,
  setSelection: (table, sel) => set((s) => ({ selections: { ...s.selections, [table]: sel } })),
  setActiveInfo: (activeInfo) => set({ activeInfo }),
  setFormulaMode: (table, mode) => set((s) => ({ formulaMode: { ...s.formulaMode, [table]: mode } })),
  requestEdit: () => set((s) => ({ editRequest: s.editRequest + 1 })),
}));

export function selectionOf(table: Id | null | undefined): Selection {
  if (!table) return ORIGIN;
  return useGrid.getState().selections[table] ?? ORIGIN;
}

export function useSelection(table: Id | null | undefined): Selection {
  return useGrid((s) => (table ? (s.selections[table] ?? ORIGIN) : ORIGIN));
}

/** Whether keyboard focus is inside a grid. */
export function gridHasFocus(): boolean {
  const a = typeof document !== 'undefined' ? document.activeElement : null;
  return !!a?.closest?.('[data-grid]');
}
