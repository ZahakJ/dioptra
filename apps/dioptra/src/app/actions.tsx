// Project-level actions: new/open/save, import/export, objects. Used by the command registry.
import * as cmd from '../engine/commands';
import type { Id, TableFormat } from '../engine/types';
import { activeMeta } from '../grid/actions';
import { ExportDialog } from '../io/ExportDialog';
import {
  baseName,
  desktopOpenPath,
  desktopSavePath,
  downloadBytes,
  fileToBase64,
  IMPORT_FILTER,
  isDesktop,
  pickFile,
  PROJECT_FILTER,
} from '../io/files';
import { FileImportDialog, PasteImportDialog, type FileImportSpec, type PasteImportSpec } from '../io/ImportDialog';
import { formatPasteReport } from '../io/report';
import { withEngine } from '../store/engine';
import { notify, notifyError, useNotices } from '../store/notices';
import { itemById, useProject } from '../store/project';
import { applyChanged, mutate, mutateReport } from '../store/sync';
import { useTables } from '../store/table';
import { confirmDialog, openDialog, promptDialog } from './dialogs';
import { NewTableDialog, type NewTableSpec } from './NewTableDialog';

/** Ask before throwing away unsaved work. */
export async function guardUnsaved(action: string): Promise<boolean> {
  const view = useProject.getState().view;
  if (!view?.dirty || view.items.length === 0) return true;
  return confirmDialog({
    title: 'Unsaved changes',
    message: `"${view.title}" has changes that are not saved. ${action} anyway and lose them?`,
    confirm: `Discard and ${action.toLowerCase()}`,
    danger: true,
  });
}

function forgetAllTables(): void {
  const t = useTables.getState();
  for (const id of Object.keys(t.metas)) t.forget(id);
}

async function afterLoad(): Promise<void> {
  forgetAllTables();
  useProject.getState().reset();
  await applyChanged({ ids: [], tree: true });
  const first = useProject.getState().view?.items[0];
  if (first) useProject.getState().open(first.id);
}

export async function newProject(): Promise<void> {
  if (!(await guardUnsaved('New project'))) return;
  try {
    await withEngine(cmd.newProject);
    await afterLoad();
  } catch (err) {
    notifyError(err);
  }
}

export async function openProject(): Promise<void> {
  if (!(await guardUnsaved('Open'))) return;
  try {
    if (isDesktop()) {
      const path = await desktopOpenPath([PROJECT_FILTER]);
      if (!path) return;
      await withEngine((e) => cmd.openProjectPath(e, path));
    } else {
      const file = await pickFile('.dioptra,application/zip');
      if (!file) return;
      const b64 = await fileToBase64(file);
      await withEngine((e) => cmd.openProjectBytes(e, b64, file.name));
    }
    await afterLoad();
    notify(`Opened "${useProject.getState().view?.title ?? 'project'}".`);
  } catch (err) {
    notifyError(err);
  }
}

export const MANUAL_URL = 'https://zahakj.github.io/dioptra/manual/';
export const SAMPLE_PATH = 'samples/cytokine-study.dioptra';

/** The bundled sample project (public/samples), opened from bytes on both backends. */
export async function openSampleProject(): Promise<void> {
  if (!(await guardUnsaved('Open the sample'))) return;
  try {
    const base = import.meta.env.BASE_URL ?? '/';
    const res = await fetch(`${base}${SAMPLE_PATH}`);
    if (!res.ok) throw new Error(`The sample project could not be read (${res.status}).`);
    const b64 = await fileToBase64(await res.blob());
    await withEngine((e) => cmd.openProjectBytes(e, b64, 'Cytokine study.dioptra'));
    await afterLoad();
    notify('Opened the sample project. Save it to keep your changes.');
  } catch (err) {
    notifyError(err);
  }
}

/** The manual in the system browser (desktop) or a new tab. */
export async function openManual(): Promise<void> {
  if (isDesktop()) {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl(MANUAL_URL);
      return;
    } catch (err) {
      notifyError(err);
      return;
    }
  }
  window.open(MANUAL_URL, '_blank', 'noopener,noreferrer');
}

function fileStem(title: string): string {
  return (title.trim() || 'Untitled').replace(/[\\/:*?"<>|]+/g, '-');
}

export async function saveProject(): Promise<void> {
  const view = useProject.getState().view;
  if (isDesktop() && view?.path) {
    const path = view.path;
    if (await mutate((e) => cmd.saveProjectPath(e, path))) notify(`Saved to ${baseName(path)}.`);
    return;
  }
  await saveProjectAs();
}

export async function saveProjectAs(): Promise<void> {
  const view = useProject.getState().view;
  try {
    if (isDesktop()) {
      let path = await desktopSavePath(`${fileStem(view?.title ?? '')}.dioptra`, [PROJECT_FILTER]);
      if (!path) return;
      if (!path.toLowerCase().endsWith('.dioptra')) path += '.dioptra';
      const p = path;
      if (await mutate((e) => cmd.saveProjectPath(e, p))) notify(`Saved to ${baseName(p)}.`);
    } else {
      const bytes = await withEngine(cmd.saveProjectBytes);
      downloadBytes(bytes.name, bytes.mime, bytes.base64);
      await applyChanged({ ids: [], tree: true });
      notify(`Saved ${bytes.name} to your downloads.`);
    }
  } catch (err) {
    notifyError(err);
  }
}

export async function newTable(): Promise<void> {
  const spec = await openDialog<NewTableSpec | null>((close) => <NewTableDialog close={close} />, null);
  if (!spec) return;
  await mutate((e) => cmd.createTable(e, spec));
}

function reportImport(r: { rows: number; cols: number } & Parameters<typeof formatPasteReport>[0]): void {
  useNotices.getState().push({ kind: 'report', text: formatPasteReport(r, 'Imported'), detail: r.notes });
}

export async function importFile(): Promise<void> {
  const spec = await openDialog<FileImportSpec | null>((close) => <FileImportDialog close={close} />, null);
  if (!spec) return;
  if (isDesktop()) {
    const path = await desktopOpenPath([IMPORT_FILTER]).catch((e: unknown) => {
      notifyError(e);
      return null;
    });
    if (!path) return;
    const r = await mutateReport((e) => cmd.importPath(e, path, spec.kind));
    if (r) reportImport(r);
  } else {
    const file = await pickFile('.csv,.tsv,.txt,.xlsx,.xls,.ods,.pzfx');
    if (!file) return;
    const b64 = await fileToBase64(file);
    const r = await mutateReport((e) => cmd.importBytes(e, file.name, b64, spec.kind));
    if (r) reportImport(r);
  }
}

export async function pasteAsNewTable(initial?: string): Promise<void> {
  let text = initial ?? '';
  if (initial === undefined) {
    try {
      text = await navigator.clipboard.readText();
    } catch {
      text = '';
    }
  }
  const spec = await openDialog<PasteImportSpec | null>((close) => <PasteImportDialog initialText={text} close={close} />, null);
  if (!spec) return;
  const r = await mutateReport((e) => cmd.importText(e, spec.text, spec.kind, spec.title, spec.options));
  if (r) reportImport(r);
}

export async function exportTable(): Promise<void> {
  const meta = activeMeta();
  if (!meta) {
    notify('Open a table to export it.');
    return;
  }
  const desktop = isDesktop();
  const format = await openDialog<TableFormat | null>(
    (close) => <ExportDialog tableTitle={meta.title} desktop={desktop} close={close} />,
    null,
  );
  if (!format) return;
  try {
    if (desktop) {
      const ext = format;
      let path = await desktopSavePath(`${fileStem(meta.title)}.${ext}`, [{ name: ext.toUpperCase(), extensions: [ext] }]);
      if (!path) return;
      if (!path.toLowerCase().endsWith(`.${ext}`)) path += `.${ext}`;
      const p = path;
      await withEngine((e) => cmd.exportTablePath(e, meta.id, format, p));
      notify(`Exported to ${baseName(p)}.`);
    } else {
      const b = await withEngine((e) => cmd.exportTable(e, meta.id, format));
      downloadBytes(b.name, b.mime, b.base64);
      notify(`Exported ${b.name}.`);
    }
  } catch (err) {
    notifyError(err);
  }
}

export async function renameObject(id: Id | null = useProject.getState().activeId): Promise<void> {
  const item = itemById(id);
  if (!item) return;
  const title = await promptDialog({
    title: `Rename ${item.kind}`,
    label: 'Title',
    initial: item.title,
    confirm: 'Rename',
    validate: (v) => (v.trim() ? null : 'A title cannot be empty.'),
  });
  if (title === null || title.trim() === item.title) return;
  await mutate((e) => cmd.renameObject(e, item.id, title.trim()));
}

export async function renameProject(): Promise<void> {
  const current = useProject.getState().view?.title ?? '';
  const title = await promptDialog({
    title: 'Rename project',
    label: 'Title',
    initial: current,
    confirm: 'Rename',
    validate: (v) => (v.trim() ? null : 'A title cannot be empty.'),
  });
  if (title === null || title.trim() === current) return;
  await mutate((e) => cmd.setProjectTitle(e, title.trim()));
}

export async function deleteObject(id: Id | null = useProject.getState().activeId): Promise<void> {
  const item = itemById(id);
  if (!item) return;
  const view = useProject.getState().view;
  const dependents = view?.items.filter((i) => i.derived_from === item.id) ?? [];
  const ok = await confirmDialog({
    title: `Delete ${item.kind}`,
    message:
      `Delete "${item.title}"` +
      (dependents.length ? ` and ${dependents.length} object${dependents.length === 1 ? '' : 's'} made from it` : '') +
      '? Undo brings it back.',
    confirm: 'Delete',
    danger: true,
  });
  if (!ok) return;
  await mutate((e) => cmd.deleteObject(e, item.id));
}

export async function recomputeAll(): Promise<void> {
  if (await mutate(cmd.recomputeAll)) notify('Everything is up to date.');
}
