// A keyboard-navigable popup menu used for context menus and the menu bar's dropdowns.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { create } from 'zustand';
import { formatShortcut } from './keys';

export interface MenuItem {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  checked?: boolean;
  testId?: string;
  run: () => unknown;
}
export type MenuEntry = MenuItem | 'separator';

export function MenuList({
  items,
  onClose,
  label,
  autoFocus = true,
  onKeyNav,
}: {
  items: MenuEntry[];
  onClose: () => void;
  label: string;
  autoFocus?: boolean;
  /** Left/right arrows (menu bar switches menus). */
  onKeyNav?: (dir: -1 | 1) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const enabled = items
    .map((it, i) => ({ it, i }))
    .filter((x): x is { it: MenuItem; i: number } => x.it !== 'separator' && !x.it.disabled)
    .map((x) => x.i);
  const [active, setActive] = useState<number>(enabled[0] ?? -1);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  const runAt = (i: number) => {
    const it = items[i];
    if (!it || it === 'separator' || it.disabled) return;
    onClose();
    void it.run();
  };
  return (
    <div
      ref={ref}
      className="menu"
      role="menu"
      aria-label={label}
      tabIndex={-1}
      onKeyDown={(e) => {
        const pos = enabled.indexOf(active);
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          setActive(enabled[(pos + 1) % enabled.length] ?? -1);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          setActive(enabled[(pos - 1 + enabled.length) % enabled.length] ?? -1);
        } else if (e.key === 'Home') {
          e.preventDefault();
          setActive(enabled[0] ?? -1);
        } else if (e.key === 'End') {
          e.preventDefault();
          setActive(enabled[enabled.length - 1] ?? -1);
        } else if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          runAt(active);
        } else if (e.key === 'Escape' || e.key === 'Tab') {
          e.preventDefault();
          onClose();
        } else if (onKeyNav && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
          e.preventDefault();
          onKeyNav(e.key === 'ArrowLeft' ? -1 : 1);
        } else return;
        e.stopPropagation();
      }}
    >
      {items.map((it, i) =>
        it === 'separator' ? (
          <div key={i} className="menu-sep" role="separator" />
        ) : (
          <div
            key={i}
            role={it.checked !== undefined ? 'menuitemcheckbox' : 'menuitem'}
            aria-checked={it.checked}
            aria-disabled={it.disabled || undefined}
            data-testid={it.testId}
            className={'menu-item' + (i === active ? ' is-active' : '') + (it.disabled ? ' is-disabled' : '')}
            onMouseEnter={() => !it.disabled && setActive(i)}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => runAt(i)}
          >
            <span className="menu-check" aria-hidden="true">
              {it.checked ? '✓' : ''}
            </span>
            <span className="menu-label">{it.label}</span>
            {it.shortcut && <kbd className="menu-kbd">{formatShortcut(it.shortcut)}</kbd>}
          </div>
        ),
      )}
    </div>
  );
}

interface ContextMenuState {
  open: { x: number; y: number; items: MenuEntry[]; label: string } | null;
}

export const useContextMenu = create<ContextMenuState>(() => ({ open: null }));

export function showContextMenu(x: number, y: number, items: MenuEntry[], label = 'Context menu'): void {
  useContextMenu.setState({ open: { x, y, items, label } });
}

export function ContextMenuHost() {
  const open = useContextMenu((s) => s.open);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const close = () => useContextMenu.setState({ open: null });
  useLayoutEffect(() => {
    if (!open || !ref.current) return setPos(null);
    const r = ref.current.getBoundingClientRect();
    setPos({
      x: Math.max(4, Math.min(open.x, window.innerWidth - r.width - 4)),
      y: Math.max(4, Math.min(open.y, window.innerHeight - r.height - 4)),
    });
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('blur', close);
      if (prev && document.activeElement === document.body) prev.focus();
    };
  }, [open]);
  if (!open) return null;
  return (
    <div
      ref={ref}
      className="popover context-menu"
      data-testid="context-menu"
      style={{ left: pos?.x ?? open.x, top: pos?.y ?? open.y, opacity: pos ? 1 : 0 }}
    >
      <MenuList items={open.items} onClose={close} label={open.label} />
    </div>
  );
}
