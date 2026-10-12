// The inspector: a column of collapsible sections, each registered with a predicate that says
// when it applies. Phase 5B registers analysis options and graph properties sections here.
import { useSyncExternalStore, type ComponentType } from 'react';
import type { Id, ProjectItem } from '../engine/types';
import { useActiveItem } from '../store/project';
import { useUi } from '../store/ui';
import { ChevronIcon } from './icons';

export interface InspectorContext {
  /** The active project object, if any. */
  item: ProjectItem | undefined;
  /** The active table's id when the active object is a table. */
  tableId: Id | null;
}

export interface InspectorSection {
  id: string;
  title: string;
  /** Lower first. Core sections use 10 (cell), 20 (column), 30 (table), 90 (history), 100 (project). */
  order: number;
  when: (ctx: InspectorContext) => boolean;
  component: ComponentType<{ ctx: InspectorContext }>;
  /** Collapsed until opened. Default false. */
  collapsed?: boolean;
}

const sections = new Map<string, InspectorSection>();
const listeners = new Set<() => void>();
let version = 0;

export function registerInspectorSection(section: InspectorSection): () => void {
  sections.set(section.id, section);
  bump();
  return () => {
    if (sections.get(section.id) === section) {
      sections.delete(section.id);
      bump();
    }
  };
}

export function inspectorSections(): InspectorSection[] {
  return [...sections.values()].sort((a, b) => a.order - b.order);
}

/** Re-evaluate every section's `when` (e.g. after a graph's family changed). */
export function refreshInspector(): void {
  bump();
}

function bump() {
  version++;
  for (const l of listeners) l();
}

export function Inspector() {
  useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => version,
    () => version,
  );
  const item = useActiveItem();
  const ctx: InspectorContext = { item, tableId: item?.kind === 'table' ? item.id : null };
  const shown = inspectorSections().filter((s) => {
    try {
      return s.when(ctx);
    } catch {
      return false;
    }
  });
  return (
    <aside className="inspector scroll-area" aria-label="Inspector" data-testid="inspector">
      {shown.map((s) => (
        <Section key={s.id} section={s} ctx={ctx} />
      ))}
    </aside>
  );
}

function Section({ section, ctx }: { section: InspectorSection; ctx: InspectorContext }) {
  // Open or closed is remembered per section across launches.
  const key = `insp:${section.id}`;
  const stored = useUi((s) => s.collapsed[key]);
  const open = !(stored ?? !!section.collapsed);
  const C = section.component;
  return (
    <section className={'insp-section' + (open ? ' is-open' : '')} data-testid={`insp-${section.id}`}>
      <button type="button" className="insp-head" aria-expanded={open} onClick={() => useUi.getState().setCollapsed(key, open)}>
        <ChevronIcon open={open} size={14} />
        <span>{section.title}</span>
      </button>
      {open && (
        <div className="insp-body">
          <C ctx={ctx} />
        </div>
      )}
    </section>
  );
}
