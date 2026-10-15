// The command registry: one list of commands that the keyboard handler, the menu bar and the
// command palette all read. Phase 5B registers its own commands (Analyze, New graph, ...) here.
import { matchesShortcut, type KeyLike } from './keys';

export type MenuName = 'File' | 'Edit' | 'Insert' | 'Data' | 'View' | 'Analyze' | 'Graph' | 'Records' | 'Help';
export const MENUS: readonly MenuName[] = ['File', 'Edit', 'Insert', 'Data', 'View', 'Analyze', 'Graph', 'Records', 'Help'];

export type CommandScope = 'global' | 'grid';

export interface AppCommand {
  /** Stable id such as `file.save`. */
  id: string;
  /** Title in menus and the palette. */
  title: string;
  /** Menu that lists the command (omit for palette-only commands). */
  menu?: MenuName;
  /** Commands in a menu are grouped by this key, groups separated by a rule, in order. */
  group?: string;
  /** Primary shortcut, e.g. `Mod+Shift+Z` (`Mod` = Cmd on macOS, Ctrl elsewhere). */
  shortcut?: string;
  /** Further shortcuts that run the command (not displayed). */
  altShortcuts?: string[];
  /** Extra words the palette matches. */
  keywords?: string[];
  /** `grid`: the shortcut only fires while the grid has focus. Default `global`. */
  scope?: CommandScope;
  /** Whether the shortcut fires while typing in a text field. Default false. */
  inInputs?: boolean;
  /**
   * The browser's own event does the work when the key is pressed (copy, cut, paste): the
   * shortcut is shown but the key handler leaves the event alone.
   */
  native?: boolean;
  /** Hidden from the palette. */
  hidden?: boolean;
  enabled?: () => boolean;
  /** For toggles: shown as a check mark (with text, not colour). */
  checked?: () => boolean;
  run: () => unknown;
}

export interface KeyContext {
  gridFocused: boolean;
  inInput: boolean;
  mac?: boolean;
}

export class CommandRegistry {
  private commands = new Map<string, AppCommand>();
  private listeners = new Set<() => void>();
  private version = 0;

  register(...cmds: AppCommand[]): () => void {
    for (const c of cmds) {
      if (this.commands.has(c.id)) throw new Error(`command ${c.id} is already registered`);
      this.commands.set(c.id, c);
    }
    this.changed();
    return () => {
      for (const c of cmds) this.commands.delete(c.id);
      this.changed();
    };
  }

  get(id: string): AppCommand | undefined {
    return this.commands.get(id);
  }

  all(): AppCommand[] {
    return [...this.commands.values()];
  }

  /** Commands of one menu, split into groups in registration order. */
  menu(name: MenuName): AppCommand[][] {
    const groups = new Map<string, AppCommand[]>();
    for (const c of this.commands.values()) {
      if (c.menu !== name) continue;
      const g = c.group ?? '';
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g)!.push(c);
    }
    return [...groups.values()];
  }

  isEnabled(c: AppCommand): boolean {
    try {
      return c.enabled ? c.enabled() : true;
    } catch {
      return false;
    }
  }

  /** The command a key event triggers in this context, if any. */
  match(e: KeyLike, ctx: KeyContext): AppCommand | undefined {
    for (const c of this.commands.values()) {
      const keys = [c.shortcut, ...(c.altShortcuts ?? [])].filter((k): k is string => !!k);
      if (!keys.some((k) => matchesShortcut(k, e, ctx.mac))) continue;
      if ((c.scope ?? 'global') === 'grid' && !ctx.gridFocused) continue;
      if (ctx.inInput && !c.inInputs) continue;
      return c;
    }
    return undefined;
  }

  async run(id: string): Promise<boolean> {
    const c = this.commands.get(id);
    if (!c || !this.isEnabled(c)) return false;
    await c.run();
    return true;
  }

  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Changes whenever commands are added or removed (for useSyncExternalStore). */
  getVersion = (): number => this.version;

  private changed(): void {
    this.version++;
    for (const l of this.listeners) l();
  }
}
