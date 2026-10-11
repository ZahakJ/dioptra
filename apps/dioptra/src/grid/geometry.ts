// The grid's geometry as numbers: how many rows the bounded sheet shows, the row and header
// heights of each density, and the colour that marks each data set. Pure, so it is tested
// without a DOM; the virtualiser takes the row count, nothing is materialised beyond the view.
import type { Density } from '../store/ui';

/** Empty rows kept below the last row with data. */
export const SPARE_ROWS = 12;
/** A sheet never shows fewer rows than this. */
export const MIN_ROWS = 20;
/** Rows the "+ rows" affordance adds to the sheet. */
export const ADD_ROWS_STEP = 20;

/**
 * Rows of the bounded sheet: the data's rows plus `SPARE_ROWS`, never fewer than `MIN_ROWS`,
 * plus any rows the user added with "+ rows" (`extra`, a multiple of `ADD_ROWS_STEP`).
 */
export function sheetRows(dataRows: number, extra = 0): number {
  const data = Math.max(0, Math.floor(dataRows || 0));
  const more = Math.max(0, Math.floor(extra || 0));
  return Math.max(data + SPARE_ROWS, MIN_ROWS) + more;
}

export const ROW_HEIGHTS: Record<Density, number> = { comfortable: 28, compact: 22 };
export const HEAD_HEIGHTS: Record<Density, number> = { comfortable: 26, compact: 22 };

export const rowHeight = (d: Density | undefined) => ROW_HEIGHTS[d === 'compact' ? 'compact' : 'comfortable'];
export const headHeight = (d: Density | undefined) => HEAD_HEIGHTS[d === 'compact' ? 'compact' : 'comfortable'];

/** Okabe–Ito, the default graph palette, in the order the engine assigns it to data sets. */
export const OKABE_ITO = ['#000000', '#E69F00', '#56B4E9', '#009E73', '#0072B2', '#D55E00', '#CC79A7', '#F0E442'];

/** The colour a graph of this table gives data set `index` by default. */
export function datasetColour(index: number, palette: readonly string[] = OKABE_ITO): string {
  if (palette.length === 0) return OKABE_ITO[0]!;
  const i = ((Math.floor(index) % palette.length) + palette.length) % palette.length;
  return palette[i]!;
}
