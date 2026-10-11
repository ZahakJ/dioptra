// What a hit-test target is, in words, and how the editor treats it.
import type { Target } from '../engine/types';

/** Targets whose text can be edited in place, with the spec path of that text. */
export function textPathOf(t: Target, annotations: { kind: string }[] = []): string | null {
  if (t.target === '/title') return '/title';
  if (t.target === '/axes/x/title' || t.target === '/axes/y/title') return t.target;
  if (t.target === '/legend' && t.id === 'legend') return null;
  const m = t.target.match(/^\/annotations\/(\d+)$/);
  if (m) {
    const a = annotations[Number(m[1])];
    if (a && (a.kind === 'text_label' || a.kind === 'significance_bracket')) return `${t.target}/text`;
    if (a && a.kind === 'reference_line') return `${t.target}/label`;
  }
  return null;
}

/** Whether a target can be dragged (legend, free text). */
export function isDraggable(t: Target, annotations: { kind: string }[] = []): boolean {
  if (t.id === 'legend') return true;
  const m = t.target.match(/^\/annotations\/(\d+)$/);
  return !!m && annotations[Number(m[1])]?.kind === 'text_label';
}

export function describeTarget(t: Target | undefined): string {
  if (!t) return '';
  const p = t.target;
  if (p === '/title') return 'Graph title';
  if (p === '/axes/x/title') return 'X axis title';
  if (p === '/axes/y/title') return 'Y axis title';
  if (p.startsWith('/axes/x')) return 'X axis';
  if (p.startsWith('/axes/y')) return 'Y axis';
  if (p === '/axes/frame') return 'Frame';
  if (p === '/axes/grid') return 'Gridlines';
  if (p === '/legend') return 'Legend';
  if (p.startsWith('/style/datasets/')) return `Data set ${Number(p.split('/')[3]) + 1}`;
  if (p.startsWith('/annotations/')) return `Annotation ${Number(p.split('/')[2]) + 1}`;
  if (p === '/mapping') return 'Data';
  if (p.startsWith('/options')) return 'Family options';
  if (p === '/style/palette') return 'Colour scale';
  return t.id.replace(/-/g, ' ');
}

/** Larger boxes first, so smaller ones sit on top and win clicks. */
export function byAreaDescending(targets: Target[]): Target[] {
  return [...targets].sort((a, b) => b.bbox.width * b.bbox.height - a.bbox.width * a.bbox.height);
}
