// Reading the data scale back from the rendered SVG: tick labels sit at their values, so two
// labels give points per data unit. Used to move text annotations (in data units) by a drag
// measured in points, and to place new annotations in the middle of the plot.

/** A tick label's number: "−2.5", "2×10³", "10⁻²", with superscripts as text or tspans. */
export function parseTick(el: Element): number | null {
  let s = '';
  for (const n of Array.from(el.childNodes)) {
    if (n.nodeType === 1) {
      const e = n as Element;
      const shift = e.getAttribute('baseline-shift') ?? '';
      const dy = Number(e.getAttribute('dy') ?? '0');
      const t = e.textContent ?? '';
      s += shift.includes('super') || dy < 0 ? `^${t}` : t;
    } else s += n.textContent ?? '';
  }
  return parseTickText(s);
}

const SUP: Record<string, string> = { '⁰': '0', '¹': '1', '²': '2', '³': '3', '⁴': '4', '⁵': '5', '⁶': '6', '⁷': '7', '⁸': '8', '⁹': '9', '⁻': '-', '⁺': '+' };

export function parseTickText(raw: string): number | null {
  let s = raw.replace(/[−–]/g, '-').replace(/\s+/g, '').replace(/,/g, '');
  s = s.replace(/[⁰¹²³⁴⁵⁶⁷⁸⁹⁻⁺]+/g, (m) => '^' + [...m].map((c) => SUP[c]).join(''));
  let m = s.match(/^(-?\d*\.?\d+)[×x·]10\^?([-+]?\d+)$/);
  if (m) return Number(m[1]) * 10 ** Number(m[2]);
  m = s.match(/^(-?)10\^([-+]?\d+)$/);
  if (m) return (m[1] ? -1 : 1) * 10 ** Number(m[2]);
  m = s.match(/^(-?)2\^([-+]?\d+)$/);
  if (m) return (m[1] ? -1 : 1) * 2 ** Number(m[2]);
  if (/^-?\d*\.?\d+(e[-+]?\d+)?$/i.test(s)) return Number(s);
  return null;
}

export type AxisScale = 'linear' | 'log10' | 'log2' | 'categorical' | undefined;

export interface AxisMap {
  /** Data value (in scale space: log10 for log axes) per point. */
  slope: number;
  scale: AxisScale;
  /** A reference: position in points and data value (in scale space). */
  pt0: number;
  v0: number;
}

const fwd = (v: number, scale: AxisScale) => (scale === 'log10' ? Math.log10(v) : scale === 'log2' ? Math.log2(v) : v);
const back = (v: number, scale: AxisScale) => (scale === 'log10' ? 10 ** v : scale === 'log2' ? 2 ** v : v);

/** From two or more labels: points `pt` with values `v` (raw data values). */
export function axisMapFrom(labels: { pt: number; v: number }[], scale: AxisScale): AxisMap | null {
  const pts = labels
    .filter((l) => Number.isFinite(l.v) && Number.isFinite(l.pt) && (scale === 'log10' || scale === 'log2' ? l.v > 0 : true))
    .map((l) => ({ pt: l.pt, v: fwd(l.v, scale) }));
  if (pts.length < 2) return null;
  const a = pts[0]!;
  const b = pts[pts.length - 1]!;
  if (b.pt === a.pt) return null;
  return { slope: (b.v - a.v) / (b.pt - a.pt), scale, pt0: a.pt, v0: a.v };
}

/** Read an axis map from a rendered SVG element. `prefix` is the layout panel prefix, if any. */
export function axisMap(svg: Element, axis: 'x' | 'y', scale: AxisScale, prefix = ''): AxisMap | null {
  const texts = Array.from(svg.querySelectorAll(`#${prefix}axis-${axis}-labels text`));
  const attr = axis === 'x' ? 'x' : 'y';
  const labels = texts.map((t, i) => {
    const pt = Number(t.getAttribute(attr));
    const parsed = parseTick(t);
    // Categories sit at 1, 2, 3, ...
    const v = scale === 'categorical' || parsed === null ? i + 1 : parsed;
    return { pt, v };
  });
  const allText = texts.length > 0 && texts.every((t) => parseTick(t) === null);
  return axisMapFrom(labels, allText ? 'categorical' : scale);
}

/** Move a data value by `dpt` points along an axis. */
export function moveBy(map: AxisMap, value: number, dpt: number): number {
  return back(fwd(value, map.scale) + dpt * map.slope, map.scale);
}

/** The data value at a position in points (approximate on the y axis: labels sit near their ticks). */
export function valueAt(map: AxisMap, pt: number): number {
  return back(map.v0 + (pt - map.pt0) * map.slope, map.scale);
}

/** Round a value to a sensible number of significant digits for a position. */
export function tidy(v: number, sig = 3): number {
  if (!Number.isFinite(v) || v === 0) return 0;
  return Number(v.toPrecision(sig));
}
