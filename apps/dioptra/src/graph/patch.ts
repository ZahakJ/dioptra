// RFC 7396 JSON merge patches for the graph spec: the inspector sends only the changed path.
// Arrays are replaced whole by a merge patch, so edits inside `style.datasets` and
// `annotations` send the whole (edited) array.
import type { JsonObject, JsonValue } from '../engine/types';

/** Split a JSON pointer (`/axes/y/title`) or a dot path (`axes.y.title`) into keys. */
export function pathKeys(path: string | readonly string[]): string[] {
  if (Array.isArray(path)) return [...path];
  const p = path as string;
  if (p.startsWith('/')) {
    return p
      .slice(1)
      .split('/')
      .filter((k) => k !== '')
      .map((k) => k.replace(/~1/g, '/').replace(/~0/g, '~'));
  }
  return p.split('.').filter(Boolean);
}

/**
 * The merge patch that sets `value` at `path`; `undefined` or `null` removes the field
 * (the engine then uses its default).
 */
export function patchAt(path: string | readonly string[], value: JsonValue | undefined): JsonObject {
  const keys = pathKeys(path);
  if (!keys.length) throw new Error('a patch needs a path');
  let out: JsonValue = value === undefined ? null : value;
  for (let i = keys.length - 1; i >= 0; i--) out = { [keys[i]!]: out };
  return out as JsonObject;
}

/** Merge two patches (later wins), for debounced edits sent together. */
export function combinePatches(a: JsonObject, b: JsonObject): JsonObject {
  const out: JsonObject = { ...a };
  for (const [k, v] of Object.entries(b)) {
    const prev = out[k];
    if (v && typeof v === 'object' && !Array.isArray(v) && prev && typeof prev === 'object' && !Array.isArray(prev)) {
      out[k] = combinePatches(prev as JsonObject, v as JsonObject);
    } else out[k] = v;
  }
  return out;
}

/** Apply a merge patch (RFC 7396) to a value; returns a new value. */
export function applyMergePatch(target: JsonValue | undefined, patch: JsonValue): JsonValue {
  if (patch === null || typeof patch !== 'object' || Array.isArray(patch)) return patch;
  const base: JsonObject =
    target && typeof target === 'object' && !Array.isArray(target) ? { ...(target as JsonObject) } : {};
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete base[k];
    else base[k] = applyMergePatch(base[k], v);
  }
  return base;
}

/** Read the value at a JSON pointer or dot path. */
export function getPath(obj: JsonValue | undefined, path: string | readonly string[]): JsonValue | undefined {
  let cur: JsonValue | undefined = obj;
  for (const k of pathKeys(path)) {
    if (cur === null || cur === undefined || typeof cur !== 'object') return undefined;
    cur = Array.isArray(cur) ? cur[Number(k)] : (cur as JsonObject)[k];
  }
  return cur;
}
