// A hook on error replies, below every caller: Records for regulated labs uses it to ask for a
// sign-in or a reason when the engine refuses a change, then sends the change again.
import type { Engine } from './bridge';
import type { Command, Response } from './types';

export type ResponseGuard = (engine: Engine, command: Command, response: Extract<Response, { type: 'error' }>) => Promise<Response>;

let guard: ResponseGuard | null = null;

/** Install (or with `null` remove) the guard. */
export function setResponseGuard(g: ResponseGuard | null): void {
  guard = g;
}

/** An engine whose error replies pass through the guard. */
export function guarded(engine: Engine): Engine {
  return {
    backend: engine.backend,
    async dispatch(command: Command): Promise<Response> {
      const r = await engine.dispatch(command);
      return r.type === 'error' && guard ? guard(engine, command, r) : r;
    },
  };
}
