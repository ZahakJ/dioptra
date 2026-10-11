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
/** Low enough that Fit shows a double-column layout whole beside both panels at 1024 px. */
export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 4;
export const clampZoom = (z: number) => Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, Number.isFinite(z) ? z : 1));

/** PNG pixel size as the engine computes it: round(mm / 25.4 × dpi). */
export const pngPixels = (mm: number, dpi: number) => Math.round((mm / MM_PER_IN) * dpi);

/** Round for display: at most `d` decimals, no trailing zeros. */
export function trim(v: number, d = 2): string {
  if (!Number.isFinite(v)) return '';
  return String(Math.round(v * 10 ** d) / 10 ** d);
}

// ---- the zoom model of the editor ---------------------------------------------------------------

/** The zoom presets of the segmented control: Fit · 50 · 100 · 200. */
export type ZoomPreset = 'fit' | 0.5 | 1 | 2;
export const ZOOM_PRESETS: ZoomPreset[] = ['fit', 0.5, 1, 2];
/** The stops −/+ walk through. */
export const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4];

/** Which preset a zoom state is, or null when it is between presets. */
export function presetOf(z: number | 'fit'): ZoomPreset | null {
  if (z === 'fit') return 'fit';
  for (const p of ZOOM_PRESETS) if (p !== 'fit' && Math.abs(p - z) < 1e-3) return p;
  return null;
}

/** The next stop up (`dir` 1) or down (`dir` −1) from the zoom actually shown. */
export function zoomStep(current: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((s) => s > current + 1e-3) ?? ZOOM_MAX;
  return [...ZOOM_STEPS].reverse().find((s) => s < current - 1e-3) ?? ZOOM_MIN;
}

/** The readout: "100%". */
export const zoomLabel = (z: number) => `${Math.round(z * 100)}%`;

/** CSS pixels per millimetre at a zoom. */
export const pxPerMm = (zoom: number) => (PX_PER_IN / MM_PER_IN) * zoom;

export interface RulerTick {
  mm: number;
  /** Position along the ruler in CSS pixels from the figure's edge. */
  px: number;
  /** Every 10 mm carries a label and a long tick; every 5 mm a short one. */
  major: boolean;
}

/** Ticks of a millimetre ruler `lengthMm` long at a zoom: every 5 mm, majors every 10 mm. */
export function rulerTicks(lengthMm: number, zoom: number): RulerTick[] {
  const k = pxPerMm(zoom);
  const out: RulerTick[] = [];
  const n = Math.floor(Math.max(0, lengthMm) / 5 + 1e-9);
  for (let i = 0; i <= n; i++) {
    const mm = i * 5;
    out.push({ mm, px: mm * k, major: mm % 10 === 0 });
  }
  return out;
}
