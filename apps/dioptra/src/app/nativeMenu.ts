// The native menu bar on macOS: the desktop shell builds it from the same command registry the
// in-window Menu button shows (set_native_menu), and sends the chosen command id back as a
// `dioptra://menu` event. Linux and Windows keep the designed in-window menu instead.
import { commands } from './commands';
import { MENUS } from './registry';

/** Keys a native accelerator may take from the web view: Mod plus a letter or digit, except
 *  the text-editing chords (undo, select all, clipboard) that fields need for themselves. */
const KEEP_IN_WEBVIEW = new Set(['Mod+Z', 'Mod+Shift+Z', 'Mod+Y', 'Mod+A', 'Mod+C', 'Mod+V', 'Mod+X']);

export function nativeAccelerator(shortcut: string | undefined, scope: string | undefined): string | null {
  if (!shortcut || scope === 'grid' || KEEP_IN_WEBVIEW.has(shortcut)) return null;
  const parts = shortcut.split('+');
  const key = parts.pop() ?? '';
  if (!parts.includes('Mod') || !/^[A-Za-z0-9]$/.test(key)) return null;
  const mods = parts.map((p) => (p === 'Mod' ? 'CmdOrCtrl' : p));
  return [...mods, key.toUpperCase()].join('+');
}

const PREDEFINED: Record<string, string> = { 'edit.cut': 'cut', 'edit.copy': 'copy', 'edit.paste': 'paste' };

export function nativeMenuModel() {
  return MENUS.filter((m) => commands.menu(m).length > 0).map((m) => ({
    title: m,
    groups: commands.menu(m).map((g) =>
      g.map((c) => ({
        id: c.id,
        title: c.title.replace(/\.\.\.$/, '…'),
        accelerator: nativeAccelerator(c.shortcut, c.scope),
        predefined: PREDEFINED[c.id] ?? null,
      })),
    ),
  }));
}

let installed = false;

/** Install the native menu (macOS desktop only) and route its events to the registry. */
export async function installNativeMenu(): Promise<void> {
  if (installed) return;
  installed = true;
  try {
    const [{ invoke }, { listen }] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]);
    await listen<string>('dioptra://menu', (e) => void commands.run(e.payload));
    let pending: ReturnType<typeof setTimeout> | undefined;
    const push = () => {
      clearTimeout(pending);
      pending = setTimeout(() => void invoke('set_native_menu', { menus: nativeMenuModel() }).catch(() => undefined), 50);
    };
    commands.subscribe(push);
    push();
  } catch {
    installed = false;
  }
}
