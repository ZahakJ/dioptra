// The one engine singleton. Every engine call in the UI goes through `engine()`.
import { createEngine, serialized, type Engine } from './bridge';
import { guarded } from './guard';

let instance: Promise<Engine> | null = null;
let ready: Engine | null = null;
const listeners = new Set<(engine: Engine) => void>();

/** The shared engine, created on first use. */
export function engine(): Promise<Engine> {
  if (!instance) {
    instance = createEngine().then((e) => {
      ready = guarded(serialized(e));
      for (const l of listeners) l(ready);
      return ready;
    });
    instance.catch(() => {
      instance = null;
    });
  }
  return instance;
}

/** The engine if it has finished loading. */
export function engineIfReady(): Engine | null {
  return ready;
}

/** Called once when the engine has loaded. */
export function onEngineReady(fn: (engine: Engine) => void): () => void {
  if (ready) fn(ready);
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Replace the engine (tests). */
export function setEngineForTests(e: Engine | null): void {
  ready = e;
  instance = e ? Promise.resolve(e) : null;
}
