// A small fuzzy matcher for the command palette: the query's characters must appear in order;
// matches at word starts, consecutive runs and the beginning score higher.

export interface FuzzyMatch {
  score: number;
  /** Indices into the candidate of the matched characters. */
  positions: number[];
}

const isWordStart = (s: string, i: number) => {
  if (i === 0) return true;
  const prev = s[i - 1]!;
  const cur = s[i]!;
  return /[\s\-_/.:(]/.test(prev) || (prev === prev.toLowerCase() && cur !== cur.toLowerCase());
};

/** Match `query` against `candidate`; `null` when the query is not a subsequence. */
export function fuzzyMatch(query: string, candidate: string): FuzzyMatch | null {
  const q = query.trim().toLowerCase().replace(/\s+/g, ' ');
  if (q === '') return { score: 0, positions: [] };
  const c = candidate.toLowerCase();
  // Greedy forward pass, preferring word starts: for each query char take the next word start
  // that still allows the rest to match, else the next occurrence.
  const positions: number[] = [];
  let from = 0;
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi]!;
    if (ch === ' ') continue;
    let found = -1;
    // Continue a run when possible.
    if (positions.length && c[from] === ch) found = from;
    if (found < 0) {
      for (let i = from; i < c.length; i++) {
        if (c[i] === ch && isWordStart(candidate, i) && canFinish(q, qi + 1, c, i + 1)) {
          found = i;
          break;
        }
      }
    }
    if (found < 0) found = c.indexOf(ch, from);
    if (found < 0) return null;
    positions.push(found);
    from = found + 1;
  }
  let score = 0;
  for (let i = 0; i < positions.length; i++) {
    const p = positions[i]!;
    score += 1;
    if (isWordStart(candidate, p)) score += 8;
    if (i > 0 && p === positions[i - 1]! + 1) score += 5;
    if (p === 0) score += 6;
  }
  // Prefer short candidates and early matches.
  score -= positions[0]! * 0.2;
  score -= (candidate.length - positions.length) * 0.05;
  if (c.startsWith(q)) score += 20;
  else if (c.includes(q)) score += 10;
  return { score, positions };
}

function canFinish(q: string, qi: number, c: string, from: number): boolean {
  let j = from;
  for (let i = qi; i < q.length; i++) {
    if (q[i] === ' ') continue;
    j = c.indexOf(q[i]!, j);
    if (j < 0) return false;
    j++;
  }
  return true;
}

export interface Ranked<T> {
  item: T;
  match: FuzzyMatch;
}

/** Filter and sort items by their best-matching text field. */
export function fuzzyFilter<T>(query: string, items: readonly T[], text: (item: T) => string[]): Ranked<T>[] {
  const out: Ranked<T>[] = [];
  items.forEach((item) => {
    let best: FuzzyMatch | null = null;
    const fields = text(item);
    fields.forEach((f, fi) => {
      const m = fuzzyMatch(query, f);
      if (!m) return;
      // Secondary fields (keywords, category) count for less.
      const adjusted = fi === 0 ? m : { score: m.score - 4, positions: [] };
      if (!best || adjusted.score > best.score) best = adjusted;
    });
    if (best) out.push({ item, match: best });
  });
  if (query.trim() === '') return out;
  return out
    .map((r, i) => ({ r, i }))
    .sort((a, b) => b.r.match.score - a.r.match.score || a.i - b.i)
    .map(({ r }) => r);
}
