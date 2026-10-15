// Import file… with the plate-map step for plate-reader exports, and Replace table data…
// (re-running a project on new data by hand). Both are free.
import { openDialog } from '../app/dialogs';
import * as cmd from '../engine/commands';
import type { PasteReport, TableKind } from '../engine/types';
import { activeMeta } from '../grid/actions';
import { withEngine } from '../store/engine';
import { notify, notifyError, useNotices } from '../store/notices';
import { mutateReport } from '../store/sync';
import { desktopOpenPath, fileToBase64, IMPORT_FILTER, isDesktop, pickFile } from './files';
import { PlateImportDialog, type PlateImportSpec } from './PlateImportDialog';
import { formatPasteReport } from './report';

const ACCEPT = '.csv,.tsv,.txt,.xlsx,.xls,.ods,.pzfx,.prism';

/** A chosen file: its name, and its path (desktop) or bytes (browser). */
interface Chosen {
  name: string;
  path?: string;
  base64?: string;
}

async function chooseFile(): Promise<Chosen | null> {
  if (isDesktop()) {
    const path = await desktopOpenPath([IMPORT_FILTER]).catch((e: unknown) => {
      notifyError(e);
      return null;
    });
    return path ? { name: path, path } : null;
  }
  const file = await pickFile(ACCEPT);
  return file ? { name: file.name, base64: await fileToBase64(file) } : null;
}

/** The plate map for a plate-reader export (`''` for none), `null` if cancelled. */
async function plateStep(f: Chosen): Promise<string | null> {
  const plate = await withEngine((e) => cmd.inspectPlate(e, f)).catch(() => null);
  if (!plate) return '';
  const short = f.name.split(/[\\/]/).pop() ?? f.name;
  const spec = await openDialog<PlateImportSpec | null>((close) => <PlateImportDialog name={short} plate={plate} close={close} />, null);
  return spec ? spec.plateMap : null;
}

function report(r: PasteReport, verb: string): void {
  useNotices.getState().push({ kind: 'report', text: formatPasteReport(r, verb), detail: r.notes });
}

/** Pick a file and import it as a new table; plate-reader exports get a plate-map step. */
export async function importChosenFile(kind: TableKind | null): Promise<void> {
  const f = await chooseFile();
  if (!f) return;
  const map = await plateStep(f);
  if (map === null) return;
  const r = await mutateReport((e) => (f.path ? cmd.importPath(e, f.path, kind, map) : cmd.importBytes(e, f.name, f.base64!, kind, map)));
  if (r) report(r, 'Imported');
}

/** Put a file's data into the open table, keeping its formulas, analyses and graphs. */
export async function replaceTableData(): Promise<void> {
  const meta = activeMeta();
  if (!meta) {
    notify('Open a table to replace its data.');
    return;
  }
  const f = await chooseFile();
  if (!f) return;
  const map = await plateStep(f);
  if (map === null) return;
  try {
    const base64 = f.base64 ?? (await readPath(f.path!));
    const r = await mutateReport((e) => cmd.replaceTableData(e, meta.id, f.name.split(/[\\/]/).pop() ?? f.name, base64, map));
    if (r) report(r, `Replaced the data of "${meta.title}":`);
  } catch (err) {
    notifyError(err);
  }
}

async function readPath(path: string): Promise<string> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<string>('batch_read', { path });
}
