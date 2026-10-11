// Store-side access to the engine singleton: every call counts towards the status line's
// "working" indicator and the first call drives the one-line loading state.
import type { Engine } from '../engine/bridge';
import { engine } from '../engine/engine';
import { ping } from '../engine/commands';
import { trackPending, useEngineStatus } from './status';

let booted: Promise<Engine> | null = null;

/** Load the engine (once) and record its backend and version. */
export function bootEngine(): Promise<Engine> {
  if (!booted) {
    booted = engine().then(
      async (e) => {
        const version = await ping(e).catch(() => null);
        useEngineStatus.setState({ ready: true, backend: e.backend, version, error: null });
        return e;
      },
      (err: unknown) => {
        booted = null;
        useEngineStatus.setState({ error: err instanceof Error ? err.message : String(err) });
        throw err;
      },
    );
  }
  return booted;
}

/** Run a function against the engine, tracked as pending work. */
export function withEngine<T>(fn: (e: Engine) => Promise<T>): Promise<T> {
  return trackPending(bootEngine().then(fn));
}
