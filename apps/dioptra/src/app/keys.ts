// Shortcut strings such as "Mod+Shift+Z": parse, match against keyboard events, display.
// `Mod` is Cmd on macOS and Ctrl elsewhere.

export interface Chord {
  mod: boolean;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

export const isMac = (): boolean =>
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

export function parseShortcut(s: string): Chord {
  const parts = s.split('+');
  const key = parts.pop() ?? '';
  const set = new Set(parts.map((p) => p.toLowerCase()));
  return { mod: set.has('mod'), ctrl: set.has('ctrl'), shift: set.has('shift'), alt: set.has('alt'), key };
}

/** Normalise a KeyboardEvent key so `k` and `K` (with Shift) compare equal. */
function normKey(key: string): string {
  if (key.length === 1) return key.toLowerCase();
  if (key === 'Esc') return 'Escape';
  if (key === 'Del') return 'Delete';
  return key;
}

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export function matchesShortcut(shortcut: string, e: KeyLike, mac = isMac()): boolean {
  const c = parseShortcut(shortcut);
  const modDown = mac ? e.metaKey : e.ctrlKey;
  const wantCtrl = c.ctrl || (!mac && c.mod);
  const wantMeta = mac && c.mod;
  if (e.ctrlKey !== wantCtrl) return false;
  if (e.metaKey !== wantMeta) return false;
  if (c.mod && !modDown) return false;
  if (e.altKey !== c.alt) return false;
  const k = normKey(e.key);
  const want = normKey(c.key);
  // Shift is implied by shifted punctuation; only compare it for letters and named keys.
  if (want.length > 1 || /[a-z0-9]/.test(want)) {
    if (e.shiftKey !== c.shift) return false;
  }
  return k === want;
}

const MAC_GLYPH: Record<string, string> = { mod: '⌘', ctrl: '⌃', shift: '⇧', alt: '⌥' };
const KEY_NAME: Record<string, string> = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Escape: 'Esc',
  Delete: 'Del',
  ' ': 'Space',
};

/** Human form: "Ctrl+Shift+Z" on Linux/Windows, glyphs on macOS. */
export function formatShortcut(s: string, mac = isMac()): string {
  const c = parseShortcut(s);
  const key = KEY_NAME[c.key] ?? (c.key.length === 1 ? c.key.toUpperCase() : c.key);
  if (mac) {
    return [c.ctrl && MAC_GLYPH.ctrl, c.alt && MAC_GLYPH.alt, c.shift && MAC_GLYPH.shift, c.mod && MAC_GLYPH.mod, key]
      .filter(Boolean)
      .join('');
  }
  return [(c.mod || c.ctrl) && 'Ctrl', c.alt && 'Alt', c.shift && 'Shift', key].filter(Boolean).join('+');
}
