// Display of results-sheet numbers and p values, mirroring crates/dioptra-core/src/results.rs
// (`format_p`, `stars`, `p_phrase`) and util.rs (`format_sig`) exactly, so the sheet on screen
// and the sheet exported by the engine agree digit for digit.
//
// Rust formats with exact decimal expansion and rounds exact ties to even ("{:.3}" of 0.0625
// is "0.062"); JavaScript's toFixed rounds ties up ("0.063"). `fixed` and `expo` below round
// the double's exact value half-to-even, as Rust does.
import type { PValueStyle } from '../engine/types';

/** A finite double's exact value as N / 10^k (N >= 0). */
function exact(ax: number): { n: bigint; k: number } {
  const buf = new DataView(new ArrayBuffer(8));
  buf.setFloat64(0, ax);
  const hi = buf.getUint32(0);
  const lo = buf.getUint32(4);
  const bexp = (hi >>> 20) & 0x7ff;
  let mant = (BigInt(hi & 0xfffff) << 32n) | BigInt(lo);
  let exp: number;
  if (bexp === 0) exp = -1074;
  else {
    mant |= 1n << 52n;
    exp = bexp - 1075;
  }
  if (exp >= 0) return { n: mant << BigInt(exp), k: 0 };
  // mant / 2^k = mant * 5^k / 10^k
  return { n: mant * 5n ** BigInt(-exp), k: -exp };
}

/** round(|x| * 10^d), half to even, on the exact value of x. `d` may be negative. */
function scaledRound(ax: number, d: number): bigint {
  const { n, k } = exact(ax);
  const shift = k - d;
  if (shift <= 0) return n * 10n ** BigInt(-shift);
  const div = 10n ** BigInt(shift);
  let q = n / div;
  const r = n % div;
  const half = div / 2n;
  if (r > half || (r === half && q % 2n === 1n)) q += 1n;
  return q;
}

/** `format!("{:.d}", x)` for d >= 0, rounding the exact value half to even. */
export function fixed(x: number, d: number): string {
  if (!Number.isFinite(x)) return String(x);
  const neg = x < 0 || Object.is(x, -0);
  const s = scaledRound(Math.abs(x), d).toString().padStart(d + 1, '0');
  const body = d > 0 ? `${s.slice(0, s.length - d)}.${s.slice(s.length - d)}` : s;
  return neg ? `-${body}` : body;
}

/** Position of the first significant digit: |x| in [10^(e-1), 10^e) gives e. */
function decimalPoint(ax: number): number {
  const { n, k } = exact(ax);
  return n.toString().length - k;
}

/** `format!("{:.*e}", sig - 1, x).parse()`: x rounded to `sig` significant digits. */
export function roundSig(x: number, sig: number): number {
  if (!Number.isFinite(x) || x === 0) return x;
  const ax = Math.abs(x);
  const d = sig - decimalPoint(ax);
  const q = scaledRound(ax, d).toString();
  const r = d >= 0 ? Number(`${q}e-${d}`) : Number(`${q}e${-d}`);
  return x < 0 ? -r : r;
}

function stripNegativeZero(s: string): string {
  return s.startsWith('-') && /^[0.]+$/.test(s.slice(1)) ? s.slice(1) : s;
}

/** Rust's `{:e}` of a double (shortest mantissa), with `E`: 1.5E-5, 1E20. */
function sciShortest(v: number): string {
  const s = v.toExponential(); // shortest round-trip mantissa, like Rust
  return s.replace('e+', 'e').replace('e', 'E');
}

/**
 * `util::format_sig`: fixed decimals when given; otherwise `sig` significant digits, shown
 * shortest, switching to scientific outside [1e-4, 1e15).
 */
export function formatSig(v: number | null | undefined, decimals: number | undefined, sig = 4): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '';
  if (v === Infinity) return 'Infinity';
  if (v === -Infinity) return '-Infinity';
  if (decimals !== undefined && decimals !== null) return stripNegativeZero(fixed(v, decimals));
  if (v === 0) return '0';
  const rounded = roundSig(v, Math.max(1, sig));
  const a = Math.abs(rounded);
  if (a >= 1e-4 && a < 1e15) return stripNegativeZero(String(rounded));
  return sciShortest(rounded);
}

/** GraphPad asterisks, decided on the stored p. */
export function stars(p: number): string {
  if (p < 0.0001) return '****';
  if (p < 0.001) return '***';
  if (p < 0.01) return '**';
  if (p < 0.05) return '*';
  return 'ns';
}

const stripZero = (s: string) => (s.startsWith('0.') ? s.slice(1) : s);

/** The value part of a p cell (GP without the asterisks). */
export function formatPValue(p: number, style: PValueStyle): string {
  if (!Number.isFinite(p)) return '';
  switch (style) {
    case 'apa':
      if (p < 0.001) return '< .001';
      if (p > 0.999) return '> .999';
      return stripZero(fixed(p, 3));
    case 'nejm':
      if (p < 0.001) return '<0.001';
      if (p > 0.999) return '>0.999';
      return fixed(p, 3);
    case 'gp':
      if (p < 0.0001) return '<0.0001';
      if (p > 0.9999) return '>0.9999';
      return fixed(p, 4);
    case 'exact':
      if (p < 0.0001) return '< 0.0001';
      return formatSig(roundSig(p, 4), undefined, 4);
  }
}

/** `results::format_p`: a p value for a table cell. */
export function formatP(p: number | null | undefined, style: PValueStyle): string {
  if (p === null || p === undefined || !Number.isFinite(p)) return '';
  const v = formatPValue(p, style);
  return style === 'gp' ? `${v} (${stars(p)})` : v;
}

/** `results::p_phrase`: a p value inside a sentence. */
export function pPhrase(p: number, style: PValueStyle): string {
  const v = formatPValue(p, style);
  const bound = v.startsWith('<') || v.startsWith('>');
  switch (style) {
    case 'apa':
    case 'exact':
      return bound ? `p ${v}` : `p = ${v}`;
    case 'nejm':
      return bound ? `P${v}` : `P=${v}`;
    case 'gp':
      return bound ? `P ${v}` : `P = ${v}`;
  }
}
