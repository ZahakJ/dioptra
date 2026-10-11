// Running mutations: call the engine, then refresh exactly what `Changed` says changed.
import type { Engine } from '../engine/bridge';
import type { Changed, PasteReport } from '../engine/types';
import { withEngine } from './engine';
import { useHistory } from './history';
import { notifyError } from './notices';
import { useProject } from './project';
import { useTables } from './table';

type ChangedListener = (changed: Changed) => void;
const changedListeners = new Set<ChangedListener>();

/**
 * Called after every applied `Changed` (mutations, undo/redo, recompute). Views of analyses,
 * graphs and layouts use it to refetch when their id is among `changed.ids`.
 */
export function onChanged(fn: ChangedListener): () => void {
  changedListeners.add(fn);
  return () => changedListeners.delete(fn);
}

/** Refresh the project tree, the changed tables and (if shown) the history. */
export async function applyChanged(changed: Changed, opts: { open?: boolean } = {}): Promise<void> {
  await useProject.getState().refresh();
  const project = useProject.getState().view;
  const live = new Set(project?.items.map((i) => i.id));
  const tables = useTables.getState();
  for (const id of changed.ids) if (!live.has(id) && tables.metas[id]) tables.forget(id);
  await tables.invalidate(changed.ids.filter((id) => live.has(id)));
  if (changed.created && opts.open !== false && live.has(changed.created)) {
    useProject.getState().open(changed.created);
  }
  if (useHistory.getState().watched > 0) void useHistory.getState().refresh();
  for (const l of [...changedListeners]) {
    try {
      l(changed);
    } catch {
      // A listener's failure must not break the mutation that triggered it.
    }
  }
}

/**
 * Run a mutation. Errors become an error notice and resolve to `undefined`, so UI handlers can
 * fire and forget.
 */
export async function mutate(
  fn: (e: Engine) => Promise<Changed>,
  opts: { open?: boolean; quiet?: boolean } = {},
): Promise<Changed | undefined> {
  try {
    const changed = await withEngine(fn);
    await applyChanged(changed, opts);
    return changed;
  } catch (err) {
    if (!opts.quiet) notifyError(err);
    return undefined;
  }
}

/** Run a paste or import: like `mutate`, returning the report. */
export async function mutateReport(
  fn: (e: Engine) => Promise<PasteReport>,
  opts: { open?: boolean } = {},
): Promise<PasteReport | undefined> {
  try {
    const report = await withEngine(fn);
    await applyChanged(report.changed, opts);
    return report;
  } catch (err) {
    notifyError(err);
    return undefined;
  }
}

/** Run a read-only engine call, reporting errors. */
export async function query<T>(fn: (e: Engine) => Promise<T>): Promise<T | undefined> {
  try {
    return await withEngine(fn);
  } catch (err) {
    notifyError(err);
    return undefined;
  }
}
