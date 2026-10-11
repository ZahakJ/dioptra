// Engine status: loading, backend, version, and how many commands are in flight (the
// status line's recompute indicator).
import { create } from 'zustand';

export interface EngineStatus {
  ready: boolean;
  error: string | null;
  backend: 'tauri' | 'wasm' | null;
  version: string | null;
  pending: number;
}

export const useEngineStatus = create<EngineStatus>(() => ({
  ready: false,
  error: null,
  backend: null,
  version: null,
  pending: 0,
}));

export function trackPending<T>(p: Promise<T>): Promise<T> {
  useEngineStatus.setState((s) => ({ pending: s.pending + 1 }));
  const done = () => useEngineStatus.setState((s) => ({ pending: Math.max(0, s.pending - 1) }));
  p.then(done, done);
  return p;
}
