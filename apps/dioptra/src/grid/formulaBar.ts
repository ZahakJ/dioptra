// The formula bar's state as a pure reducer. In "cell" mode it shows and edits the active
// cell's raw input; in "column" mode it shows and edits the column formula.
import type { CellView } from '../engine/types';
import { formatFull } from './clipboard';

export type FormulaMode = 'cell' | 'column';

export interface FormulaBarState {
  mode: FormulaMode;
  /** What the bar currently shows (possibly being edited). */
  text: string;
  /** The value the bar was synced to (to tell an edit from a sync). */
  original: string;
  editing: boolean;
  /** Raw input of the active cell. */
  cellText: string;
  /** The column formula with a leading `=`, or empty. */
  columnText: string;
}

export const INITIAL_BAR: FormulaBarState = {
  mode: 'cell',
  text: '',
  original: '',
  editing: false,
  cellText: '',
  columnText: '',
};

/** The raw input of a cell as the user would retype it. */
export function rawCellInput(cell: CellView | undefined, cellFormula?: string | null): string {
  if (cellFormula) return cellFormula.startsWith('=') ? cellFormula : `=${cellFormula}`;
  if (!cell) return '';
  if (typeof cell.v === 'number') return formatFull(cell.v);
  if (cell.t !== undefined) return cell.t;
  return '';
}

export type FormulaBarEvent =
  | { type: 'sync'; cellText: string; columnText: string }
  | { type: 'mode'; mode: FormulaMode }
  | { type: 'input'; text: string }
  | { type: 'cancel' }
  | { type: 'committed' };

function shown(s: FormulaBarState, mode: FormulaMode): string {
  return mode === 'column' ? s.columnText : s.cellText;
}

export function formulaBarReducer(s: FormulaBarState, e: FormulaBarEvent): FormulaBarState {
  switch (e.type) {
    case 'sync': {
      const next = { ...s, cellText: e.cellText, columnText: e.columnText };
      if (s.editing) return next;
      const text = shown(next, s.mode);
      return { ...next, text, original: text };
    }
    case 'mode': {
      const text = shown(s, e.mode);
      return { ...s, mode: e.mode, text, original: text, editing: false };
    }
    case 'input':
      return { ...s, text: e.text, editing: e.text !== s.original || s.editing };
    case 'cancel':
    case 'committed': {
      const text = shown(s, s.mode);
      return { ...s, text, original: text, editing: false };
    }
  }
}

export type FormulaCommit =
  | { kind: 'cell'; input: string }
  | { kind: 'column'; expr: string | null };

/** What pressing Enter should send to the engine, or null when nothing changed. */
export function commitOf(s: FormulaBarState): FormulaCommit | null {
  if (s.text === s.original) return null;
  if (s.mode === 'column') {
    const t = s.text.trim().replace(/^=\s*/, '').trim();
    return { kind: 'column', expr: t === '' ? null : t };
  }
  return { kind: 'cell', input: s.text };
}
