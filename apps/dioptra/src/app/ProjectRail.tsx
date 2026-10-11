// The project rail: tables, analyses, graphs and layouts as a tree grouped by kind.
// Rename with a double-click or F2, delete with Delete (confirmed), New table at the top.
import { useEffect, useRef, useState } from 'react';
import * as cmd from '../engine/commands';
import type { ProjectItem } from '../engine/types';
import { KIND_GROUPS, useProject } from '../store/project';
import { mutate } from '../store/sync';
import { deleteObject, newTable } from './actions';
import { showContextMenu, type MenuEntry } from './ContextMenu';
import { ChevronIcon, ITEM_ICONS, PlusIcon } from './icons';
import { kindInfo } from './kinds';

type RailAction = (item: ProjectItem) => MenuEntry[];
const railActions: RailAction[] = [];

/** Phase 5B adds per-object context-menu entries (Analyze..., New graph...) here. */
export function registerRailActions(fn: RailAction): () => void {
  railActions.push(fn);
  return () => {
    const i = railActions.indexOf(fn);
    if (i >= 0) railActions.splice(i, 1);
  };
}

function railItemActions(item: ProjectItem): MenuEntry[] {
  const extra = railActions.flatMap((fn) => fn(item));
  return extra.length ? ['separator', ...extra] : [];
}

export function ProjectRail() {
  const view = useProject((s) => s.view);
  const activeId = useProject((s) => s.activeId);
  const open = useProject((s) => s.open);
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [renaming, setRenaming] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const items = view?.items ?? [];
  const visible = KIND_GROUPS.flatMap((g) => (collapsed[g.kind] ? [] : items.filter((i) => i.kind === g.kind)));

  const focusItem = (id: string) =>
    (listRef.current?.querySelector(`[data-item="${id}"]`) as HTMLElement | null)?.focus();

  return (
    <nav className="rail" aria-label="Project">
      <div className="rail-head">
        <span className="rail-title" title={view?.path ?? view?.title}>
          {view?.title ?? 'Untitled'}
          {view?.dirty && (
            <span className="dirty-mark" title="Unsaved changes" aria-label="unsaved changes">
              {' '}
              (edited)
            </span>
          )}
        </span>
        <button type="button" className="btn btn-small" onClick={() => void newTable()} data-testid="rail-new-table" title="New table">
          <PlusIcon size={12} /> Table
        </button>
      </div>
      <div className="rail-tree" role="tree" aria-label="Project objects" ref={listRef}>
        {KIND_GROUPS.map((g) => {
          const members = items.filter((i) => i.kind === g.kind);
          const Icon = ITEM_ICONS[g.kind];
          const isOpen = !collapsed[g.kind];
          return (
            <div key={g.kind} className="rail-group" role="group" aria-label={g.label}>
              <button
                type="button"
                className="rail-group-head"
                aria-expanded={isOpen}
                onClick={() => setCollapsed((c) => ({ ...c, [g.kind]: !c[g.kind] }))}
              >
                <ChevronIcon open={isOpen} size={10} />
                <Icon size={12} />
                <span>{g.label}</span>
                <span className="count">{members.length}</span>
              </button>
              {isOpen && members.length === 0 && <div className="rail-empty">None yet</div>}
              {isOpen &&
                members.map((it) =>
                  renaming === it.id ? (
                    <RenameField
                      key={it.id}
                      item={it}
                      done={() => {
                        setRenaming(null);
                        requestAnimationFrame(() => focusItem(it.id));
                      }}
                    />
                  ) : (
                    <RailItem
                      key={it.id}
                      item={it}
                      active={it.id === activeId}
                      onOpen={() => open(it.id)}
                      onRename={() => setRenaming(it.id)}
                      onMove={(d) => {
                        const i = visible.findIndex((v) => v.id === it.id);
                        const next = visible[i + d];
                        if (next) focusItem(next.id);
                      }}
                    />
                  ),
                )}
            </div>
          );
        })}
      </div>
    </nav>
  );
}

function RailItem({
  item,
  active,
  onOpen,
  onRename,
  onMove,
}: {
  item: ProjectItem;
  active: boolean;
  onOpen: () => void;
  onRename: () => void;
  onMove: (d: -1 | 1) => void;
}) {
  const Icon = ITEM_ICONS[item.kind];
  const sub = item.kind === 'table' ? kindInfo(item.subtype)?.name : item.subtype;
  return (
    <div
      role="treeitem"
      aria-selected={active}
      tabIndex={active ? 0 : -1}
      data-item={item.id}
      data-testid={`rail-item-${item.title}`}
      className={'rail-item' + (active ? ' is-active' : '')}
      onClick={onOpen}
      onDoubleClick={onRename}
      onContextMenu={(e) => {
        e.preventDefault();
        onOpen();
        showContextMenu(e.clientX, e.clientY, [
          { label: 'Open', run: onOpen },
          { label: 'Rename', shortcut: 'F2', run: onRename },
          ...railItemActions(item),
          'separator',
          { label: 'Delete...', shortcut: 'Delete', run: () => deleteObject(item.id) },
        ]);
      }}
      onKeyDown={(e) => {
        if (e.key === 'F2') {
          e.preventDefault();
          onRename();
        } else if (e.key === 'Delete') {
          e.preventDefault();
          void deleteObject(item.id);
        } else if (e.key === 'Enter') {
          e.preventDefault();
          onOpen();
        } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault();
          onMove(e.key === 'ArrowDown' ? 1 : -1);
        } else return;
        e.stopPropagation();
      }}
    >
      <Icon />
      <span className="rail-item-title">{item.title}</span>
      {sub && <span className="rail-item-sub">{sub}</span>}
      {item.stale && (
        <span className="stale-mark" title="Out of date; recomputes when shown">
          stale
        </span>
      )}
    </div>
  );
}

function RenameField({ item, done }: { item: ProjectItem; done: () => void }) {
  const [v, setV] = useState(item.title);
  const ref = useRef<HTMLInputElement>(null);
  const committed = useRef(false);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const commit = async () => {
    if (committed.current) return;
    committed.current = true;
    const t = v.trim();
    if (t && t !== item.title) await mutate((e) => cmd.renameObject(e, item.id, t));
    done();
  };
  return (
    <div className="rail-item is-renaming">
      <input
        ref={ref}
        className="input rail-rename"
        value={v}
        aria-label={`Rename ${item.title}`}
        data-testid="rail-rename"
        onChange={(e) => setV(e.target.value)}
        onBlur={() => void commit()}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void commit();
          } else if (e.key === 'Escape') {
            e.preventDefault();
            committed.current = true;
            done();
          }
          e.stopPropagation();
        }}
      />
    </div>
  );
}
