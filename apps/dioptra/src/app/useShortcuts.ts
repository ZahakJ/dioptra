// The one keyboard handler for registry shortcuts.
import { useEffect } from 'react';
import { gridHasFocus } from '../grid/store';
import { commands } from './commands';

function isTextInput(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.closest) return false;
  return !!el.closest('input, textarea, select, [contenteditable="true"]');
}

export function useShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const c = commands.match(e, { gridFocused: gridHasFocus(), inInput: isTextInput(e.target) });
      if (!c || c.native) return;
      e.preventDefault();
      if (!commands.isEnabled(c)) return;
      void commands.run(c.id);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
