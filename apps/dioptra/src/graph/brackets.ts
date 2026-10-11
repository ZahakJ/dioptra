// Significance brackets: placing one by hand and deriving them from an analysis' sheet.
import type { PValueStyle } from '../engine/types';
import type { Annotation, ResultCell, Sheet } from '../engine/objects';
import { pPhrase, stars } from '../results/format';

export type Bracket = Extract<Annotation, { kind: 'significance_bracket' }>;

/** The label on a bracket in the project's p style: asterisks for GP, else a p phrase. */
export function bracketText(p: number, style: PValueStyle): string {
  return style === 'gp' ? stars(p) : pPhrase(p, style);
}

/**
 * Add a bracket between two categories (0-based, any order). Returns the new annotation list,
 * or the old one unchanged when the pair is invalid or already bracketed (its text is then
 * updated). The engine stacks overlapping brackets itself, so no height is set.
 */
export function placeBracket(
  annotations: Annotation[],
  a: number,
  b: number,
  text: string,
  categories: number,
): Annotation[] {
  const from = Math.min(a, b);
  const to = Math.max(a, b);
  if (from === to || from < 0 || to >= categories) return annotations;
  const i = annotations.findIndex((x) => x.kind === 'significance_bracket' && x.from === from && x.to === to);
  if (i >= 0) {
    const next = [...annotations];
    next[i] = { ...(annotations[i] as Bracket), text };
    return next;
  }
  return [...annotations, { kind: 'significance_bracket', from, to, text }];
}

/** The first pair not yet bracketed: neighbours first, then wider spans. */
export function nextFreePair(annotations: Annotation[], categories: number): [number, number] | null {
  const taken = new Set(
    annotations.filter((x): x is Bracket => x.kind === 'significance_bracket').map((x) => `${x.from}:${x.to}`),
  );
  for (let span = 1; span < categories; span++) {
    for (let a = 0; a + span < categories; a++) if (!taken.has(`${a}:${a + span}`)) return [a, a + span];
  }
  return null;
}

const isP = (c: ResultCell | undefined): c is { p_value: { p: number } } =>
  !!c && typeof c === 'object' && 'p_value' in c && typeof c.p_value.p === 'number';
const textOf = (c: ResultCell | undefined): string | null =>
  !!c && typeof c === 'object' && 'text' in c ? c.text : null;

/** Split "A vs B", "A vs. B", "A – B", "A - B" into two names. */
export function splitPair(label: string): [string, string] | null {
  const m = label.match(/^\s*(.+?)\s+(?:vs\.?|versus|–|—|-)\s+(.+?)\s*$/i);
  return m ? [m[1]!, m[2]!] : null;
}

export interface BracketSource {
  from: number;
  to: number;
  p: number;
}

/**
 * Comparisons a sheet reports, as category pairs with p values. `names` are the graph's
 * categories in order (the plotted data sets). Two-group tests give one bracket between their
 * two groups; multiple-comparison tables give one per row whose label names two categories.
 */
export function comparisonsFromSheet(sheet: Sheet, names: string[]): BracketSource[] {
  const index = (n: string) => names.findIndex((x) => x.trim().toLowerCase() === n.trim().toLowerCase());
  const out: BracketSource[] = [];
  for (const s of sheet.sections) {
    for (const row of s.table.rows) {
      const p = row.find(isP);
      if (!p) continue;
      const labels = row.map(textOf).filter((t): t is string => !!t);
      let pair: [number, number] | null = null;
      for (const l of labels) {
        const sp = splitPair(l);
        if (sp) {
          const a = index(sp[0]);
          const b = index(sp[1]);
          if (a >= 0 && b >= 0 && a !== b) {
            pair = [a, b];
            break;
          }
        }
      }
      if (!pair && labels.length >= 2) {
        const a = index(labels[0]!);
        const b = index(labels[1]!);
        if (a >= 0 && b >= 0 && a !== b) pair = [a, b];
      }
      if (pair) out.push({ from: Math.min(...pair), to: Math.max(...pair), p: p.p_value.p });
    }
  }
  if (out.length === 0 && names.length === 2 && TWO_GROUP.has(sheet.analysis_kind)) {
    // A two-group test: the first p value of the sheet is the comparison.
    for (const s of sheet.sections) {
      for (const row of s.table.rows) {
        const p = row.find(isP);
        if (p) return [{ from: 0, to: 1, p: p.p_value.p }];
      }
    }
  }
  // One bracket per pair (the first row wins).
  const seen = new Set<string>();
  return out.filter((b) => {
    const k = `${b.from}:${b.to}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}

const TWO_GROUP = new Set(['t_test', 'mann_whitney', 'wilcoxon']);

/** Annotations with brackets from a sheet added (existing brackets for the same pair updated). */
export function bracketsFromSheet(
  annotations: Annotation[],
  sheet: Sheet,
  names: string[],
  style: PValueStyle,
): Annotation[] {
  let next = annotations;
  for (const c of comparisonsFromSheet(sheet, names)) next = placeBracket(next, c.from, c.to, bracketText(c.p, style), names.length);
  return next;
}
