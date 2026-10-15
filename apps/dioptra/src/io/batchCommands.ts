// File-menu commands for batch work: Batch analysis… and Watch a folder… (Dioptra Lab), and
// Replace table data… (free).
import type { AppCommand } from '../app/registry';
import { hasActiveTable, isEditableTable } from '../grid/actions';
import { useWatch } from '../store/watch';
import { openBatchDialog } from './BatchDialog';
import { replaceTableData } from './fileImport';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const BATCH_COMMANDS: AppCommand[] = [
  { id: 'table.replaceData', title: 'Replace table data...', menu: 'File', group: 'io', keywords: ['re-run', 'new data', 'swap', 'template', 'reimport'], enabled: () => hasActiveTable() && isEditableTable(), run: replaceTableData },
  { id: 'file.batch', title: 'Batch analysis...', menu: 'File', group: 'batch', keywords: ['lab', 'template', 'many files', 'plates', 'folder'], run: () => openBatchDialog('batch') },
  { id: 'file.watch', title: 'Watch a folder...', menu: 'File', group: 'batch', keywords: ['lab', 'instrument', 'plate reader', 'automatic'], hidden: !inTauri, run: () => openBatchDialog('watch') },
  { id: 'file.stopWatch', title: 'Stop watching the folder', menu: 'File', group: 'batch', hidden: !inTauri, enabled: () => !!useWatch.getState().dir, run: () => useWatch.getState().stop() },
];
