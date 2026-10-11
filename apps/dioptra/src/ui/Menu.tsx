// Menus: a keyboard-navigable list of commands with optional group headings, check marks,
// icons, shortcuts right-aligned in mono, and submenus. Used by the top bar's Menu button,
// context menus and the small "Add" and "Copy" menus.
import { Check, ChevronRight } from 'lucide-react';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { formatShortcut } from '../app/keys';
import { Popover, type Anchor, type Placement } from './Popover';

export interface MenuItem {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  checked?: boolean;
  icon?: ReactNode;
  testId?: string;
  run: () => unknown;
}
export interface MenuSubmenu {
  label: string;
  icon?: ReactNode;
  testId?: string;
  items: MenuEntry[];
}
export interface MenuHeading {
  heading: string;
}
export type MenuEntry = MenuItem | MenuSubmenu | MenuHeading | 'separator';

const isSub = (e: MenuEntry): e is MenuSubmenu => typeof e === 'object' && 'items' in e;
const isItem = (e: MenuEntry): e is MenuItem => typeof e === 'object' && 'run' in e;
const focusable = (e: MenuEntry) => isSub(e) || (isItem(e) && !e.disabled);

export function MenuList({
  items,
  onClose,
  label,
  autoFocus = true,
  onBack,
  testId,
}: {
  items: MenuEntry[];
  /** Close the whole menu (after running an item, Escape, Tab). */
  onClose: () => void;
  label: string;
  autoFocus?: boolean;
  /** ArrowLeft in a submenu returns to the parent. */
  onBack?: () => void;
  testId?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const enabled = items.map((it, i) => (focusable(it) ? i : -1)).filter((i) => i >= 0);
  const [active, setActive] = useState<number>(autoFocus ? (enabled[0] ?? -1) : -1);
  const [openSub, setOpenSub] = useState<number | null>(null);
  const rowRefs = useRef(new Map<number, HTMLDivElement>());
  const hoverTimer = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  useEffect(() => () => window.clearTimeout(hoverTimer.current), []);
  useEffect(() => {
    rowRefs.current.get(active)?.scrollIntoView?.({ block: 'nearest' });
  }, [active]);

  const runAt = (i: number) => {
    const it = items[i];
    if (!it) return;
    if (isSub(it)) {
      setOpenSub(i);
      return;
    }
    if (!isItem(it) || it.disabled) return;
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
      data-testid={testId}
      onKeyDown={(e) => {
        const pos = enabled.indexOf(active);
        if (e.key === 'ArrowDown') {
          setActive(enabled[(pos + 1) % enabled.length] ?? -1);
        } else if (e.key === 'ArrowUp') {
          setActive(enabled[(pos - 1 + enabled.length) % enabled.length] ?? -1);
        } else if (e.key === 'Home') {
          setActive(enabled[0] ?? -1);
        } else if (e.key === 'End') {
          setActive(enabled[enabled.length - 1] ?? -1);
        } else if (e.key === 'Enter' || e.key === ' ') {
          runAt(active);
        } else if (e.key === 'ArrowRight' && isSub(items[active] ?? 'separator')) {
          setOpenSub(active);
        } else if (e.key === 'ArrowLeft' && onBack) {
          onBack();
        } else if (e.key === 'Escape') {
          if (onBack) onBack();
          else onClose();
        } else if (e.key === 'Tab') {
          onClose();
        } else if (e.key.length === 1 && /\S/.test(e.key)) {
          // Typeahead: the next item starting with the letter.
          const k = e.key.toLowerCase();
          const order = [...enabled.slice(pos + 1), ...enabled.slice(0, pos + 1)];
          const hit = order.find((i) => {
            const it = items[i]!;
            return (isItem(it) || isSub(it)) && it.label.toLowerCase().startsWith(k);
          });
          if (hit === undefined) return;
          setActive(hit);
        } else return;
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      {items.map((it, i) => {
        if (it === 'separator') return <div key={i} className="menu-sep" role="separator" />;
        if ('heading' in it)
          return (
            <div key={i} className="menu-heading" role="presentation">
              {it.heading}
            </div>
          );
        const sub = isSub(it);
        const disabled = isItem(it) && it.disabled;
        const checked = isItem(it) ? it.checked : undefined;
        return (
          <div
            key={i}
            ref={(el) => {
              if (el) rowRefs.current.set(i, el);
              else rowRefs.current.delete(i);
            }}
            role={checked !== undefined ? 'menuitemcheckbox' : 'menuitem'}
            aria-checked={checked}
            aria-disabled={disabled || undefined}
            aria-haspopup={sub ? 'menu' : undefined}
            aria-expanded={sub ? openSub === i : undefined}
            data-testid={it.testId}
            className={'menu-item' + (i === active ? ' is-active' : '') + (disabled ? ' is-disabled' : '') + (sub && openSub === i ? ' is-open' : '')}
            onMouseEnter={() => {
              if (disabled) return;
              setActive(i);
              window.clearTimeout(hoverTimer.current);
              hoverTimer.current = window.setTimeout(() => setOpenSub(sub ? i : null), sub ? 80 : 160);
            }}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => runAt(i)}
          >
            <span className="menu-lead" aria-hidden="true">
              {checked ? <Check size={14} strokeWidth={2} /> : it.icon}
            </span>
            <span className="menu-label">{it.label}</span>
            {sub ? (
              <ChevronRight className="menu-chev" size={14} strokeWidth={1.5} aria-hidden="true" />
            ) : (
              isItem(it) && it.shortcut && <kbd className="kbd menu-kbd">{formatShortcut(it.shortcut)}</kbd>
            )}
          </div>
        );
      })}
      {openSub !== null && isSub(items[openSub] ?? 'separator') && (
        <Popover
          anchor={rowRefs.current.get(openSub) ?? null}
          open
          placement="right-start"
          onClose={() => setOpenSub(null)}
          className="menu-pop"
          closeOnBlur={false}
        >
          <MenuList
            items={(items[openSub] as MenuSubmenu).items}
            label={(items[openSub] as MenuSubmenu).label}
            onClose={onClose}
            onBack={() => {
              setOpenSub(null);
              ref.current?.focus();
            }}
          />
        </Popover>
      )}
    </div>
  );
}

/** A menu in a popover, anchored to a button or a point. */
export function Menu({
  anchor,
  open,
  onClose,
  items,
  label,
  placement = 'bottom-start',
  testId,
}: {
  anchor: Anchor;
  open: boolean;
  onClose: () => void;
  items: MenuEntry[];
  label: string;
  placement?: Placement;
  testId?: string;
}) {
  return (
    <Popover anchor={anchor} open={open} onClose={onClose} placement={placement} className="menu-pop" testId={testId}>
      <MenuList items={items} onClose={onClose} label={label} />
    </Popover>
  );
}
