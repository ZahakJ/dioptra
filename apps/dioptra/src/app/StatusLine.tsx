// The status line: selection summary (cells, n, mean, SD, sum), recompute state, engine.
import { useEffect, useState } from 'react';
import type { Range } from '../engine/types';
import { allRanges, clipRange } from '../grid/selection';
import { formatStat, summarize, type SelectionSummary } from '../grid/stats';
import { useSelection } from '../grid/store';
import { useActiveItem } from '../store/project';
import { useEngineStatus } from '../store/status';
import { cellCache, useTableMeta, useTables, useTableVersion } from '../store/table';

const MAX_FETCH_CELLS = 250_000;

function useSummary(): { summary: SelectionSummary | null; partial: boolean } {
  const item = useActiveItem();
  const tableId = item?.kind === 'table' ? item.id : null;
  const meta = useTableMeta(tableId);
  const sel = useSelection(tableId);
  const version = useTableVersion(tableId);
  const [state, setState] = useState<{ summary: SelectionSummary | null; partial: boolean }>({ summary: null, partial: false });
  useEffect(() => {
    if (!meta || !tableId) {
      setState({ summary: null, partial: false });
      return;
    }
    const ranges = allRanges(sel)
      .map((r) => clipRange(r, meta.rows, meta.columns.length))
      .filter((r) => r.row1 > r.row0 && r.col1 > r.col0);
    if (ranges.every((r) => cellCache.covers(tableId, r))) {
      setState({ summary: summarize(ranges, (r, c) => cellCache.get(tableId, r, c)), partial: false });
      return;
    }
    const total = ranges.reduce((n, r) => n + (r.row1 - r.row0) * (r.col1 - r.col0), 0);
    if (total > MAX_FETCH_CELLS) {
      setState({ summary: { cells: total, n: 0, sum: 0, mean: null, sd: null }, partial: true });
      return;
    }
    let cancelled = false;
    const t = setTimeout(async () => {
      const fetched = new Map<Range, Awaited<ReturnType<ReturnType<typeof useTables.getState>['fetchRange']>>>();
      try {
        for (const r of ranges) fetched.set(r, await useTables.getState().fetchRange(tableId, r));
      } catch {
        return;
      }
      if (cancelled) return;
      const get = (row: number, col: number) => {
        for (const [r, rows] of fetched) {
          if (row >= r.row0 && row < r.row1 && col >= r.col0 && col < r.col1) return rows[row - r.row0]?.[col - r.col0];
        }
        return undefined;
      };
      setState({ summary: summarize(ranges, get), partial: false });
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [meta, tableId, sel, version]);
  return state;
}

export function StatusLine() {
  const status = useEngineStatus();
  const { summary, partial } = useSummary();
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (status.pending === 0) {
      setBusy(false);
      return;
    }
    const t = setTimeout(() => setBusy(true), 150);
    return () => clearTimeout(t);
  }, [status.pending]);

  return (
    <footer className="statusline" data-testid="statusline">
      <div className="sl-summary" aria-live="polite" data-testid="selection-summary">
        {summary && summary.cells > 0 && (
          <>
            <span className="sl-item">
              <span className="sl-k">Cells</span> {summary.cells.toLocaleString('en-US')}
            </span>
            {partial ? (
              <span className="sl-item muted">Selection too large to summarise</span>
            ) : (
              <>
                <span className="sl-item" data-testid="sum-n">
                  <span className="sl-k">n</span> {summary.n.toLocaleString('en-US')}
                </span>
                {summary.n > 0 && (
                  <>
                    <span className="sl-item" data-testid="sum-mean">
                      <span className="sl-k">Mean</span> {formatStat(summary.mean)}
                    </span>
                    <span className="sl-item" data-testid="sum-sd">
                      <span className="sl-k">SD</span> {formatStat(summary.sd)}
                    </span>
                    <span className="sl-item" data-testid="sum-sum">
                      <span className="sl-k">Sum</span> {formatStat(summary.sum)}
                    </span>
                  </>
                )}
              </>
            )}
          </>
        )}
      </div>
      <div className="sl-right">
        <span className="sl-item" data-testid="recompute-state">
          {!status.ready ? (status.error ? 'Engine failed to load' : 'Loading the engine...') : busy ? 'Computing...' : 'Up to date'}
        </span>
        {status.ready && (
          <span className="sl-item muted" data-testid="engine-backend">
            {status.backend === 'tauri' ? 'Native engine' : 'WebAssembly engine'} {status.version ?? ''}
          </span>
        )}
      </div>
    </footer>
  );
}
