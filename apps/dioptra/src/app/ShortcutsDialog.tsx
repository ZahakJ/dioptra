import { useUi } from '../store/ui';
import { commands } from './commands';
import { formatShortcut } from './keys';

const GRID_KEYS: [string, string][] = [
  ['Arrows', 'Move'],
  ['Shift+Arrows', 'Extend the selection'],
  ['Ctrl+Arrows', 'Jump to the edge of the data'],
  ['Tab / Shift+Tab', 'Move right / left'],
  ['Enter / F2', 'Edit the active cell'],
  ['Enter / Shift+Enter (editing)', 'Commit and move down / up'],
  ['Esc (editing)', 'Cancel the edit'],
  ['Home / End', 'First / last column'],
  ['Ctrl+Home / Ctrl+End', 'First / last cell'],
  ['PageUp / PageDown', 'Move a screen up / down'],
  ['Type any character', 'Replace the cell'],
];

export function ShortcutsDialog() {
  const open = useUi((s) => s.shortcutsOpen);
  const setOpen = useUi((s) => s.setShortcutsOpen);
  if (!open) return null;
  const withKeys = commands.all().filter((c) => c.shortcut);
  return (
    <div className="modal-scrim" onMouseDown={(e) => e.target === e.currentTarget && setOpen(false)}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        onKeyDown={(e) => {
          if (e.key === 'Escape') setOpen(false);
          e.stopPropagation();
        }}
      >
        <div className="modal-body modal-wide">
          <header className="modal-head">
            <h2>Keyboard shortcuts</h2>
            <p>Every command is also in the palette ({formatShortcut('Mod+K')}).</p>
          </header>
          <div className="shortcut-cols">
            <table className="shortcut-table">
              <caption>Commands</caption>
              <tbody>
                {withKeys.map((c) => (
                  <tr key={c.id}>
                    <td>{c.title.replace(/\.\.\.$/, '')}</td>
                    <td>
                      <kbd className="menu-kbd">{formatShortcut(c.shortcut!)}</kbd>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <table className="shortcut-table">
              <caption>In the grid</caption>
              <tbody>
                {GRID_KEYS.map(([k, v]) => (
                  <tr key={k}>
                    <td>{v}</td>
                    <td>
                      <kbd className="menu-kbd">{k}</kbd>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <footer className="modal-foot">
            <button type="button" className="btn btn-primary" autoFocus onClick={() => setOpen(false)}>
              Close
            </button>
          </footer>
        </div>
      </div>
    </div>
  );
}
