// Files across the two backends. Desktop: native dialogs and the engine's `*Path` commands.
// Browser: <input type=file> to read bytes, a download link to write them.
import { engineIfReady } from '../engine/engine';

export function isDesktop(): boolean {
  return engineIfReady()?.backend === 'tauri';
}

export interface DialogFilter {
  name: string;
  extensions: string[];
}

export const PROJECT_FILTER: DialogFilter = { name: 'Dioptra project', extensions: ['dioptra'] };
export const IMPORT_FILTER: DialogFilter = {
  name: 'Data files',
  extensions: ['csv', 'tsv', 'txt', 'xlsx', 'xls', 'ods', 'pzfx', 'prism'],
};

/** Desktop: ask for a file to open. */
export async function desktopOpenPath(filters: DialogFilter[]): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ multiple: false, directory: false, filters });
  return typeof r === 'string' ? r : null;
}

/** Desktop: ask where to save. */
export async function desktopSavePath(defaultPath: string, filters: DialogFilter[]): Promise<string | null> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const r = await save({ defaultPath, filters });
  return r ?? null;
}

/** Browser: let the user pick a file. Resolves `null` if they cancel. */
export function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.style.display = 'none';
    input.setAttribute('data-testid', 'file-input');
    let settled = false;
    const finish = (f: File | null) => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(f);
    };
    input.addEventListener('change', () => finish(input.files?.[0] ?? null));
    input.addEventListener('cancel', () => finish(null));
    document.body.appendChild(input);
    input.click();
  });
}

export async function fileToBase64(file: Blob): Promise<string> {
  return bytesToBase64(new Uint8Array(await file.arrayBuffer()));
}

export function bytesToBase64(bytes: Uint8Array): string {
  let s = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    s += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(s);
}

export function base64ToBytes(b64: string): Uint8Array<ArrayBuffer> {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Browser: hand the user a file to save. */
export function downloadBytes(name: string, mime: string, base64: string): void {
  const blob = new Blob([base64ToBytes(base64)], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.style.display = 'none';
  document.body.appendChild(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(url);
    a.remove();
  }, 1000);
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
