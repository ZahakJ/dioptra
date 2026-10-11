// The command palette (Ctrl/Cmd-K): fuzzy search over every command, shortcuts shown,
// arrows to move, Enter to run, Escape to close. Also lists project objects to open.
import { useEffect, useMemo, useRef, useState } from 'react';
import { useProject } from '../store/project';
import { useUi } from '../store/ui';
import { commands } from './commands';
import { fuzzyFilter } from './fuzzy';
import { ItemIcon, MENU_ICONS, SearchIcon } from './icons';
import { formatShortcut } from './keys';

interface Entry {
  key: string;
  title: string;
  detail: string;
  shortcut?: string;
  keywords: string[];
  disabled: boolean;
  icon?: React.ReactNode;
  run: () => unknown;
}

function Highlight({ text, positions }: { text: string; positions: number[] }) {
  if (!positions.length) return <>{text}</>;
  const set = new Set(positions);
  return (
    <>
      {[...text].map((ch, i) =>
        set.has(i) ? (
          <mark key={i} className="hl">
            {ch}
          </mark>
        ) : (
          <span key={i}>{ch}</span>
        ),
      )}
    </>
  );
}

export function Palette() {
  const open = useUi((s) => s.paletteOpen);
  if (!open) return null;
  return <PaletteBody />;
}

function PaletteBody() {
  const setOpen = useUi((s) => s.setPaletteOpen);
  const items = useProject((s) => s.view?.items ?? []);
  const [q, setQ] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const prevFocus = useRef<HTMLElement | null>(document.activeElement as HTMLElement | null);

  const entries = useMemo<Entry[]>(() => {
    const cmds: Entry[] = commands
      .all()
      .filter((c) => !c.hidden)
      .map((c) => ({
        key: c.id,
        title: c.title.replace(/\.\.\.$/, ''),
        detail: c.menu ?? '',
        shortcut: c.shortcut,
        keywords: c.keywords ?? [],
        disabled: !commands.isEnabled(c),
        icon: c.menu ? MENU_ICONS[c.menu] : undefined,
        run: () => commands.run(c.id),
      }));
    const objs: Entry[] = items.map((it) => {
      return {
        key: `open:${it.id}`,
        title: `Go to ${it.title}`,
        detail: it.kind === 'table' ? 'Table' : it.kind === 'analysis' ? 'Analysis' : it.kind === 'graph' ? 'Graph' : 'Layout',
        keywords: [it.title],
        disabled: false,
        icon: <ItemIcon item={it} />,
        run: () => useProject.getState().open(it.id),
      };
    });
    return [...cmds, ...objs];
  }, [items]);

  const ranked = useMemo(
    () => fuzzyFilter(q, entries, (e) => [e.title, ...e.keywords, e.detail]).slice(0, 60),
    [q, entries],
  );

  useEffect(() => setActive(0), [q]);
  useEffect(() => inputRef.current?.focus(), []);
  useEffect(() => {
    listRef.current?.querySelector(`[data-idx="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [active]);

  const close = (restore = true) => {
    setOpen(false);
    if (restore) prevFocus.current?.focus?.();
  };
  const runAt = (i: number) => {
    const r = ranked[i];
    if (!r || r.item.disabled) return;
    close();
    void r.item.run();
  };

  return (
    <div className="palette-scrim" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="palette" role="dialog" aria-label="Command palette" data-testid="palette">
        <div className="palette-input">
          <SearchIcon size={18} />
          <input
            ref={inputRef}
            value={q}
            placeholder="Type a command or the name of a table"
            aria-label="Search commands"
            aria-controls="palette-list"
            aria-activedescendant={ranked[active] ? `pal-${active}` : undefined}
            role="combobox"
            aria-expanded="true"
            spellCheck={false}
            data-testid="palette-input"
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                setActive((a) => Math.min(ranked.length - 1, a + 1));
              } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                setActive((a) => Math.max(0, a - 1));
              } else if (e.key === 'PageDown') {
                e.preventDefault();
                setActive((a) => Math.min(ranked.length - 1, a + 8));
              } else if (e.key === 'PageUp') {
                e.preventDefault();
                setActive((a) => Math.max(0, a - 8));
              } else if (e.key === 'Enter') {
                e.preventDefault();
                runAt(active);
              } else if (e.key === 'Escape') {
                e.preventDefault();
                close();
              } else return;
              e.stopPropagation();
            }}
          />
        </div>
        <div className="palette-list scroll-area" role="listbox" id="palette-list" ref={listRef}>
          {ranked.length === 0 && <div className="palette-empty">No command matches "{q}".</div>}
          {ranked.map((r, i) => (
            <div
              key={r.item.key}
              id={`pal-${i}`}
              data-idx={i}
              role="option"
              aria-selected={i === active}
              aria-disabled={r.item.disabled || undefined}
              data-testid={`palette-item-${r.item.key}`}
              className={'palette-item' + (i === active ? ' is-active' : '') + (r.item.disabled ? ' is-disabled' : '')}
              onMouseMove={() => setActive(i)}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => runAt(i)}
            >
              <span className="palette-icon">{r.item.icon}</span>
              <span className="palette-title">
                <Highlight text={r.item.title} positions={r.match.positions} />
              </span>
              <span className="palette-detail">{r.item.detail}</span>
              {r.item.shortcut && <kbd className="kbd">{formatShortcut(r.item.shortcut)}</kbd>}
            </div>
          ))}
        </div>
        <div className="palette-foot" aria-hidden="true">
          <span>
            <kbd className="kbd">↑↓</kbd> move
          </span>
          <span>
            <kbd className="kbd">Enter</kbd> run
          </span>
          <span>
            <kbd className="kbd">Esc</kbd> close
          </span>
          <span className="palette-count">{ranked.length} {ranked.length === 1 ? 'result' : 'results'}</span>
        </div>
      </div>
    </div>
  );
}
