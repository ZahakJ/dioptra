// The virtualised spreadsheet over the engine's viewport cache. Only the visible cells (plus a
// small overscan) exist in the DOM; selection and the active cell are overlays, so moving the
// selection never re-renders the cells.
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { showContextMenu, type MenuEntry } from '../app/ContextMenu';
import { PlusIcon } from '../app/icons';
import { isMac } from '../app/keys';
import * as cmd from '../engine/commands';
import type { CellView, Id, TableMeta } from '../engine/types';
import { withEngine } from '../store/engine';
import { notify } from '../store/notices';
import { mutate } from '../store/sync';
import { cellCache, useTables, useTableVersion } from '../store/table';
import { useUi } from '../store/ui';
import * as actions from './actions';
import { rawCellInput } from './formulaBarState';
import { navigate, type NavContext } from './navigation';
import {
  activeRange,
  addCell,
  allRanges,
  extendTo,
  inRange,
  selectCell,
  type CellPos,
  type Selection,
} from './selection';
import { useGrid, useSelection } from './store';

export const ROW_H = 24;
export const HEAD_H = 22;
const ROWNUM_W = 52;
const TITLE_W = 132;
const SPARE_ROWS = 30;
const MIN_ROWS = 60;
const TAIL_W = 112;

export function defaultColWidth(meta: TableMeta, col: number): number {
  const c = meta.columns[col];
  if (!c) return 96;
  if (c.role === 'x') return 88;
  if (meta.kind === 'variables') return 112;
  if (c.format && c.format.kind !== 'single' && !(c.format.kind === 'values' && c.format.n === 1)) return 84;
  return 96;
}

/** The second header row's text: the subcolumn part of the label, or the variable type. */
function subLabel(meta: TableMeta, col: number): string {
  const c = meta.columns[col]!;
  if (meta.kind === 'variables') return c.var_type ?? 'number';
  const i = c.label.indexOf(':');
  return i >= 0 ? c.label.slice(i + 1) : c.label;
}

const isFilled = (c: CellView | undefined) => !!c && (c.v !== undefined || c.t !== undefined);

interface EditState {
  row: number;
  col: number;
  text: string;
  /** `type`: started by typing (arrows commit and move); `edit`: F2/Enter/double-click. */
  mode: 'type' | 'edit';
}

export function Grid({ meta }: { meta: TableMeta }) {
  const tableId = meta.id;
  const version = useTableVersion(tableId);
  const sel = useSelection(tableId);
  const widths = useUi((s) => s.colWidths[tableId]);
  const setColWidth = useUi((s) => s.setColWidth);
  const editRequest = useGrid((s) => s.editRequest);
  const scrollRef = useRef<HTMLDivElement>(null);
  const cellsRef = useRef<HTMLDivElement>(null);
  const [editing, setEditing] = useState<EditState | null>(null);
  const [titleEdit, setTitleEdit] = useState<number | null>(null);

  const ncols = meta.columns.length;
  const hasTitles = meta.has_row_titles;
  const leftW = ROWNUM_W + (hasTitles ? TITLE_W : 0);
  const headerH = HEAD_H * 2;
  const displayRows = Math.max(meta.rows + SPARE_ROWS, MIN_ROWS);
  const colWidth = useCallback((i: number) => widths?.[i] ?? defaultColWidth(meta, i), [widths, meta]);

  const rowV = useVirtualizer({
    count: displayRows,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: 10,
    scrollMargin: headerH,
  });
  const colV = useVirtualizer({
    horizontal: true,
    count: ncols,
    getScrollElement: () => scrollRef.current,
    estimateSize: colWidth,
    overscan: 3,
    scrollMargin: leftW,
  });
  useLayoutEffect(() => {
    colV.measure();
  }, [colWidth, colV]);

  const vRows = rowV.getVirtualItems();
  const vCols = colV.getVirtualItems();
  const bodyW = colV.getTotalSize();
  const bodyH = rowV.getTotalSize();
  // Up to date: getVirtualItems/getTotalSize above recompute it.
  const meas = colV.measurementsCache;
  const colLeft = useCallback((c: number) => (meas[c]?.start ?? leftW) - leftW, [meas, leftW]);
  const colRight = useCallback((c: number) => (meas[c]?.end ?? leftW) - leftW, [meas, leftW]);
  const r0 = vRows[0]?.index ?? 0;
  const r1 = (vRows[vRows.length - 1]?.index ?? -1) + 1;
  const c0 = vCols[0]?.index ?? 0;
  const c1 = (vCols[vCols.length - 1]?.index ?? -1) + 1;

  // Fetch the visible blocks (and refetch stale ones after a change).
  useEffect(() => {
    useTables.getState().ensureCells(tableId, { row0: r0, row1: r1, col0: c0, col1: c1 });
  }, [tableId, r0, r1, c0, c1, version]);

  // Provenance of the active cell for the inspector and the formula bar.
  const { row: ar, col: ac } = sel.anchor;
  useEffect(() => {
    if (ar >= meta.rows || ac >= ncols) {
      useGrid.getState().setActiveInfo(null);
      return;
    }
    const t = setTimeout(() => {
      withEngine((e) => cmd.getCellInfo(e, tableId, ar, ac))
        .then((info) => useGrid.getState().setActiveInfo(info))
        .catch(() => useGrid.getState().setActiveInfo(null));
    }, 30);
    return () => clearTimeout(t);
  }, [tableId, ar, ac, version, meta.rows, ncols]);

  const setSel = useCallback((s: Selection) => useGrid.getState().setSelection(tableId, s), [tableId]);

  const scrollIntoView = useCallback(
    (p: CellPos) => {
      const el = scrollRef.current;
      if (!el) return;
      const top = p.row * ROW_H;
      const viewH = el.clientHeight - headerH;
      if (top < el.scrollTop) el.scrollTop = top;
      else if (top + ROW_H > el.scrollTop + viewH) el.scrollTop = top + ROW_H - viewH;
      if (p.col < ncols) {
        const l = colLeft(p.col);
        const r = colRight(p.col);
        const viewW = el.clientWidth - leftW;
        if (l < el.scrollLeft) el.scrollLeft = l;
        else if (r > el.scrollLeft + viewW) el.scrollLeft = Math.min(l, r - viewW);
      }
    },
    [headerH, leftW, ncols, colLeft, colRight],
  );

  const focusGrid = () => scrollRef.current?.focus({ preventScroll: true });

  // A freshly opened table takes the keyboard unless something else has it.
  useEffect(() => {
    const a = document.activeElement;
    if (!a || a === document.body) scrollRef.current?.focus({ preventScroll: true });
  }, [tableId]);

  // ---- editing ----

  const rawAt = useCallback(
    (row: number, col: number) => {
      const info = useGrid.getState().activeInfo;
      const cf = info && info.table === tableId && info.row === row && info.col === col ? info.cell_formula : undefined;
      return rawCellInput(cellCache.get(tableId, row, col), cf);
    },
    [tableId],
  );

  const beginEdit = useCallback(
    (mode: EditState['mode'], initial?: string) => {
      if (meta.read_only) {
        notify('This table is derived from another table and is read-only.');
        return;
      }
      const { row, col } = useGrid.getState().selections[tableId]?.anchor ?? { row: 0, col: 0 };
      if (col >= ncols) return;
      scrollIntoView({ row, col });
      setEditing({ row, col, text: initial ?? rawAt(row, col), mode });
    },
    [meta.read_only, tableId, ncols, rawAt, scrollIntoView],
  );

  useEffect(() => {
    // Only new requests count; beginEdit is read at that moment.
    if (editRequest > 0 && document.activeElement === scrollRef.current) beginEdit('edit');
  }, [editRequest]); // eslint-disable-line react-hooks/exhaustive-deps

  const commitEdit = useCallback(
    (ed: EditState, move: { dr: number; dc: number } | null) => {
      setEditing(null);
      const original = rawAt(ed.row, ed.col);
      if (ed.text !== original) {
        void mutate((e) => cmd.setCell(e, tableId, ed.row, ed.col, ed.text));
      }
      if (move) {
        const next = {
          row: Math.max(0, Math.min(displayRows - 1, ed.row + move.dr)),
          col: Math.max(0, Math.min(ncols - 1, ed.col + move.dc)),
        };
        setSel(selectCell(next));
        scrollIntoView(next);
      }
      focusGrid();
    },
    [rawAt, tableId, displayRows, ncols, setSel, scrollIntoView],
  );

  // ---- keyboard ----

  const navCtx: NavContext = useMemo(
    () => ({
      rows: displayRows,
      cols: ncols,
      dataRows: meta.rows,
      pageRows: Math.max(1, Math.floor(((scrollRef.current?.clientHeight ?? 600) - headerH) / ROW_H) - 1),
      filled: (r, c) => (r >= meta.rows ? false : cellCache.get(tableId, r, c) === undefined ? undefined : isFilled(cellCache.get(tableId, r, c))),
    }),
    // `version` is a dependency on purpose: the cached data the accessor reads changed.
    [displayRows, ncols, meta.rows, tableId, headerH, version], // eslint-disable-line react-hooks/exhaustive-deps
  );

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.target !== e.currentTarget || editing) return;
    const mod = isMac() ? e.metaKey : e.ctrlKey;
    if ((e.key === 'Enter' && !e.shiftKey && !mod && !e.altKey) || e.key === 'F2') {
      e.preventDefault();
      beginEdit('edit');
      return;
    }
    if (e.key === 'Escape') {
      if (sel.extra.length || sel.anchor.row !== sel.focus.row || sel.anchor.col !== sel.focus.col) {
        e.preventDefault();
        setSel(selectCell(sel.anchor));
      }
      return;
    }
    if (e.altKey && !mod) return;
    const next = navigate(sel, { key: e.key, shift: e.shiftKey, mod }, navCtx);
    if (next) {
      e.preventDefault();
      setSel(next);
      scrollIntoView(e.shiftKey && e.key !== 'Tab' && e.key !== 'Enter' ? next.focus : next.anchor);
      return;
    }
    if (e.key.length === 1 && !mod && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      beginEdit('type', e.key);
    }
  };

  // ---- clipboard ----

  const onCopy = (e: React.ClipboardEvent) => {
    if (e.target !== e.currentTarget || editing) return;
    e.preventDefault();
    const tsv = actions.selectionTsvSync();
    if (tsv !== null) e.clipboardData.setData('text/plain', tsv);
    else void actions.copySelection();
  };
  const onCut = (e: React.ClipboardEvent) => {
    onCopy(e);
    if (e.target === e.currentTarget && !editing) void actions.clearSelection();
  };
  const onPaste = (e: React.ClipboardEvent) => {
    if (e.target !== e.currentTarget || editing) return;
    e.preventDefault();
    const text = e.clipboardData.getData('text/plain');
    if (text) void actions.pasteText(text);
  };

  // ---- mouse ----

  const cellAt = useCallback(
    (clientX: number, clientY: number): CellPos => {
      const rect = cellsRef.current!.getBoundingClientRect();
      const y = clientY - rect.top;
      const x = clientX - rect.left;
      const row = Math.max(0, Math.min(displayRows - 1, Math.floor(y / ROW_H)));
      let lo = 0;
      let hi = ncols - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (colLeft(mid) <= x) lo = mid;
        else hi = mid - 1;
      }
      return { row, col: Math.max(0, lo) };
    },
    [displayRows, ncols, colLeft],
  );

  const drag = useCallback(
    (onMove: (e: MouseEvent) => void) => {
      const el = scrollRef.current;
      const move = (e: MouseEvent) => {
        if (el) {
          const r = el.getBoundingClientRect();
          if (e.clientY > r.bottom - 8) el.scrollTop += ROW_H;
          else if (e.clientY < r.top + headerH + 4) el.scrollTop -= ROW_H;
          if (e.clientX > r.right - 8) el.scrollLeft += 32;
          else if (e.clientX < r.left + leftW + 4) el.scrollLeft -= 32;
        }
        onMove(e);
      };
      const up = () => {
        window.removeEventListener('mousemove', move);
        window.removeEventListener('mouseup', up);
      };
      window.addEventListener('mousemove', move);
      window.addEventListener('mouseup', up);
    },
    [headerH, leftW],
  );

  const onCellsMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('.cell-editor')) return;
    const pos = cellAt(e.clientX, e.clientY);
    if (e.button === 2) {
      if (!allRanges(sel).some((r) => inRange(r, pos.row, pos.col))) setSel(selectCell(pos));
      return;
    }
    if (e.button !== 0) return;
    e.preventDefault();
    focusGrid();
    const mod = isMac() ? e.metaKey : e.ctrlKey;
    let base: Selection;
    if (e.shiftKey) base = extendTo(sel, pos);
    else if (mod) base = addCell(sel, pos);
    else base = selectCell(pos);
    setSel(base);
    drag((m) => {
      const p = cellAt(m.clientX, m.clientY);
      const cur = useGrid.getState().selections[tableId] ?? base;
      if (cur.focus.row !== p.row || cur.focus.col !== p.col) setSel(extendTo(cur, p));
    });
  };

  const selectCols = (e: React.MouseEvent, first: number, last: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    focusGrid();
    const lastRow = Math.max(0, meta.rows - 1);
    const anchorCol = e.shiftKey ? sel.anchor.col : first;
    const base: Selection = { anchor: { row: 0, col: anchorCol }, focus: { row: lastRow, col: last }, extra: [] };
    setSel(base);
    drag((m) => {
      const p = cellAt(m.clientX, m.clientY);
      setSel({ ...base, focus: { row: lastRow, col: p.col } });
    });
  };

  const selectRowsFrom = (e: React.MouseEvent, row: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    focusGrid();
    const lastCol = Math.max(0, ncols - 1);
    const anchorRow = e.shiftKey ? sel.anchor.row : row;
    const base: Selection = { anchor: { row: anchorRow, col: 0 }, focus: { row, col: lastCol }, extra: [] };
    setSel(base);
    drag((m) => {
      const p = cellAt(m.clientX, m.clientY);
      setSel({ ...base, focus: { row: p.row, col: lastCol } });
    });
  };

  const startResize = (e: React.MouseEvent, col: number) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startW = colWidth(col);
    const move = (m: MouseEvent) => setColWidth(tableId, col, startW + m.clientX - startX);
    const up = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
      document.body.classList.remove('is-resizing');
    };
    document.body.classList.add('is-resizing');
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  };

  // ---- context menus ----

  const cellMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    const ro = meta.read_only;
    const items: MenuEntry[] = [
      { label: 'Cut', shortcut: 'Mod+X', disabled: ro, run: actions.cutSelection },
      { label: 'Copy', shortcut: 'Mod+C', run: actions.copySelection },
      { label: 'Paste', shortcut: 'Mod+V', disabled: ro, run: actions.pasteFromClipboard },
      'separator',
      { label: 'Insert rows above', shortcut: 'Mod+Alt+I', disabled: ro, run: () => actions.insertRows('above') },
      { label: 'Insert rows below', disabled: ro, run: () => actions.insertRows('below') },
      { label: 'Delete rows', shortcut: 'Mod+Alt+-', disabled: ro, run: actions.deleteRows },
      'separator',
      { label: 'Add data set', shortcut: 'Mod+Alt+D', disabled: ro, run: actions.addDataset },
      { label: 'Rename data set...', disabled: ro, run: () => actions.renameDataset() },
      { label: 'Decimals...', disabled: ro, run: () => actions.setDecimals() },
      'separator',
      { label: 'Fill down', shortcut: 'Mod+D', disabled: ro, run: actions.fillDownSelection },
      { label: 'Exclude or include', shortcut: 'Mod+E', disabled: ro, run: actions.toggleExclude },
      { label: 'Clear', shortcut: 'Delete', disabled: ro, run: actions.clearSelection },
      'separator',
      {
        label: 'Cell info',
        run: () => {
          const ui = useUi.getState();
          if (!ui.inspectorVisible) ui.toggleInspector();
          requestAnimationFrame(() => document.querySelector('[data-testid="insp-cell"]')?.scrollIntoView({ block: 'nearest' }));
        },
      },
    ];
    showContextMenu(e.clientX, e.clientY, items, 'Cell');
  };

  const headerMenu = (e: React.MouseEvent, col: number) => {
    e.preventDefault();
    if (!(sel.anchor.col <= col && col <= sel.focus.col) && !(sel.focus.col <= col && col <= sel.anchor.col)) {
      setSel({ anchor: { row: 0, col }, focus: { row: Math.max(0, meta.rows - 1), col }, extra: [] });
    }
    const c = meta.columns[col]!;
    const ro = meta.read_only;
    const items: MenuEntry[] = [];
    if (c.dataset !== null) {
      items.push({ label: meta.kind === 'variables' ? 'Rename variable...' : 'Rename data set...', disabled: ro, run: () => actions.renameDataset(col) });
      if (meta.kind === 'variables') items.push({ label: 'Variable type...', disabled: ro, run: () => actions.setVariableType(col) });
    }
    items.push(
      { label: 'Subcolumn format...', disabled: ro || meta.kind === 'variables', run: actions.setSubcolumnFormat },
      { label: 'Decimals...', disabled: ro, run: () => actions.setDecimals(col) },
      'separator',
      { label: c.formula ? 'Edit column formula...' : 'Set column formula...', disabled: ro, run: () => actions.setColumnFormula(col) },
      { label: 'Clear column formula', disabled: ro || !c.formula, run: () => actions.clearColumnFormula(col) },
      'separator',
      { label: 'Add data set', disabled: ro, run: actions.addDataset },
      { label: 'Remove data set', disabled: ro || c.dataset === null, run: () => actions.removeDataset(col) },
    );
    showContextMenu(e.clientX, e.clientY, items, 'Column');
  };

  // ---- render ----

  const ar0 = activeRange(sel);
  const ranges = allRanges(sel);
  const visibleDatasets = useMemo(() => {
    const out: { key: string; name: string; first: number; last: number; xcol: boolean }[] = [];
    if (c1 <= c0) return out;
    if (meta.has_x && c0 === 0) out.push({ key: 'x', name: meta.columns[0]!.dataset_name || 'X', first: 0, last: 0, xcol: true });
    for (let d = 0; d < meta.datasets.length; d++) {
      const ds = meta.datasets[d]!;
      const first = ds.first_col;
      const last = ds.first_col + ds.subcolumns - 1;
      if (last < c0 || first >= c1) continue;
      out.push({ key: `d${d}`, name: ds.name, first, last, xcol: false });
    }
    return out;
  }, [meta, c0, c1]);

  const rowTitle = (r: number) => meta.row_titles[r] ?? '';

  return (
    <div
      ref={scrollRef}
      className="grid"
      data-grid
      data-testid="grid"
      tabIndex={0}
      role="grid"
      aria-label={`${meta.title} data`}
      aria-rowcount={meta.rows}
      aria-colcount={ncols}
      aria-multiselectable="true"
      onKeyDown={onKeyDown}
      onCopy={onCopy}
      onCut={onCut}
      onPaste={onPaste}
    >
      <div className="grid-canvas" style={{ width: leftW + bodyW + TAIL_W, height: headerH + bodyH }}>
        {/* Header */}
        <div className="grid-head" style={{ height: headerH, width: leftW + bodyW + TAIL_W }}>
          <div className="grid-corner" style={{ width: leftW, height: headerH }}>
            <button
              type="button"
              className="corner-btn"
              aria-label="Select all"
              data-tip="Select all"
              tabIndex={-1}
              onMouseDown={(e) => {
                e.preventDefault();
                focusGrid();
                actions.selectAllCells();
              }}
            />
            {hasTitles && (
              <div className="corner-titles" style={{ left: ROWNUM_W, width: TITLE_W }}>
                {meta.kind === 'xy' ? 'Row title' : 'Row titles'}
              </div>
            )}
          </div>
          <div className="grid-head-cols" style={{ width: bodyW, height: headerH }}>
            {visibleDatasets.map((g) => (
              <div
                key={g.key}
                className={'gh-group' + (g.xcol ? ' is-x' : '') + (ar0.col0 <= g.first && ar0.col1 > g.last ? ' is-sel' : '')}
                style={{ left: colLeft(g.first), width: colRight(g.last) - colLeft(g.first), height: HEAD_H }}
                data-tip={g.name}
                onMouseDown={(e) => selectCols(e, g.first, g.last)}
                onContextMenu={(e) => headerMenu(e, g.first)}
                onDoubleClick={() => !g.xcol && void actions.renameDataset(g.first)}
              >
                <span className="gh-text">{g.name}</span>
              </div>
            ))}
            {vCols.map((vc) => {
              const c = meta.columns[vc.index]!;
              const selected = vc.index >= ar0.col0 && vc.index < ar0.col1;
              return (
                <div
                  key={vc.key}
                  className={'gh-col' + (selected ? ' is-sel' : '') + (c.role === 'x' ? ' is-x' : '')}
                  style={{ left: vc.start - leftW, width: vc.size, top: HEAD_H, height: HEAD_H }}
                  data-testid={`col-head-${vc.index}`}
                  data-tip={c.formula ? `${c.header}  =${c.formula}` : c.header}
                  onMouseDown={(e) => selectCols(e, vc.index, vc.index)}
                  onContextMenu={(e) => headerMenu(e, vc.index)}
                >
                  {c.formula && <span className="gh-fx" aria-label="column formula">fx</span>}
                  <span className="gh-text">{subLabel(meta, vc.index)}</span>
                  <span
                    className="col-resize"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label={`Resize column ${c.label}`}
                    onMouseDown={(e) => startResize(e, vc.index)}
                    onDoubleClick={(e) => {
                      e.stopPropagation();
                      setColWidth(tableId, vc.index, defaultColWidth(meta, vc.index));
                    }}
                  />
                </div>
              );
            })}
          </div>
          <div className="grid-tail" style={{ width: TAIL_W, height: headerH }}>
            {!meta.read_only && (
              <button
                type="button"
                className="btn btn-quiet btn-sm"
                tabIndex={-1}
                onClick={() => void actions.addDataset()}
                data-testid="add-dataset"
                data-tip={meta.kind === 'variables' ? 'Add a variable' : 'Add a data set'}
              >
                <PlusIcon size={16} /> {meta.kind === 'variables' ? 'Variable' : 'Data set'}
              </button>
            )}
          </div>
        </div>

        {/* Body */}
        <div className="grid-body" style={{ height: bodyH, width: leftW + bodyW + TAIL_W }}>
          <div className="grid-rowheads" style={{ width: leftW, height: bodyH }}>
            {vRows.map((vr) => {
              const selected = vr.index >= ar0.row0 && vr.index < ar0.row1;
              const spare = vr.index >= meta.rows;
              return (
                <div key={vr.key} className="rh-row" style={{ top: vr.start - headerH, height: ROW_H }}>
                  <div
                    className={'rh-num' + (selected ? ' is-sel' : '') + (spare ? ' is-spare' : '')}
                    style={{ width: ROWNUM_W }}
                    onMouseDown={(e) => selectRowsFrom(e, vr.index)}
                  >
                    {vr.index + 1}
                  </div>
                  {hasTitles &&
                    (titleEdit === vr.index ? (
                      <RowTitleEditor
                        initial={rowTitle(vr.index)}
                        width={TITLE_W}
                        done={(v) => {
                          setTitleEdit(null);
                          focusGrid();
                          if (v !== null && v !== rowTitle(vr.index)) {
                            void mutate((en) => cmd.setRowTitle(en, tableId, vr.index, v));
                          }
                        }}
                      />
                    ) : (
                      <div
                        className={'rh-title' + (spare ? ' is-spare' : '')}
                        style={{ width: TITLE_W }}
                        data-tip="Double-click to edit the row title"
                        data-testid={`row-title-${vr.index}`}
                        onDoubleClick={() => !meta.read_only && setTitleEdit(vr.index)}
                      >
                        {rowTitle(vr.index)}
                      </div>
                    ))}
                </div>
              );
            })}
          </div>
          <div
            ref={cellsRef}
            className="grid-cells"
            style={{ width: bodyW, height: bodyH }}
            onMouseDown={onCellsMouseDown}
            onDoubleClick={(e) => {
              if ((e.target as HTMLElement).closest('.cell-editor')) return;
              beginEdit('edit');
            }}
            onContextMenu={cellMenu}
          >
            <GridCells
              tableId={tableId}
              version={version}
              vRows={vRows}
              vCols={vCols}
              headerH={headerH}
              leftW={leftW}
              dataRows={meta.rows}
            />
            {ranges.map((r, i) => {
              const rr = { ...r, col1: Math.min(r.col1, ncols) };
              if (rr.col1 <= rr.col0) return null;
              const single = r.row1 - r.row0 === 1 && r.col1 - r.col0 === 1;
              if (single && i === ranges.length - 1) return null;
              return (
                <div
                  key={i}
                  className="sel-range"
                  style={{
                    top: rr.row0 * ROW_H,
                    height: (rr.row1 - rr.row0) * ROW_H,
                    left: colLeft(rr.col0),
                    width: colRight(rr.col1 - 1) - colLeft(rr.col0),
                  }}
                />
              );
            })}
            {sel.anchor.col < ncols && (
              <div
                className="sel-active"
                data-testid="active-cell"
                data-row={sel.anchor.row}
                data-col={sel.anchor.col}
                style={{
                  top: sel.anchor.row * ROW_H,
                  height: ROW_H,
                  left: colLeft(sel.anchor.col),
                  width: colRight(sel.anchor.col) - colLeft(sel.anchor.col),
                }}
              />
            )}
            {editing && (
              <CellEditor
                key={`${editing.row}:${editing.col}`}
                state={editing}
                left={colLeft(editing.col)}
                top={editing.row * ROW_H}
                width={colRight(editing.col) - colLeft(editing.col)}
                onChange={(text) => setEditing((s) => (s ? { ...s, text } : s))}
                onCommit={(s, move) => commitEdit(s, move)}
                onCancel={() => {
                  setEditing(null);
                  focusGrid();
                }}
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

interface CellsProps {
  tableId: Id;
  version: number;
  vRows: VirtualItem[];
  vCols: VirtualItem[];
  headerH: number;
  leftW: number;
  dataRows: number;
}

/** The visible cells. Re-renders only when the viewport or the cached data change. */
const GridCells = memo(
  function GridCells({ tableId, vRows, vCols, headerH, leftW, dataRows }: CellsProps) {
    const out: React.ReactElement[] = [];
    for (const vr of vRows) {
      const top = vr.start - headerH;
      const spare = vr.index >= dataRows;
      for (const vc of vCols) {
        const cell = spare ? undefined : cellCache.get(tableId, vr.index, vc.index);
        const left = vc.start - leftW;
        let cls = 'gc';
        let text = '';
        let title: string | undefined;
        if (cell) {
          text = cell.d ?? cell.t ?? '';
          if (cell.t !== undefined && cell.v === undefined && !cell.e) cls += ' gc-text';
          if (cell.x) cls += ' gc-x';
          if (cell.f) cls += ' gc-f';
          if (cell.e) {
            cls += ' gc-e';
            title = cell.e;
          }
        }
        if (spare) cls += ' gc-spare';
        out.push(
          <div
            key={`${vr.index}:${vc.index}`}
            className={cls}
            style={{ top, left, width: vc.size }}
            data-cell={`${vr.index}:${vc.index}`}
            data-tip={title}
          >
            {text}
          </div>,
        );
      }
    }
    return <>{out}</>;
  },
  (a, b) =>
    a.tableId === b.tableId &&
    a.version === b.version &&
    a.headerH === b.headerH &&
    a.leftW === b.leftW &&
    a.dataRows === b.dataRows &&
    sameItems(a.vRows, b.vRows) &&
    sameItems(a.vCols, b.vCols),
);

function sameItems(a: VirtualItem[], b: VirtualItem[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    const x = a[i]!;
    const y = b[i]!;
    if (x.index !== y.index || x.start !== y.start || x.size !== y.size) return false;
  }
  return true;
}

function CellEditor({
  state,
  left,
  top,
  width,
  onChange,
  onCommit,
  onCancel,
}: {
  state: EditState;
  left: number;
  top: number;
  width: number;
  onChange: (text: string) => void;
  onCommit: (s: EditState, move: { dr: number; dc: number } | null) => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  const latest = useRef(state);
  latest.current = state;
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    const n = el.value.length;
    el.setSelectionRange(n, n);
  }, []);
  const finish = (move: { dr: number; dc: number } | null) => {
    if (done.current) return;
    done.current = true;
    onCommit(latest.current, move);
  };
  return (
    <input
      ref={ref}
      className="cell-editor"
      data-testid="cell-editor"
      value={state.text}
      spellCheck={false}
      autoComplete="off"
      aria-label="Cell value"
      style={{ left, top, width: Math.max(width, Math.min(480, 16 + state.text.length * 7.5)), height: ROW_H }}
      onChange={(e) => onChange(e.target.value)}
      onBlur={() => finish(null)}
      onKeyDown={(e) => {
        const k = e.key;
        if (k === 'Enter') {
          e.preventDefault();
          finish({ dr: e.shiftKey ? -1 : 1, dc: 0 });
        } else if (k === 'Tab') {
          e.preventDefault();
          finish({ dr: 0, dc: e.shiftKey ? -1 : 1 });
        } else if (k === 'Escape') {
          e.preventDefault();
          done.current = true;
          onCancel();
        } else if (state.mode === 'type' && (k === 'ArrowUp' || k === 'ArrowDown' || k === 'ArrowLeft' || k === 'ArrowRight')) {
          e.preventDefault();
          finish({ dr: k === 'ArrowUp' ? -1 : k === 'ArrowDown' ? 1 : 0, dc: k === 'ArrowLeft' ? -1 : k === 'ArrowRight' ? 1 : 0 });
        }
        e.stopPropagation();
      }}
    />
  );
}

function RowTitleEditor({ initial, width, done }: { initial: string; width: number; done: (v: string | null) => void }) {
  const [v, setV] = useState(initial);
  const finished = useRef(false);
  const finish = (val: string | null) => {
    if (finished.current) return;
    finished.current = true;
    done(val);
  };
  return (
    <input
      autoFocus
      className="cell-editor row-title-editor"
      style={{ position: 'relative', width, height: ROW_H }}
      value={v}
      aria-label="Row title"
      onChange={(e) => setV(e.target.value)}
      onBlur={() => finish(v)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          finish(v);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          finish(null);
        }
        e.stopPropagation();
      }}
    />
  );
}
