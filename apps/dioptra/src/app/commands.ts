// Every command of the app, with its shortcut. The keyboard handler, the menu bar and the
// command palette all read this one registry. Phase 5B adds its commands with
// `commands.register(...)` (menus `Analyze` and `Graph` are reserved for it).
import * as grid from '../grid/actions';
import { copyMethods, exportReport, hasProjectContent } from '../graph/journal';
import { exportPrism, hasTables } from '../io/prismExport';
import { useGrid } from '../grid/store';
import { useLab } from '../store/lab';
import { useProject } from '../store/project';
import { nextTheme, resolveTheme, useUi } from '../store/ui';
import * as app from './actions';
import { RECORDS_COMMANDS } from './Records';
import { OQ_COMMANDS } from './OqDialog';
import { BATCH_COMMANDS } from '../io/batchCommands';
import { CommandRegistry, type AppCommand } from './registry';

export const commands = new CommandRegistry();

const canUndo = () => !!useProject.getState().view?.can_undo;
const canRedo = () => !!useProject.getState().view?.can_redo;
const systemDark = () =>
  typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-color-scheme: dark)').matches;

export const CORE_COMMANDS: AppCommand[] = [
  // ---- File ----
  { id: 'file.new', title: 'New project', menu: 'File', group: 'new', shortcut: 'Mod+Alt+N', run: app.newProject },
  { id: 'table.new', title: 'New table...', menu: 'File', group: 'new', shortcut: 'Mod+Alt+T', keywords: ['create', 'data'], inInputs: true, run: app.newTable },
  { id: 'file.open', title: 'Open...', menu: 'File', group: 'open', shortcut: 'Mod+O', inInputs: true, run: app.openProject },
  { id: 'file.save', title: 'Save', menu: 'File', group: 'save', shortcut: 'Mod+S', inInputs: true, run: app.saveProject },
  { id: 'file.saveAs', title: 'Save as...', menu: 'File', group: 'save', shortcut: 'Mod+Shift+S', inInputs: true, run: app.saveProjectAs },
  { id: 'file.import', title: 'Import file...', menu: 'File', group: 'io', shortcut: 'Mod+I', keywords: ['csv', 'xlsx', 'excel', 'pzfx', 'prism'], run: app.importFile },
  { id: 'file.pasteNew', title: 'Paste as new table...', menu: 'File', group: 'io', shortcut: 'Mod+Shift+V', keywords: ['clipboard', 'import'], run: () => app.pasteAsNewTable() },
  { id: 'file.export', title: 'Export table...', menu: 'File', group: 'io', shortcut: 'Mod+Shift+E', keywords: ['csv', 'tsv', 'xlsx'], enabled: grid.hasActiveTable, run: app.exportTable },
  { id: 'file.exportPrism', title: 'Export to Prism...', menu: 'File', group: 'io', keywords: ['pzfx', 'graphpad', 'prism', 'lab'], enabled: hasTables, run: exportPrism },
  { id: 'file.exportReport', title: 'Export report...', menu: 'File', group: 'io', keywords: ['docx', 'word', 'pdf', 'lab'], enabled: hasProjectContent, run: exportReport },
  { id: 'file.methods', title: 'Copy methods paragraph...', menu: 'File', group: 'io', keywords: ['methods', 'references', 'citation', 'lab'], enabled: () => !!useProject.getState().view?.items.some((i) => i.kind === 'analysis'), run: () => copyMethods('project') },
  { id: 'project.rename', title: 'Rename project...', menu: 'File', group: 'object', keywords: ['title', 'name'], run: app.renameProject },
  { id: 'object.rename', title: 'Rename object...', menu: 'File', group: 'object', keywords: ['title'], enabled: () => !!useProject.getState().activeId, run: () => app.renameObject() },
  { id: 'object.delete', title: 'Delete object...', menu: 'File', group: 'object', keywords: ['remove'], enabled: () => !!useProject.getState().activeId, run: () => app.deleteObject() },

  // ---- Edit ----
  { id: 'edit.undo', title: 'Undo', menu: 'Edit', group: 'undo', shortcut: 'Mod+Z', enabled: canUndo, run: grid.undoAction },
  { id: 'edit.redo', title: 'Redo', menu: 'Edit', group: 'undo', shortcut: 'Mod+Shift+Z', altShortcuts: ['Mod+Y'], enabled: canRedo, run: grid.redoAction },
  { id: 'edit.cut', title: 'Cut', menu: 'Edit', group: 'clip', shortcut: 'Mod+X', scope: 'grid', native: true, enabled: grid.isEditableTable, run: grid.cutSelection },
  { id: 'edit.copy', title: 'Copy', menu: 'Edit', group: 'clip', shortcut: 'Mod+C', scope: 'grid', native: true, enabled: grid.hasActiveTable, run: grid.copySelection },
  { id: 'edit.paste', title: 'Paste', menu: 'Edit', group: 'clip', shortcut: 'Mod+V', scope: 'grid', native: true, enabled: grid.isEditableTable, run: grid.pasteFromClipboard },
  { id: 'edit.editCell', title: 'Edit cell', menu: 'Edit', group: 'cell', shortcut: 'F2', scope: 'grid', native: true, enabled: grid.isEditableTable, run: () => useGrid.getState().requestEdit() },
  { id: 'edit.fillDown', title: 'Fill down', menu: 'Edit', group: 'cell', shortcut: 'Mod+D', scope: 'grid', enabled: grid.isEditableTable, run: grid.fillDownSelection },
  { id: 'edit.exclude', title: 'Exclude or include', menu: 'Edit', group: 'cell', shortcut: 'Mod+E', scope: 'grid', keywords: ['outlier', 'strike'], enabled: grid.isEditableTable, run: grid.toggleExclude },
  { id: 'edit.clear', title: 'Clear', menu: 'Edit', group: 'cell', shortcut: 'Delete', altShortcuts: ['Backspace'], scope: 'grid', keywords: ['delete', 'erase'], enabled: grid.isEditableTable, run: grid.clearSelection },
  { id: 'edit.selectAll', title: 'Select all', menu: 'Edit', group: 'select', shortcut: 'Mod+A', scope: 'grid', enabled: grid.hasActiveTable, run: grid.selectAllCells },
  { id: 'edit.columnFormula', title: 'Column formula...', menu: 'Edit', group: 'column', keywords: ['transform', 'compute', '='], enabled: grid.isEditableTable, run: () => grid.setColumnFormula() },
  { id: 'edit.decimals', title: 'Decimals...', menu: 'Edit', group: 'column', keywords: ['format', 'digits', 'precision'], enabled: grid.isEditableTable, run: () => grid.setDecimals() },
  { id: 'edit.subcolumnFormat', title: 'Subcolumn format...', menu: 'Edit', group: 'column', keywords: ['replicates', 'mean sd n'], enabled: grid.isEditableTable, run: grid.setSubcolumnFormat },
  { id: 'edit.renameDataset', title: 'Rename data set...', menu: 'Edit', group: 'column', keywords: ['column name', 'header'], enabled: grid.isEditableTable, run: () => grid.renameDataset() },
  { id: 'edit.recompute', title: 'Recompute everything', menu: 'Edit', group: 'recompute', shortcut: 'F9', keywords: ['refresh', 'formulas'], run: app.recomputeAll },

  // ---- Insert ----
  { id: 'insert.rowsAbove', title: 'Rows above', menu: 'Insert', group: 'rows', shortcut: 'Mod+Alt+I', scope: 'grid', keywords: ['insert row'], enabled: grid.isEditableTable, run: () => grid.insertRows('above') },
  { id: 'insert.rowsBelow', title: 'Rows below', menu: 'Insert', group: 'rows', keywords: ['insert row'], enabled: grid.isEditableTable, run: () => grid.insertRows('below') },
  { id: 'insert.deleteRows', title: 'Delete rows', menu: 'Insert', group: 'rows', shortcut: 'Mod+Alt+-', scope: 'grid', keywords: ['remove row'], enabled: grid.isEditableTable, run: grid.deleteRows },
  { id: 'insert.dataset', title: 'Data set', menu: 'Insert', group: 'datasets', shortcut: 'Mod+Alt+D', keywords: ['add column', 'group', 'variable'], enabled: grid.isEditableTable, run: grid.addDataset },
  { id: 'insert.removeDataset', title: 'Remove data set', menu: 'Insert', group: 'datasets', keywords: ['delete column'], enabled: grid.isEditableTable, run: () => grid.removeDataset() },

  // ---- View ----
  { id: 'view.palette', title: 'Command palette', menu: 'View', group: 'palette', shortcut: 'Mod+K', inInputs: true, hidden: true, run: () => useUi.getState().setPaletteOpen(!useUi.getState().paletteOpen) },
  {
    id: 'view.toggleTheme',
    title: 'Toggle dark theme',
    menu: 'View',
    group: 'theme',
    shortcut: 'Mod+Shift+L',
    keywords: ['dark mode', 'light', 'appearance'],
    run: () => {
      const ui = useUi.getState();
      ui.setTheme(resolveTheme(ui.theme, systemDark()) === 'dark' ? 'light' : 'dark');
    },
  },
  { id: 'view.themeLight', title: 'Light theme', menu: 'View', group: 'theme', checked: () => useUi.getState().theme === 'light', run: () => useUi.getState().setTheme('light') },
  { id: 'view.themeDark', title: 'Dark theme', menu: 'View', group: 'theme', checked: () => useUi.getState().theme === 'dark', run: () => useUi.getState().setTheme('dark') },
  { id: 'view.themeSystem', title: 'Theme follows the system', menu: 'View', group: 'theme', checked: () => useUi.getState().theme === 'system', run: () => useUi.getState().setTheme('system') },
  { id: 'view.cycleTheme', title: 'Next theme', hidden: true, run: () => useUi.getState().setTheme(nextTheme(useUi.getState().theme)) },
  { id: 'view.densityComfortable', title: 'Comfortable rows', menu: 'View', group: 'density', keywords: ['density', 'row height', 'spacing'], checked: () => useUi.getState().density === 'comfortable', run: () => useUi.getState().setDensity('comfortable') },
  { id: 'view.densityCompact', title: 'Compact rows', menu: 'View', group: 'density', keywords: ['density', 'row height', 'dense', 'spacing'], checked: () => useUi.getState().density === 'compact', run: () => useUi.getState().setDensity('compact') },
  { id: 'view.toggleDensity', title: 'Toggle compact rows', keywords: ['density', 'row height'], run: () => useUi.getState().setDensity(useUi.getState().density === 'compact' ? 'comfortable' : 'compact') },
  { id: 'view.rail', title: 'Project rail', menu: 'View', group: 'panels', shortcut: 'Mod+B', inInputs: true, keywords: ['sidebar', 'tree', 'panel'], checked: () => useUi.getState().railVisible, run: () => useUi.getState().toggleRail() },
  { id: 'view.inspector', title: 'Inspector', menu: 'View', group: 'panels', shortcut: 'Mod+\\', inInputs: true, keywords: ['properties', 'panel', 'provenance'], checked: () => useUi.getState().inspectorVisible, run: () => useUi.getState().toggleInspector() },
  { id: 'view.focusGrid', title: 'Focus the grid', menu: 'View', group: 'focus', shortcut: 'Mod+Alt+G', inInputs: true, run: () => (document.querySelector('[data-grid]') as HTMLElement | null)?.focus() },
  { id: 'view.closeTab', title: 'Close tab', menu: 'View', group: 'focus', shortcut: 'Mod+Alt+W', enabled: () => !!useProject.getState().activeId, run: () => { const id = useProject.getState().activeId; if (id) useProject.getState().close(id); } },

  // ---- Help ----
  { id: 'help.manual', title: 'Read the manual', menu: 'Help', group: 'help', keywords: ['docs', 'documentation', 'guide', 'help'], run: app.openManual },
  { id: 'file.openSample', title: 'Open the sample project', menu: 'Help', group: 'sample', keywords: ['example', 'demo', 'cytokine', 'tutorial'], run: app.openSampleProject },
  { id: 'help.lab', title: 'Dioptra Lab...', menu: 'Help', group: 'lab', keywords: ['license', 'key', 'activate', 'buy', 'pro'], inInputs: true, run: () => useLab.getState().setDialogOpen(true) },
  { id: 'help.shortcuts', title: 'Keyboard shortcuts', menu: 'Help', group: 'help', shortcut: 'F1', inInputs: true, keywords: ['keys', 'help'], run: () => useUi.getState().setShortcutsOpen(true) },
];

commands.register(...CORE_COMMANDS);
commands.register(...RECORDS_COMMANDS);
commands.register(...OQ_COMMANDS);
commands.register(...BATCH_COMMANDS);
