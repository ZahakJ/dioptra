// Context menus: one host shows a designed menu at the pointer (or under a button). The menu
// itself is the control set's Menu.
import { useEffect } from 'react';
import { create } from 'zustand';
import { Menu, type MenuEntry } from '../ui';

export { MenuList, type MenuEntry, type MenuItem } from '../ui';

interface ContextMenuState {
  open: { x: number; y: number; items: MenuEntry[]; label: string } | null;
}

export const useContextMenu = create<ContextMenuState>(() => ({ open: null }));

export function showContextMenu(x: number, y: number, items: MenuEntry[], label = 'Context menu'): void {
  useContextMenu.setState({ open: { x, y, items, label } });
}

export function ContextMenuHost() {
  const open = useContextMenu((s) => s.open);
  const close = () => useContextMenu.setState({ open: null });
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    return () => {
      if (prev && (document.activeElement === document.body || !document.activeElement)) prev.focus();
    };
  }, [open]);
  if (!open) return null;
  return (
    <Menu
      key={`${open.x}:${open.y}:${open.label}`}
      anchor={{ x: open.x, y: open.y }}
      open
      onClose={close}
      items={open.items}
      label={open.label}
      placement="bottom-start"
      testId="context-menu"
    />
  );
}
