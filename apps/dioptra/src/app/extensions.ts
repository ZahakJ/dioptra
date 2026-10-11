// Extension points for Phase 5B (analysis flow, graph editor, layouts). Import from here.
//
//   registerWorkArea('analysis', ResultsSheetView)       work-area renderer per object kind
//   registerInspectorSection({ id, title, order, when, component })
//   commands.register({ id: 'analyze.new', title: 'Analyze...', menu: 'Analyze', shortcut: 'Mod+Shift+A', run })
//   mutate((e) => call(e, {...}))                       run a mutation and refresh from Changed
//   useProject / useTables / useGrid                     stores for the active object and selection
export { registerWorkArea, getWorkArea, PlaceholderWorkArea, type WorkAreaProps, type WorkAreaRenderer } from './workArea';
export { registerInspectorSection, inspectorSections, type InspectorContext, type InspectorSection } from './inspector';
export { commands } from './commands';
export type { AppCommand, MenuName } from './registry';
export { openDialog, confirmDialog, promptDialog, chooseDialog, DialogHeader } from './dialogs';
export { showContextMenu, type MenuEntry } from './ContextMenu';
export { mutate, mutateReport, query, applyChanged, onChanged } from '../store/sync';
export { withEngine } from '../store/engine';
export { useProject, useActiveItem, usePValueStyle } from '../store/project';
export { useTables, useTableMeta } from '../store/table';
export { useGrid, useSelection, selectionOf } from '../grid/store';
export { notify, notifyError } from '../store/notices';
export { registerTableTool, type TableTool } from './tableTools';
export { registerRailActions } from './ProjectRail';
