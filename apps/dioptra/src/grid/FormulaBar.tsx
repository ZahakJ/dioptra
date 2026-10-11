// The formula bar: the active cell's raw input (numbers at full precision, text, =formula) or,
// in column mode, the column formula. Enter commits, Escape reverts.
import { useEffect, useReducer, useRef } from 'react';
import { FormulaIcon } from '../app/icons';
import { TableTools } from '../app/tableTools';
import { Segmented } from '../ui';
import * as cmd from '../engine/commands';
import type { TableMeta } from '../engine/types';
import { mutate } from '../store/sync';
import { cellCache, useTableVersion } from '../store/table';
import { commitOf, formulaBarReducer, INITIAL_BAR, rawCellInput } from './formulaBarState';
import { activeRange } from './selection';
import { useGrid, useSelection } from './store';

/**
 * A cell address in the formula language: `B3` for a single-subcolumn data set, `A.2[3]` for
 * subcolumn 2 of data set A, `X3` for the X column.
 */
export function addressOf(meta: TableMeta, row: number, col: number): string {
  const c = meta.columns[col];
  if (!c) return '';
  if (c.role === 'x') return `X${row + 1}`;
  const ds = c.dataset !== null ? meta.datasets[c.dataset] : undefined;
  const letter = ds?.letter ?? c.label.replace(/:.*/, '');
  if (ds && ds.subcolumns > 1) return `${letter}.${c.subcolumn + 1}[${row + 1}]`;
  return `${letter}${row + 1}`;
}

export function FormulaBar({ meta }: { meta: TableMeta }) {
  const tableId = meta.id;
  const sel = useSelection(tableId);
  const version = useTableVersion(tableId);
  const info = useGrid((s) => s.activeInfo);
  const storedMode = useGrid((s) => s.formulaMode[tableId]);
  const [state, dispatch] = useReducer(formulaBarReducer, INITIAL_BAR);
  const inputRef = useRef<HTMLInputElement>(null);
  const { row, col } = sel.anchor;
  const column = meta.columns[col];
  const hasColumnFormula = !!column?.formula;
  const mode = storedMode ?? (hasColumnFormula ? 'column' : 'cell');

  // Default the mode per column: column mode where the column has a formula.
  const lastCol = useRef(col);
  useEffect(() => {
    if (lastCol.current !== col) {
      lastCol.current = col;
      useGrid.getState().setFormulaMode(tableId, hasColumnFormula ? 'column' : 'cell');
    }
  }, [col, hasColumnFormula, tableId]);

  useEffect(() => {
    if (state.mode !== mode) dispatch({ type: 'mode', mode });
  }, [mode, state.mode]);

  useEffect(() => {
    const cf = info && info.table === tableId && info.row === row && info.col === col ? info.cell_formula : undefined;
    const cellText = row < meta.rows ? rawCellInput(cellCache.get(tableId, row, col), cf) : '';
    dispatch({ type: 'sync', cellText, columnText: column?.formula ? `=${column.formula}` : '' });
  }, [tableId, row, col, version, info, column?.formula, meta.rows]);

  const commit = async () => {
    const c = commitOf(state);
    dispatch({ type: 'committed' });
    if (!c || !column) return;
    if (c.kind === 'column') {
      await mutate((e) => cmd.setColumnFormula(e, tableId, col, c.expr));
    } else {
      await mutate((e) => cmd.setCell(e, tableId, row, col, c.input));
    }
  };

  const r = activeRange(sel);
  const multi = r.row1 - r.row0 > 1 || r.col1 - r.col0 > 1;
  const address = column
    ? multi
      ? `${addressOf(meta, r.row0, r.col0)}:${addressOf(meta, r.row1 - 1, Math.min(r.col1, meta.columns.length) - 1)}`
      : addressOf(meta, row, col)
    : '';
  const ro = meta.read_only;

  return (
    <div className="formula-bar" data-testid="formula-bar">
      <div className="fb-address mono" data-tip="Active cell or selection" data-testid="formula-address">
        {address}
      </div>
      <Segmented
        label="Formula bar edits"
        value={mode}
        choices={[
          { value: 'cell' as const, label: 'Cell', testId: 'formula-mode-cell', hint: 'Edit the active cell' },
          {
            value: 'column' as const,
            label: hasColumnFormula ? (
              <>
                Column <span className="seg-mark">=</span>
              </>
            ) : (
              'Column'
            ),
            testId: 'formula-mode-column',
            hint: 'One formula for every row of the column',
          },
        ]}
        onChange={(m) => {
          useGrid.getState().setFormulaMode(tableId, m);
          if (m === 'column') requestAnimationFrame(() => inputRef.current?.focus());
        }}
      />
      <span className="fb-well">
        <FormulaIcon size={16} />
        <input
          ref={inputRef}
          className="fb-input mono"
          data-testid="formula-input"
          aria-label={mode === 'column' ? `Formula for column ${column?.label ?? ''}` : `Content of ${address}`}
          spellCheck={false}
          autoComplete="off"
          disabled={!column || ro}
          placeholder={mode === 'column' ? `=formula for every row of ${column?.label ?? 'the column'}, e.g. =log10(A)` : ''}
          value={state.text}
          onChange={(e) => dispatch({ type: 'input', text: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void commit().then(() => (document.querySelector('[data-grid]') as HTMLElement | null)?.focus());
            } else if (e.key === 'Escape') {
              e.preventDefault();
              dispatch({ type: 'cancel' });
              (document.querySelector('[data-grid]') as HTMLElement | null)?.focus();
            }
            e.stopPropagation();
          }}
          onBlur={() => state.editing && dispatch({ type: 'cancel' })}
        />
      </span>
      {mode === 'column' && column?.formula && (
        <span className="fb-note">Computed for every row</span>
      )}
      <TableTools tableId={tableId} />
    </div>
  );
}
