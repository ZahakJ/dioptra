// The work area: tabs of open objects and, for the active one, the renderer registered for its
// kind. Phase 5B registers `analysis`, `graph` and `layout` renderers with `registerWorkArea`.
import { BookOpen, ClipboardPaste, ExternalLink, FlaskConical } from 'lucide-react';
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

/** The first-launch and nothing-open state (DESIGN.md §12.8): one line of purpose, three ways
 *  in, and the keys that matter. */
function EmptyWork() {
  const base = import.meta.env.BASE_URL ?? '/';
  const cards: { id: string; icon: React.ReactNode; title: string; text: string; testId: string; external?: boolean }[] = [
    {
      id: 'file.pasteNew',
      icon: <ClipboardPaste size={20} strokeWidth={1.5} aria-hidden="true" />,
      title: 'Paste data',
      text: 'Copy columns from a plate reader, a spreadsheet or a notebook. Dioptra reads them into a new table and says what it decided.',
      testId: 'welcome-paste',
    },
    {
      id: 'file.openSample',
      icon: <FlaskConical size={20} strokeWidth={1.5} aria-hidden="true" />,
      title: 'Open a sample project',
      text: 'A cytokine study: a dose comparison, a dose-response fit and a survival curve, each with its analysis and figure.',
      testId: 'welcome-sample',
    },
    {
      id: 'help.manual',
      icon: <BookOpen size={20} strokeWidth={1.5} aria-hidden="true" />,
      title: 'Read the manual',
      text: 'How tables, analyses and figures fit together, and what every test assumes.',
      testId: 'welcome-manual',
      external: true,
    },
  ];
  const keys: [string, string][] = [
    ['table.new', 'New table'],
    ['file.open', 'Open'],
    ['view.palette', 'All commands'],
    ['help.shortcuts', 'Shortcuts'],
  ];
  return (
    <div className="welcome" data-testid="welcome">
      <div className="welcome-inner">
        <img className="welcome-mark" src={`${base}icon.svg`} width={44} height={44} alt="" />
        <h1 className="welcome-title">A sheet for your data. Paste, test, export.</h1>
        <div className="welcome-cards">
          {cards.map((c) => {
            const sc = commands.get(c.id)?.shortcut;
            return (
              <button key={c.id} type="button" className="welcome-card" data-testid={c.testId} onClick={() => void commands.run(c.id)}>
                <span className="welcome-card-icon">{c.icon}</span>
                <span className="welcome-card-title">
                  {c.title}
                  {c.external && <ExternalLink size={12} strokeWidth={1.5} aria-label="opens in the browser" />}
                </span>
                <span className="welcome-card-text">{c.text}</span>
                {sc && <kbd className="kbd welcome-card-kbd">{formatShortcut(sc)}</kbd>}
              </button>
            );
          })}
        </div>
        <div className="welcome-keys" aria-label="Keyboard">
          {keys.map(([id, label]) => {
            const c = commands.get(id);
            return (
              <button key={id} type="button" className="welcome-key" data-testid={`empty-${id}`} onClick={() => void commands.run(id)}>
                {c?.shortcut && <kbd className="kbd">{formatShortcut(c.shortcut)}</kbd>}
                <span>{label}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
