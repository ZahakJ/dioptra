import { describe, expect, it, vi } from 'vitest';
import { formatShortcut, matchesShortcut, parseShortcut } from './keys';
import { CommandRegistry, type AppCommand } from './registry';

const key = (k: string, mods: Partial<{ ctrl: boolean; meta: boolean; shift: boolean; alt: boolean }> = {}) => ({
  key: k,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
  shiftKey: !!mods.shift,
  altKey: !!mods.alt,
});

const cmd = (over: Partial<AppCommand> & { id: string }): AppCommand => ({ title: over.id, run: () => undefined, ...over });

describe('shortcuts', () => {
  it('parses chords', () => {
    expect(parseShortcut('Mod+Shift+Z')).toEqual({ mod: true, ctrl: false, shift: true, alt: false, key: 'Z' });
    expect(parseShortcut('Mod+Alt+-').key).toBe('-');
    expect(parseShortcut('F2')).toEqual({ mod: false, ctrl: false, shift: false, alt: false, key: 'F2' });
  });

  it('maps Mod to Ctrl off macOS and Cmd on macOS', () => {
    expect(matchesShortcut('Mod+K', key('k', { ctrl: true }), false)).toBe(true);
    expect(matchesShortcut('Mod+K', key('k', { meta: true }), false)).toBe(false);
    expect(matchesShortcut('Mod+K', key('k', { meta: true }), true)).toBe(true);
    expect(matchesShortcut('Mod+K', key('k', { ctrl: true }), true)).toBe(false);
  });

  it('requires the exact modifiers for letters', () => {
    expect(matchesShortcut('Mod+Z', key('z', { ctrl: true }), false)).toBe(true);
    expect(matchesShortcut('Mod+Z', key('Z', { ctrl: true, shift: true }), false)).toBe(false);
    expect(matchesShortcut('Mod+Shift+Z', key('Z', { ctrl: true, shift: true }), false)).toBe(true);
    expect(matchesShortcut('Delete', key('Delete'), false)).toBe(true);
    expect(matchesShortcut('Delete', key('Delete', { ctrl: true }), false)).toBe(false);
  });

  it('formats for display', () => {
    expect(formatShortcut('Mod+Shift+Z', false)).toBe('Ctrl+Shift+Z');
    expect(formatShortcut('Mod+K', true)).toBe('⌘K');
    expect(formatShortcut('Delete', false)).toBe('Del');
  });
});

describe('CommandRegistry', () => {
  it('registers, lists and refuses duplicates', () => {
    const r = new CommandRegistry();
    const undo = r.register(cmd({ id: 'a' }), cmd({ id: 'b' }));
    expect(r.all().map((c) => c.id)).toEqual(['a', 'b']);
    expect(() => r.register(cmd({ id: 'a' }))).toThrow(/already registered/);
    undo();
    expect(r.all()).toEqual([]);
  });

  it('groups menu entries in registration order', () => {
    const r = new CommandRegistry();
    r.register(
      cmd({ id: 'new', menu: 'File', group: '1' }),
      cmd({ id: 'save', menu: 'File', group: '2' }),
      cmd({ id: 'open', menu: 'File', group: '1' }),
      cmd({ id: 'undo', menu: 'Edit' }),
    );
    expect(r.menu('File').map((g) => g.map((c) => c.id))).toEqual([['new', 'open'], ['save']]);
    expect(r.menu('Help')).toEqual([]);
  });

  it('matches keys respecting scope and text inputs', () => {
    const r = new CommandRegistry();
    r.register(
      cmd({ id: 'palette', shortcut: 'Mod+K', inInputs: true }),
      cmd({ id: 'clear', shortcut: 'Delete', altShortcuts: ['Backspace'], scope: 'grid' }),
      cmd({ id: 'undo', shortcut: 'Mod+Z' }),
    );
    const ctx = { gridFocused: false, inInput: false, mac: false };
    expect(r.match(key('k', { ctrl: true }), ctx)?.id).toBe('palette');
    expect(r.match(key('Delete'), ctx)).toBeUndefined();
    expect(r.match(key('Backspace'), { ...ctx, gridFocused: true })?.id).toBe('clear');
    expect(r.match(key('z', { ctrl: true }), { ...ctx, inInput: true })).toBeUndefined();
    expect(r.match(key('k', { ctrl: true }), { ...ctx, inInput: true })?.id).toBe('palette');
  });

  it('runs only enabled commands and notifies subscribers', async () => {
    const r = new CommandRegistry();
    const fn = vi.fn();
    let on = false;
    r.register(cmd({ id: 'x', run: fn, enabled: () => on }));
    expect(await r.run('x')).toBe(false);
    on = true;
    expect(await r.run('x')).toBe(true);
    expect(fn).toHaveBeenCalledTimes(1);
    const sub = vi.fn();
    const v = r.getVersion();
    r.subscribe(sub);
    r.register(cmd({ id: 'y' }));
    expect(sub).toHaveBeenCalled();
    expect(r.getVersion()).toBeGreaterThan(v);
  });

  it('treats a throwing enabled() as disabled', () => {
    const r = new CommandRegistry();
    const c = cmd({ id: 'z', enabled: () => { throw new Error('no'); } });
    r.register(c);
    expect(r.isEnabled(c)).toBe(false);
  });
});
