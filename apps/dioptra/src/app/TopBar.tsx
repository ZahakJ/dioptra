// The top bar (DESIGN.md §12.5): the app mark, the project title (click to rename), the command
// field that opens the palette, the theme control and a compact "Menu" button that opens the
// whole command tree. There is no in-window menu bar; on macOS the native menu bar carries the
// same tree and the Menu button is hidden.
import { Menu as MenuIcon, Monitor, Moon, Search, Sun } from 'lucide-react';
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import * as cmd from '../engine/commands';
import { useProject } from '../store/project';
import { useEngineStatus } from '../store/status';
import { mutate } from '../store/sync';
import { useUi, type ThemePref } from '../store/ui';
import { Menu, Segmented, type MenuEntry } from '../ui';
import { commands } from './commands';
import { MENU_ICONS } from './icons';
import { formatShortcut, isMac } from './keys';
import { MENUS, type MenuName } from './registry';

function useRegistryVersion(): number {
  return useSyncExternalStore(
    (fn) => commands.subscribe(fn),
    commands.getVersion,
    commands.getVersion,
  );
}

/** One menu of the registry as menu entries (groups separated by rules). */
export function menuEntries(menu: MenuName): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const group of commands.menu(menu)) {
    if (out.length) out.push('separator');
    for (const c of group) {
      out.push({
        label: c.title.replace(/\.\.\.$/, '…'),
        shortcut: c.shortcut,
        disabled: !commands.isEnabled(c),
        checked: c.checked ? c.checked() : undefined,
        testId: `menu-${c.id}`,
        run: () => commands.run(c.id),
      });
    }
  }
  return out;
}

/** The whole command tree: one submenu per menu of the registry. */
export function commandTree(): MenuEntry[] {
  return MENUS.filter((m) => commands.menu(m).length > 0).map((m) => ({
    label: m,
    icon: MENU_ICONS[m],
    testId: `menubar-${m}`,
    items: menuEntries(m),
  }));
}

/** Whether the native menu bar carries the commands (the desktop app on macOS). */
export function nativeMenuShown(backend: string | null | undefined): boolean {
  return backend === 'tauri' && isMac();
}

const THEMES: { value: ThemePref; label: React.ReactNode; aria: string; hint: string; testId: string }[] = [
  { value: 'light', label: <Sun size={16} strokeWidth={1.5} />, aria: 'Light', hint: 'Light theme', testId: 'theme-light' },
  { value: 'dark', label: <Moon size={16} strokeWidth={1.5} />, aria: 'Dark', hint: 'Dark theme', testId: 'theme-dark' },
  { value: 'system', label: <Monitor size={16} strokeWidth={1.5} />, aria: 'System', hint: 'Follow the system', testId: 'theme-system' },
];

export function TopBar() {
  useRegistryVersion();
  const theme = useUi((s) => s.theme);
  const setTheme = useUi((s) => s.setTheme);
  const setPaletteOpen = useUi((s) => s.setPaletteOpen);
  const backend = useEngineStatus((s) => s.backend);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuBtn = useRef<HTMLButtonElement>(null);
  const showMenuButton = !nativeMenuShown(backend);
  const base = import.meta.env.BASE_URL ?? '/';
  return (
    <header className="topbar" data-testid="topbar">
      <div className="topbar-left">
        <span className="brand" aria-label="Dioptra">
          <img className="brand-mark" src={`${base}icon-small.svg`} width={18} height={18} alt="" />
          <span className="brand-name">Dioptra</span>
        </span>
        <span className="topbar-slash" aria-hidden="true">
          /
        </span>
        <ProjectTitle />
      </div>
      <button type="button" className="cmd-field" onClick={() => setPaletteOpen(true)} data-testid="open-palette" aria-label="Search commands">
        <Search size={16} strokeWidth={1.5} aria-hidden="true" />
        <span className="cmd-field-text">Search commands</span>
        <kbd className="kbd cmd-field-kbd">{formatShortcut('Mod+K')}</kbd>
      </button>
      <div className="topbar-right">
        <Segmented label="Theme" testId="theme-control" choices={THEMES} value={theme} onChange={setTheme} />
        {showMenuButton && (
          <>
            <button
              ref={menuBtn}
              type="button"
              className={'btn btn-quiet btn-sm topbar-menu' + (menuOpen ? ' is-on' : '')}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              data-testid="app-menu"
              onClick={() => setMenuOpen((o) => !o)}
            >
              <MenuIcon size={16} strokeWidth={1.5} aria-hidden="true" />
              Menu
            </button>
            <Menu
              anchor={menuBtn.current}
              open={menuOpen}
              onClose={() => setMenuOpen(false)}
              items={commandTree()}
              label="Menu"
              placement="bottom-end"
              testId="app-menu-popover"
            />
          </>
        )}
      </div>
    </header>
  );
}

function ProjectTitle() {
  const view = useProject((s) => s.view);
  const [editing, setEditing] = useState(false);
  const [v, setV] = useState('');
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    if (editing) {
      ref.current?.focus();
      ref.current?.select();
    }
  }, [editing]);
  const title = view?.title ?? 'Untitled';
  const finish = async (commit: boolean) => {
    if (done.current) return;
    done.current = true;
    setEditing(false);
    const t = v.trim();
    if (commit && t && t !== title) await mutate((e) => cmd.setProjectTitle(e, t));
  };
  if (editing) {
    return (
      <input
        ref={ref}
        className="input input-sm topbar-title-input"
        value={v}
        aria-label="Project title"
        data-testid="project-title-input"
        spellCheck={false}
        onChange={(e) => setV(e.target.value)}
        onBlur={() => void finish(true)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void finish(true);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            void finish(false);
          }
          e.stopPropagation();
        }}
      />
    );
  }
  return (
    <button
      type="button"
      className="topbar-title"
      data-testid="project-title"
      data-tip={view?.path ? `${view.path} (click to rename)` : 'Click to rename the project'}
      onClick={() => {
        done.current = false;
        setV(title);
        setEditing(true);
      }}
    >
      <span className="topbar-title-text">{title}</span>
      {view?.dirty && (
        <span className="dirty-dot" aria-label="unsaved changes" data-tip="Unsaved changes">
          <i aria-hidden="true" />
        </span>
      )}
    </button>
  );
}
