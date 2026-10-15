// Dioptra Lab batch analysis: the engine's `lab_batch_*` commands in the browser, the desktop
// shell's `batch_*` and `watch_*` commands (which read and write files themselves).
import { EngineError, type Engine } from '../engine/bridge';
import type { Command } from '../engine/types';
import { withEngine } from '../store/engine';
import { BATCH_EXTENSIONS, summarise, summaryCsv, type BatchItem, type OutputFile } from './batchSummary';
import { bytesToBase64 } from './files';

export type FigureFormat = 'pdf' | 'png' | 'svg';

export interface BatchOptions {
  format: FigureFormat;
  dpi?: number;
  plate_map?: string;
}

export interface TemplateInfo {
  title: string;
  tables: string[];
  analyses: number;
  figures: number;
}

/** A template: the project bytes and where they came from. */
export interface Template {
  label: string;
  base64: string;
}

async function lab<T>(e: Engine, command: Record<string, unknown>): Promise<T> {
  const r = (await e.dispatch(command as unknown as Command)) as { type: string; message?: string };
  if (r.type === 'error') throw new EngineError(r.message ?? 'engine error', command as unknown as Command);
  return r as T;
}

async function invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

/** The open project as a template, without saving it. */
export async function currentProjectTemplate(): Promise<Template> {
  const r = await withEngine((e) => lab<{ name: string; base64: string }>(e, { type: 'lab_template' }));
  return { label: 'The open project', base64: r.base64 };
}

/** Desktop: a template file's bytes. */
export function readTemplatePath(path: string): Promise<string> {
  return invoke<string>('batch_read', { path });
}

export function templateInfo(base64: string): Promise<TemplateInfo> {
  return withEngine((e) => lab<TemplateInfo & { type: string }>(e, { type: 'lab_template_info', template_base64: base64 }));
}

/** Browser: run one file through the template in the engine. */
export async function runInEngine(template: Template, table: string, file: File, options: BatchOptions): Promise<BatchItem> {
  const base64 = bytesToBase64(new Uint8Array(await file.arrayBuffer()));
  const r = await withEngine((e) =>
    lab<{ items: BatchItem[] }>(e, {
      type: 'lab_batch_run',
      template_base64: template.base64,
      table,
      inputs: [{ name: file.name, base64 }],
      options,
    }),
  );
  return r.items[0]!;
}

/** Browser: every output and `summary.csv` in one zip. */
export async function zipResults(items: BatchItem[], name = 'Batch results.zip'): Promise<{ name: string; mime: string; base64: string }> {
  const files: OutputFile[] = items.flatMap((i) => i.files);
  files.push({ name: 'summary.csv', mime: 'text/csv', base64: bytesToBase64(new TextEncoder().encode(summaryCsv(summarise(items)))) });
  return withEngine((e) => lab<{ name: string; mime: string; base64: string }>(e, { type: 'lab_zip', name, files }));
}

/** Desktop: run one file and write its outputs into `outDir`. */
export function runOnDisk(template: Template, table: string, input: string, outDir: string, options: BatchOptions): Promise<BatchItem> {
  return invoke<BatchItem>('batch_file', { templateBase64: template.base64, table, input, outDir, options });
}

/** Desktop: write `summary.csv`; returns its path. */
export function writeSummary(outDir: string, items: BatchItem[]): Promise<string> {
  return invoke<string>('batch_summary', { outDir, items });
}

/** Desktop: the data files in a folder. */
export function listFolder(dir: string): Promise<string[]> {
  return invoke<string[]>('batch_list', { dir });
}

/** Desktop: pick a folder. */
export async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === 'string' ? r : null;
}

/** Desktop: pick data files. */
export async function pickDataPaths(): Promise<string[]> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ multiple: true, directory: false, filters: [{ name: 'Data files', extensions: BATCH_EXTENSIONS }] });
  return Array.isArray(r) ? r : typeof r === 'string' ? [r] : [];
}

/** Browser: pick several files. */
export function pickFiles(accept: string): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.accept = accept;
    input.style.display = 'none';
    input.setAttribute('data-testid', 'batch-file-input');
    let settled = false;
    const finish = (f: File[]) => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(f);
    };
    input.addEventListener('change', () => finish([...(input.files ?? [])]));
    input.addEventListener('cancel', () => finish([]));
    document.body.appendChild(input);
    input.click();
  });
}

export interface WatchEvent {
  dir: string;
  item?: BatchItem;
  error?: string;
  stopped: boolean;
}

/** Desktop: watch a folder; `onEvent` hears every analysed file. Returns the unlisten function. */
export async function startWatch(
  dir: string,
  template: Template,
  table: string,
  options: BatchOptions,
  onEvent: (e: WatchEvent) => void,
): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<WatchEvent>('dioptra://watch', (ev) => onEvent(ev.payload));
  try {
    await invoke('watch_start', { dir, templateBase64: template.base64, table, options });
  } catch (err) {
    unlisten();
    throw err;
  }
  return unlisten;
}

export function stopWatch(): Promise<void> {
  return invoke<void>('watch_stop', {});
}
