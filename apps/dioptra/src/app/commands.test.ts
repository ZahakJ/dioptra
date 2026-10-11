import { describe, expect, it } from 'vitest';
import { commands, CORE_COMMANDS } from './commands';
import { parseShortcut } from './keys';

describe('core commands', () => {
  it('are all registered', () => {
    for (const c of CORE_COMMANDS) expect(commands.get(c.id)).toBe(c);
  });

  it('have unique shortcuts per scope', () => {
    const seen = new Map<string, string>();
    for (const c of CORE_COMMANDS) {
      for (const s of [c.shortcut, ...(c.altShortcuts ?? [])]) {
        if (!s) continue;
        const k = `${c.scope ?? 'global'}:${s.toLowerCase()}`;
        expect(seen.get(k), `${s} used by ${seen.get(k)} and ${c.id}`).toBeUndefined();
        seen.set(k, c.id);
      }
    }
  });

  it('cover the menus the shell promises', () => {
    const ids = new Set(CORE_COMMANDS.map((c) => c.id));
    for (const id of [
      'file.new', 'file.open', 'file.save', 'file.saveAs', 'file.import', 'file.export',
      'edit.undo', 'edit.redo', 'edit.cut', 'edit.copy', 'edit.paste', 'edit.fillDown', 'edit.exclude', 'edit.clear',
      'insert.rowsAbove', 'insert.dataset', 'view.toggleTheme', 'view.rail', 'view.inspector', 'help.shortcuts', 'table.new',
    ]) {
      expect(ids.has(id), id).toBe(true);
    }
  });

  it('use parseable shortcuts and plain titles', () => {
    for (const c of CORE_COMMANDS) {
      if (c.shortcut) expect(parseShortcut(c.shortcut).key.length).toBeGreaterThan(0);
      expect(c.title).not.toMatch(/!/);
    }
  });
});
