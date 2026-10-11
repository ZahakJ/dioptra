// The work-area renderer for tables: formula bar over the grid.
import { useEffect } from 'react';
import type { WorkAreaProps } from '../app/workArea';
import { useTables, useTableMeta } from '../store/table';
import { FormulaBar } from './FormulaBar';
import { Grid } from './Grid';

export function TableView({ item }: WorkAreaProps) {
  const meta = useTableMeta(item.id);
  useEffect(() => {
    if (!useTables.getState().metas[item.id]) void useTables.getState().loadMeta(item.id);
  }, [item.id]);
  if (!meta) return <div className="loading-line">Loading {item.title}...</div>;
  return (
    <div className="table-view" data-testid={`table-view-${item.title}`}>
      <FormulaBar meta={meta} />
      <Grid meta={meta} />
    </div>
  );
}
