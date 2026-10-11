// The work area: tabs of open objects and, for the active one, the renderer registered for its
// kind. Phase 5B registers `analysis`, `graph` and `layout` renderers with `registerWorkArea`.
import { useSyncExternalStore, type ComponentType } from 'react';
import type { ItemKind, ProjectItem } from '../engine/types';
import { useProject } from '../store/project';
import { commands } from './commands';
import { CloseIcon, ITEM_ICONS, ItemIcon } from './icons';
import { formatShortcut } from './keys';

export interface WorkAreaProps {
  item: ProjectItem;
}

export type WorkAreaRenderer = ComponentType<WorkAreaProps>;

const renderers = new Map<ItemKind, WorkAreaRenderer>();
const listeners = new Set<() => void>();
let version = 0;

/** Register the renderer for one object kind (replacing any earlier one). Returns an undo. */
export function registerWorkArea(kind: ItemKind, renderer: WorkAreaRenderer): () => void {
  const previous = renderers.get(kind);
  renderers.set(kind, renderer);
  bump();
  return () => {
    if (renderers.get(kind) === renderer) {
      if (previous) renderers.set(kind, previous);
      else renderers.delete(kind);
      bump();
    }
  };
}

export function getWorkArea(kind: ItemKind): WorkAreaRenderer | undefined {
  return renderers.get(kind);
}

function bump() {
  version++;
  for (const l of listeners) l();
}

function useRenderersVersion(): number {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => version,
    () => version,
  );
}

/** Shown for kinds without a registered renderer. */
export function PlaceholderWorkArea({ item }: WorkAreaProps) {
  const Icon = ITEM_ICONS[item.kind];
  return (
    <div className="placeholder" data-testid={`placeholder-${item.kind}`}>
      <div className="placeholder-card">
        <Icon size={20} />
        <h2>{item.title}</h2>
        <p>
          This {item.kind} is saved in the project{item.subtype ? ` (${item.subtype.replace(/_/g, ' ')})` : ''}, and its
          data stay linked. The {item.kind} editor is not part of this build yet.
        </p>
      </div>
    </div>
  );
}

export function WorkArea() {
  useRenderersVersion();
  const view = useProject((s) => s.view);
  const tabs = useProject((s) => s.tabs);
  const activeId = useProject((s) => s.activeId);
  const open = useProject((s) => s.open);
  const close = useProject((s) => s.close);
  const items = new Map(view?.items.map((i) => [i.id, i]));
  const active = activeId ? items.get(activeId) : undefined;
  const Renderer = active ? (getWorkArea(active.kind) ?? PlaceholderWorkArea) : null;
  return (
    <main className="work">
      <div className="tabs" role="tablist" aria-label="Open objects" data-empty={tabs.length === 0 || undefined}>
        {tabs.map((id) => {
          const it = items.get(id);
          if (!it) return null;
          const isActive = id === activeId;
          return (
            <div key={id} className={'tab' + (isActive ? ' is-active' : '')} role="presentation">
              <button
                type="button"
                role="tab"
                aria-selected={isActive}
                className="tab-main"
                onClick={() => open(id)}
                onAuxClick={(e) => e.button === 1 && close(id)}
                data-testid={`tab-${it.title}`}
              >
                <ItemIcon item={it} />
                <span className="tab-title">{it.title}</span>
              </button>
              <button type="button" className="tab-close" aria-label={`Close ${it.title}`} data-tip="Close tab" onClick={() => close(id)}>
                <CloseIcon size={16} />
              </button>
            </div>
          );
        })}
      </div>
      <div className="work-body" role="tabpanel">
        {active && Renderer ? <Renderer key={active.id} item={active} /> : <EmptyWork />}
      </div>
    </main>
  );
}

function EmptyWork() {
  return (
    <div className="placeholder">
      <div className="placeholder-card">
        <h2>No table open</h2>
        <p>Create a table, paste data as a new table, or open a project. The palette lists every command.</p>
        <EmptyActions />
      </div>
    </div>
  );
}

function EmptyActions() {
  const rows: [string, string][] = [
    ['table.new', 'New table'],
    ['file.pasteNew', 'Paste as new table'],
    ['file.open', 'Open a project'],
    ['view.palette', 'Command palette'],
  ];
  return (
    <div className="empty-actions">
      {rows.map(([id, label]) => {
        const c = commands.get(id);
        return (
          <button key={id} type="button" className="btn btn-secondary" data-testid={`empty-${id}`} onClick={() => void commands.run(id)}>
            {label}
            {c?.shortcut && <kbd className="kbd">{formatShortcut(c.shortcut)}</kbd>}
          </button>
        );
      })}
    </div>
  );
}
