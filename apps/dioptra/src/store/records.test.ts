import { describe, expect, it } from 'vitest';
import { guarded, setResponseGuard } from '../engine/guard';
import type { Engine } from '../engine/bridge';
import type { Command, Response } from '../engine/types';
import { recordsWarning, type RecordsStatus } from './records';

const base: RecordsStatus = {
  regulated: true,
  intact: true,
  entries: 3,
  version: 1,
  locked: false,
  content_matches: true,
  signatures: [],
  has_users: true,
};

describe('records', () => {
  it('warns only when a regulated record cannot be trusted', () => {
    expect(recordsWarning(base)).toBeNull();
    expect(recordsWarning({ ...base, regulated: false, content_matches: false })).toBeNull();
    expect(recordsWarning({ ...base, intact: false, first_break: { line: 2, problem: 'the entry was changed after it was written' } })).toMatch(
      /broken at entry 2/,
    );
    expect(recordsWarning({ ...base, content_matches: false })).toMatch(/changed outside/);
  });

  it('passes refused changes through the guard and sends them again', async () => {
    const sent: Command[] = [];
    const inner: Engine = {
      backend: 'wasm',
      async dispatch(c: Command): Promise<Response> {
        sent.push(c);
        return c.type === ('lab_with_reason' as string) ? { type: 'ok', changed: { ids: [], tree: false } } : { type: 'error', message: 'A reason is required for every change to a regulated project.' };
      },
    };
    setResponseGuard(async (e, command) => e.dispatch({ type: 'lab_with_reason', reason: 'typo', command } as unknown as Command));
    const r = await guarded(inner).dispatch({ type: 'undo' });
    setResponseGuard(null);
    expect(r.type).toBe('ok');
    expect(sent.map((c) => c.type)).toEqual(['undo', 'lab_with_reason']);
  });
});
