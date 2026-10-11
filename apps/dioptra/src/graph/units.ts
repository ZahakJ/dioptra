// Physical units of the graph editor. The engine's SVG and its target boxes are in points
// (1 pt = 1/72 in) in the viewBox; sizes are millimetres; the screen is CSS pixels at 96 per
// inch at 100% zoom ("actual size").
export const MM_PER_IN = 25.4;
export const PT_PER_IN = 72;
export const PX_PER_IN = 96;

export const mmToPt = (mm: number) => (mm / MM_PER_IN) * PT_PER_IN;
export const ptToMm = (pt: number) => (pt / PT_PER_IN) * MM_PER_IN;
export const mmToIn = (mm: number) => mm / MM_PER_IN;
export const inToMm = (inch: number) => inch * MM_PER_IN;

/** CSS pixels per point at a zoom factor (1 = actual size at 96 dpi). */
export const pxPerPt = (zoom: number) => (PX_PER_IN / PT_PER_IN) * zoom;
export const ptToPx = (pt: number, zoom: number) => pt * pxPerPt(zoom);
export const pxToPt = (px: number, zoom: number) => px / pxPerPt(zoom);

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A target box in points to screen pixels inside the scaled figure. */
export function boxToPx(b: Box, zoom: number): Box {
  const k = pxPerPt(zoom);
  return { x: b.x * k, y: b.y * k, width: b.width * k, height: b.height * k };
}

/** Zoom that fits a figure of `wMm` × `hMm` into a viewport of CSS pixels, with a margin. */
export function fitZoom(wMm: number, hMm: number, viewW: number, viewH: number, margin = 72): number {
  const wPx = (mmToPt(wMm) * PX_PER_IN) / PT_PER_IN;
  const hPx = (mmToPt(hMm) * PX_PER_IN) / PT_PER_IN;
  if (wPx <= 0 || hPx <= 0) return 1;
  const z = Math.min((viewW - margin) / wPx, (viewH - margin) / hPx);
  // Fitting never enlarges past 200%: beyond that the figure stops reading as a page.
  return clampZoom(Math.min(z, FIT_MAX));
}

export const FIT_MAX = 2;
export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 4;
export const clampZoom = (z: number) => Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, Number.isFinite(z) ? z : 1));

/** PNG pixel size as the engine computes it: round(mm / 25.4 × dpi). */
export const pngPixels = (mm: number, dpi: number) => Math.round((mm / MM_PER_IN) * dpi);

/** Round for display: at most `d` decimals, no trailing zeros. */
export function trim(v: number, d = 2): string {
  if (!Number.isFinite(v)) return '';
  return String(Math.round(v * 10 ** d) / 10 ** d);
}
