// The in-app menu bar, built from the command registry.
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { commands } from './commands';
import { MenuList, type MenuEntry } from './ContextMenu';
import { MENUS, type MenuName } from './registry';

function useRegistryVersion(): number {
  return useSyncExternalStore(
    (fn) => commands.subscribe(fn),
    commands.getVersion,
    commands.getVersion,
  );
}

function entriesFor(menu: MenuName): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const group of commands.menu(menu)) {
    if (out.length) out.push('separator');
    for (const c of group) {
      out.push({
        label: c.title,
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

export function MenuBar() {
  useRegistryVersion();
  const menus = MENUS.filter((m) => commands.menu(m).length > 0);
  const [open, setOpen] = useState<MenuName | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) setOpen(null);
    };
    window.addEventListener('mousedown', onDown, true);
    return () => window.removeEventListener('mousedown', onDown, true);
  }, [open]);
  const move = (dir: -1 | 1) => {
    if (!open) return;
    const i = menus.indexOf(open);
    setOpen(menus[(i + dir + menus.length) % menus.length]!);
  };
  return (
    <div className="menubar" role="menubar" aria-label="Main menu" ref={barRef}>
      {menus.map((m) => (
        <div key={m} className="menubar-slot">
          <button
            type="button"
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === m}
            className={'menubar-item' + (open === m ? ' is-open' : '')}
            data-testid={`menubar-${m}`}
            onMouseDown={(e) => {
              e.preventDefault();
              setOpen(open === m ? null : m);
            }}
            onMouseEnter={() => open && open !== m && setOpen(m)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                setOpen(m);
              }
            }}
          >
            {m}
          </button>
          {open === m && (
            <div className="popover menubar-drop">
              <MenuList items={entriesFor(m)} onClose={() => setOpen(null)} label={m} onKeyNav={move} />
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
