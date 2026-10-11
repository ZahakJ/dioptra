import type { Command, Response } from './types';

/** One engine session: the Tauri process on desktop, a wasm instance in the browser. */
export interface Engine {
  readonly backend: 'tauri' | 'wasm';
  dispatch(command: Command): Promise<Response>;
}

export class EngineError extends Error {
  constructor(message: string, public readonly command: Command) {
    super(message);
    this.name = 'EngineError';
  }
}

/** Dispatch and throw on `{type: "error"}` so callers can await typed results. */
export async function call<T extends Response = Response>(engine: Engine, command: Command): Promise<T> {
  const response = await engine.dispatch(command);
  if (response.type === 'error') {
    throw new EngineError(String(response.message ?? 'engine error'), command);
  }
  return response as T;
}

function hasTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Pick the backend: Tauri IPC inside the desktop app, otherwise the wasm build. */
export async function createEngine(): Promise<Engine> {
  if (hasTauri()) {
    const { createTauriEngine } = await import('./tauri');
    return createTauriEngine();
  }
  const { createWasmEngine } = await import('./wasm');
  return createWasmEngine();
}
