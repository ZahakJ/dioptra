// Dioptra Lab journal figures: presets, the figure check, TIFF export, the methods paragraph
// and the project report. Each helper sends one `lab_*` command; the engine checks the key.
import { call, type Engine } from '../engine/bridge';
import type { Command, Id, SizeMm } from '../engine/types';
import { chooseDialog, confirmDialog } from '../app/dialogs';
import { base64ToBytes, desktopSavePath, downloadBytes, isDesktop, type DialogFilter } from '../io/files';
import { useLab } from '../store/lab';
import { withEngine } from '../store/engine';
import { notify, notifyError } from '../store/notices';
import { useProject } from '../store/project';

export type ColumnWidth = 'single' | 'onehalf' | 'double';
export type LetterCase = 'upper' | 'lower' | 'none';

export interface JournalPreset {
  id: string;
  name: string;
  single_mm: number;
  onehalf_mm: number | null;
  double_mm: number;
  max_height_mm: number | null;
  font: string;
  min_font_pt: number;
  max_font_pt: number | null;
  min_line_pt: number;
  dpi_line_art: number;
  dpi_halftone: number;
  dpi_combination: number;
  file_types: string[];
  file_note: string;
  letters: LetterCase;
  letter_pt: number;
}

export type Severity = 'error' | 'warning' | 'info';

export interface FigureIssue {
  rule: 'font_size' | 'line_width' | 'resolution' | 'colour_vision' | 'panel_consistency' | 'size';
  severity: Severity;
  message: string;
  element?: string;
  target?: string;
}

export interface FigureCheck {
  issues: FigureIssue[];
  width_mm: number;
  height_mm: number;
  scale: number;
}

export interface LayoutStyle {
  width_mm?: number | null;
  font_pt?: number | null;
  line_pt?: number | null;
  letters?: LetterCase;
  letter_pt?: number | null;
}

export interface LabFile {
  name: string;
  mime: string;
  base64: string;
  notes: string[];
}

/** Send a Lab command and require a reply type. */
export async function lab<T>(e: Engine, command: Record<string, unknown>, type: string): Promise<T> {
  const r = (await call(e, command as unknown as Command)) as unknown as { type: string } & T;
  if (r.type !== type) throw new Error(`expected a ${type} reply, got ${r.type}`);
  return r;
}

export const journalPresets = (e: Engine) =>
  lab<{ presets: JournalPreset[] }>(e, { type: 'lab_journal_presets' }, 'lab_journal_presets').then((r) => r.presets);

export const applyJournalPreset = (e: Engine, preset: string, width: ColumnWidth, targets: 'all' | Id[]) =>
  lab<{ ids: Id[] }>(e, { type: 'lab_apply_journal_preset', preset, width, targets }, 'lab_changed').then((r) => r.ids);

export const checkFigure = (e: Engine, id: Id, preset: string | null, dpi: number | null = null) =>
  lab<FigureCheck>(
    e,
    { type: 'lab_check_figure', id, preset, raster: dpi === null ? null : { dpi } },
    'lab_figure_check',
  );

export const exportTiff = (e: Engine, id: Id, dpi: number, size: SizeMm | null) =>
  lab<LabFile>(e, { type: 'lab_export_tiff', id, dpi, size }, 'lab_file');

export const methodsParagraph = (e: Engine, scope: 'project' | Id[]) =>
  lab<{ text: string; references: string[] }>(e, { type: 'lab_methods', scope }, 'lab_methods');

export const projectReport = (e: Engine, format: 'docx' | 'pdf') =>
  lab<LabFile>(e, { type: 'lab_report', format }, 'lab_file');

let presetCache: Promise<JournalPreset[]> | null = null;
/** The presets (no key needed), fetched once. */
export function loadPresets(): Promise<JournalPreset[]> {
  presetCache ??= withEngine(journalPresets).catch((err: unknown) => {
    presetCache = null;
    throw err;
  });
  return presetCache;
}

/** Width of a preset at a column choice, or null when the journal has none. */
export function widthOf(p: JournalPreset, w: ColumnWidth): number | null {
  return w === 'single' ? p.single_mm : w === 'double' ? p.double_mm : p.onehalf_mm;
}

/** Explain a Lab feature when Lab is off, offering Help → Dioptra Lab. Returns whether Lab is on. */
export async function requireLab(feature: string, what: string): Promise<boolean> {
  if (useLab.getState().view.lab_enabled) return true;
  const go = await confirmDialog({ title: feature, message: `${what} Part of Dioptra Lab.`, confirm: 'Dioptra Lab…' });
  if (go) useLab.getState().setDialogOpen(true);
  return false;
}

const FILTERS: Record<string, DialogFilter> = {
  tiff: { name: 'TIFF image', extensions: ['tiff', 'tif'] },
  docx: { name: 'Word document', extensions: ['docx'] },
  pdf: { name: 'PDF', extensions: ['pdf'] },
  pzfx: { name: 'Prism file', extensions: ['pzfx'] },
};

/** Save a file the engine wrote: a save dialog on desktop, a download in the browser. */
export async function saveLabFile(f: LabFile, name = f.name): Promise<boolean> {
  if (isDesktop()) {
    const ext = name.split('.').pop() ?? '';
    const path = await desktopSavePath(name, FILTERS[ext] ? [FILTERS[ext]] : []);
    if (!path) return false;
    const { writeFile } = await import('@tauri-apps/plugin-fs');
    await writeFile(path, base64ToBytes(f.base64));
    notify(`Saved ${path.split(/[\\/]/).pop()}.`);
  } else {
    downloadBytes(name, f.mime, f.base64);
  }
  return true;
}

export const METHODS_WHAT =
  'A methods paragraph naming each test, its options, the correction for multiple comparisons, the software version and the references to cite.';
export const REPORT_WHAT = 'A Word or PDF report of the data tables, results sheets, methods and figures.';
export const JOURNAL_WHAT =
  'Size figures for a journal in one step (column width, type, line widths, panel letters) and check them for small text, thin lines, low resolution, colours colour-blind readers cannot tell apart and panels that differ.';
export const TIFF_WHAT = 'TIFF with LZW compression, the raster format most journals ask for.';

/** Copy the methods paragraph for the project or some analyses. */
export async function copyMethods(scope: 'project' | Id[]): Promise<void> {
  if (!(await requireLab('Methods paragraph', METHODS_WHAT))) return;
  try {
    const m = await withEngine((e) => methodsParagraph(e, scope));
    await navigator.clipboard.writeText(m.text);
    notify('Copied the methods paragraph.');
  } catch (err) {
    notifyError(err);
  }
}

/** File → Export report… */
export async function exportReport(): Promise<void> {
  if (!(await requireLab('Export report', REPORT_WHAT))) return;
  const format = await chooseDialog<'docx' | 'pdf'>({
    title: 'Export report',
    label: 'Format',
    options: [
      { value: 'docx', label: 'Word (.docx)', hint: 'Editable; figures at 300 dpi.' },
      { value: 'pdf', label: 'PDF', hint: 'A4 pages; figures as vectors.' },
    ],
    initial: 'docx',
    confirm: isDesktop() ? 'Choose location…' : 'Download',
  });
  if (!format) return;
  try {
    const f = await withEngine((e) => projectReport(e, format));
    await saveLabFile(f);
  } catch (err) {
    notifyError(err);
  }
}

/** Whether the project has anything to report. */
export const hasProjectContent = () => (useProject.getState().view?.items.length ?? 0) > 0;
