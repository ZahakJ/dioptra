import type { Command, Response, ResponseOf, ResponseType } from './types';

/** One engine session: the Tauri process on desktop, a wasm instance in the browser. */
export interface Engine {
  readonly backend: 'tauri' | 'wasm';
  dispatch(command: Command): Promise<Response>;
}

export class EngineError extends Error {
  constructor(
    message: string,
    public readonly command: Command,
  ) {
    super(message);
    this.name = 'EngineError';
  }
}

/** Dispatch and throw on `{type: "error"}` so callers can await typed results. */
export async function call(engine: Engine, command: Command): Promise<Response> {
  const response = await engine.dispatch(command);
  if (response.type === 'error') {
    throw new EngineError(response.message || 'engine error', command);
  }
  return response;
}

/** Dispatch and require a particular response type. */
export async function expectType<T extends ResponseType>(
  engine: Engine,
  command: Command,
  type: T,
): Promise<ResponseOf<T>> {
  const response = await call(engine, command);
  if (response.type !== type) {
    throw new EngineError(`expected a ${type} response, got ${response.type}`, command);
  }
  return response as ResponseOf<T>;
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

/**
 * Serialise every dispatch through one queue so commands reach the engine in the order the
 * UI issued them (Tauri IPC calls are otherwise free to interleave).
 */
export function serialized(engine: Engine): Engine {
  let tail: Promise<unknown> = Promise.resolve();
  return {
    backend: engine.backend,
    dispatch(command: Command): Promise<Response> {
      const next = tail.then(() => engine.dispatch(command));
      tail = next.catch(() => undefined);
      return next;
    },
  };
}
