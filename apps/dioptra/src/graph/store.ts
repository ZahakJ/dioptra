// Graph editor state per graph: the spec, the last rendered SVG with its hit-test targets,
// the selected target and the zoom. Edits go to the engine as merge patches; the engine's
// `Changed` then triggers one refetch and re-render. The previous SVG stays on screen until
// the new one arrives.
import { create } from 'zustand';
import * as obj from '../engine/objects';
import type { GraphSpecT, Rendered } from '../engine/objects';
import type { Id, JsonObject, JsonValue } from '../engine/types';
import { withEngine } from '../store/engine';
import { notifyError } from '../store/notices';
import { useProject } from '../store/project';
import { applyChanged, onChanged } from '../store/sync';
import { applyMergePatch, combinePatches, patchAt } from './patch';

export type Zoom = number | 'fit';

export interface GraphState {
  spec: GraphSpecT | null;
  rendered: Rendered | null;
  error: string | null;
  /** Selected target id (`axis-y-title`), if any. */
  selected: string | null;
  zoom: Zoom;
  /** Bumped on every successful render. */
  version: number;
}

const EMPTY: GraphState = { spec: null, rendered: null, error: null, selected: null, zoom: 'fit', version: 0 };

interface Store {
  graphs: Record<Id, GraphState>;
  set(id: Id, s: Partial<GraphState>): void;
}

export const useGraphs = create<Store>((set) => ({
  graphs: {},
  set: (id, s) => set((st) => ({ graphs: { ...st.graphs, [id]: { ...(st.graphs[id] ?? EMPTY), ...s } } })),
}));

export function useGraph(id: Id | null | undefined): GraphState {
  return useGraphs((s) => (id ? (s.graphs[id] ?? EMPTY) : EMPTY));
}

export function graphState(id: Id): GraphState {
  return useGraphs.getState().graphs[id] ?? EMPTY;
}

const seq: Record<Id, number> = {};

/** Fetch the spec and render. Results of an older request never replace a newer one. */
export async function loadGraph(id: Id): Promise<void> {
  const n = (seq[id] = (seq[id] ?? 0) + 1);
  try {
    const [spec, rendered] = await withEngine(async (e) => [await obj.getGraphSpec(e, id), await obj.renderGraph(e, id)] as const);
    if (n !== seq[id]) return;
    const prev = graphState(id);
    const known = prev.selected && rendered.targets.some((t) => t.id === prev.selected) ? prev.selected : prev.selected?.startsWith('annotation-') ? null : prev.selected;
    useGraphs.getState().set(id, { spec, rendered, error: null, selected: known, version: prev.version + 1 });
    if (useProject.getState().view?.items.find((i) => i.id === id)?.stale) void useProject.getState().refresh();
  } catch (err) {
    if (n !== seq[id]) return;
    useGraphs.getState().set(id, { error: err instanceof Error ? err.message : String(err) });
  }
}

let listening = false;
/** Refetch loaded graphs when a change touches them (edits, undo, data changes). */
export function listenForGraphChanges(): void {
  if (listening) return;
  listening = true;
  onChanged((c) => {
    const loaded = useGraphs.getState().graphs;
    for (const id of c.ids) if (loaded[id]) void loadGraph(id);
  });
}

/** Send a merge patch now. The local spec updates at once so controls do not flicker. */
export async function patchGraph(id: Id, patch: JsonObject): Promise<boolean> {
  const st = graphState(id);
  if (st.spec) useGraphs.getState().set(id, { spec: applyMergePatch(st.spec as unknown as JsonValue, patch) as unknown as GraphSpecT });
  try {
    const changed = await withEngine((e) => obj.patchGraph(e, id, patch));
    await applyChanged(changed);
    return true;
  } catch (err) {
    notifyError(err);
    void loadGraph(id);
    return false;
  }
}

/** Set one value by path (JSON pointer or dot path). */
export function setGraphValue(id: Id, path: string, value: JsonValue | undefined): Promise<boolean> {
  return patchGraph(id, patchAt(path, value));
}

const pending: Record<Id, { patch: JsonObject; timer: ReturnType<typeof setTimeout> }> = {};

/** Text edits: patch the local spec now, send to the engine after 150 ms of quiet. */
export function setGraphValueDebounced(id: Id, path: string, value: JsonValue | undefined, ms = 150): void {
  const p = patchAt(path, value);
  const st = graphState(id);
  if (st.spec) useGraphs.getState().set(id, { spec: applyMergePatch(st.spec as unknown as JsonValue, p) as unknown as GraphSpecT });
  const prev = pending[id];
  if (prev) clearTimeout(prev.timer);
  const patch = prev ? combinePatches(prev.patch, p) : p;
  pending[id] = {
    patch,
    timer: setTimeout(() => {
      delete pending[id];
      void patchGraph(id, patch);
    }, ms),
  };
}

/** Send any debounced edit now (before export, undo or closing). */
export async function flushGraph(id: Id): Promise<void> {
  const p = pending[id];
  if (!p) return;
  clearTimeout(p.timer);
  delete pending[id];
  await patchGraph(id, p.patch);
}

export function selectTarget(id: Id, target: string | null): void {
  useGraphs.getState().set(id, { selected: target });
}

export function setZoom(id: Id, zoom: Zoom): void {
  useGraphs.getState().set(id, { zoom });
}

/** The JSON pointer of the selected target. */
export function selectedPointer(st: GraphState): string | null {
  if (!st.selected || !st.rendered) return null;
  return st.rendered.targets.find((t) => t.id === st.selected)?.target ?? null;
}
