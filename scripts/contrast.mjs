#!/usr/bin/env node
// WCAG 2 contrast of the token pairs that carry text or UI meaning, in both themes, read from
// apps/dioptra/src/styles/tokens.css (DESIGN.md §12.1). Exit 1 when a body-text pair is below
// 4.5:1 or a UI pair below 3:1. No dependencies.
//   node scripts/contrast.mjs            table for both themes
//   node scripts/contrast.mjs --markdown the same table as Markdown (for docs/DESIGN-AUDIT.md)
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const css = readFileSync(resolve(here, '../apps/dioptra/src/styles/tokens.css'), 'utf8');

/** The custom properties declared in the first block whose selector list matches `re`. */
function block(re) {
  const m = re.exec(css);
  if (!m) throw new Error(`no block for ${re}`);
  const start = css.indexOf('{', m.index) + 1;
  const end = css.indexOf('}', start);
  const out = {};
  for (const d of css.slice(start, end).matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) out[d[1]] = d[2].trim();
  return out;
}

const themes = {
  light: block(/:root,\s*:root\[data-theme='light'\]\s*\{/),
  dark: block(/^:root\[data-theme='dark'\]\s*\{/m),
};

// The prefers-color-scheme copy of the dark theme must say the same as the explicit one.
const media = block(/:root:not\(\[data-theme='light'\]\)\s*\{/);
const drift = Object.keys({ ...media, ...themes.dark }).filter((k) => media[k] !== themes.dark[k]);
if (drift.length) {
  console.error(`The two dark blocks in tokens.css differ: ${drift.join(', ')}`);
  process.exit(1);
}

function hex(v) {
  const m = /^#([0-9a-f]{6})$/i.exec(v);
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
function lum([r, g, b]) {
  const f = (c) => {
    c /= 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
function ratio(a, b) {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

// [foreground, background, class]: body = text people read (4.5:1), ui = marks, glyphs,
// row numbers, placeholders and focus rings (3:1).
const PAIRS = [
  ['--fg', '--bg-app', 'body'],
  ['--fg', '--bg-panel', 'body'],
  ['--fg', '--bg-sheet', 'body'],
  ['--fg', '--bg-raised', 'body'],
  ['--fg', '--bg-inset', 'body'],
  ['--fg', '--accent-soft', 'body'],
  ['--fg-muted', '--bg-app', 'body'],
  ['--fg-muted', '--bg-panel', 'body'],
  ['--fg-muted', '--bg-sheet', 'body'],
  ['--fg-muted', '--bg-raised', 'body'],
  ['--fg-muted', '--bg-inset', 'body'],
  ['--fg-faint', '--bg-panel', 'ui'],
  ['--fg-faint', '--bg-sheet', 'ui'],
  ['--fg-faint', '--bg-inset', 'ui'],
  ['--accent', '--bg-panel', 'body'],
  ['--accent', '--bg-sheet', 'body'],
  ['--accent', '--bg-app', 'ui'],
  ['--accent-fg', '--accent', 'body'],
  ['--accent-fg', '--accent-hover', 'body'],
  ['--ok', '--bg-panel', 'ui'],
  ['--caution', '--bg-panel', 'ui'],
  ['--fail', '--bg-panel', 'ui'],
  ['--ok', '--bg-sheet', 'ui'],
  ['--caution', '--bg-sheet', 'ui'],
  ['--fail', '--bg-sheet', 'ui'],
  ['--focus', '--bg-panel', 'ui'],
  ['--tooltip-fg', '--tooltip-bg', 'body'],
];
const NEED = { body: 4.5, ui: 3 };

const md = process.argv.includes('--markdown');
let failed = 0;
const rows = [];
for (const [fg, bg, cls] of PAIRS) {
  const row = { fg, bg, cls };
  for (const [name, t] of Object.entries(themes)) {
    const a = hex(t[fg] ?? '');
    const b = hex(t[bg] ?? '');
    if (!a || !b) throw new Error(`${name}: ${fg} or ${bg} is not a #rrggbb colour`);
    const r = ratio(a, b);
    const ok = r >= NEED[cls];
    if (!ok) failed++;
    row[name] = { r, ok, a: t[fg], b: t[bg] };
  }
  rows.push(row);
}

const cell = (x) => `${x.r.toFixed(2)}${x.ok ? '' : ' FAIL'}`;
if (md) {
  console.log('| Foreground | Background | Class | Light | Dark |');
  console.log('|---|---|---|---|---|');
  for (const r of rows) console.log(`| \`${r.fg}\` | \`${r.bg}\` | ${r.cls} (${NEED[r.cls]}:1) | ${cell(r.light)} | ${cell(r.dark)} |`);
} else {
  const pad = (s, n) => String(s).padEnd(n);
  console.log(pad('foreground', 14) + pad('background', 15) + pad('class', 10) + pad('light', 22) + 'dark');
  for (const r of rows) {
    const t = (x) => `${x.r.toFixed(2).padStart(5)} ${x.ok ? 'ok  ' : 'FAIL'} ${x.a}`;
    console.log(pad(r.fg, 14) + pad(r.bg, 15) + pad(`${r.cls} ${NEED[r.cls]}`, 10) + pad(t(r.light), 22) + t(r.dark));
  }
}
console.log(failed ? `\n${failed} pair(s) below the WCAG AA threshold.` : '\nEvery pair meets WCAG AA.');
process.exit(failed ? 1 : 0);
