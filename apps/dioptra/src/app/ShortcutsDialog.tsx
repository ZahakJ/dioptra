import { useUi } from '../store/ui';
import { Button, DialogFrame, DialogHeader } from '../ui';
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
    <DialogFrame onCancel={() => setOpen(false)} label="Keyboard shortcuts" testId="shortcuts-dialog">
      <div className="modal-body modal-wide">
        <DialogHeader
          title="Keyboard shortcuts"
          sub={`Every command is also in the palette (${formatShortcut('Mod+K')}).`}
          onClose={() => setOpen(false)}
        />
        <div className="shortcut-cols">
          <table className="shortcut-table">
            <caption>Commands</caption>
            <tbody>
              {withKeys.map((c) => (
                <tr key={c.id}>
                  <td>{c.title.replace(/\.\.\.$/, '')}</td>
                  <td>
                    <kbd className="kbd">{formatShortcut(c.shortcut!)}</kbd>
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
                    <kbd className="kbd">{k}</kbd>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <footer className="modal-foot">
          <Button variant="primary" data-autofocus onClick={() => setOpen(false)}>
            Close
          </Button>
        </footer>
      </div>
    </DialogFrame>
  );
}
