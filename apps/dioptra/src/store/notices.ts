// Small non-modal notices (paste reports, errors, confirmations of saves).
import { create } from 'zustand';

export interface NoticeAction {
  label: string;
  run: () => void | Promise<void>;
}

export interface Notice {
  id: number;
  kind: 'info' | 'error' | 'report';
  text: string;
  detail?: string[];
  action?: NoticeAction;
}

interface NoticeState {
  notices: Notice[];
  push(n: Omit<Notice, 'id'>, timeoutMs?: number): number;
  dismiss(id: number): void;
}

let next = 1;

export const useNotices = create<NoticeState>((set, get) => ({
  notices: [],
  push(n, timeoutMs) {
    const id = next++;
    // Keep the list short: a new report replaces older ones of the same kind.
    set((s) => ({ notices: [...s.notices.filter((x) => !(n.kind === 'report' && x.kind === 'report')), { ...n, id }].slice(-4) }));
    const t = timeoutMs ?? (n.kind === 'error' ? 8000 : n.kind === 'report' ? 12000 : 4000);
    if (t > 0) setTimeout(() => get().dismiss(id), t);
    return id;
  },
  dismiss(id) {
    set((s) => ({ notices: s.notices.filter((n) => n.id !== id) }));
  },
}));

export const notify = (text: string, kind: Notice['kind'] = 'info') => useNotices.getState().push({ kind, text });
export const notifyError = (err: unknown) =>
  useNotices.getState().push({ kind: 'error', text: err instanceof Error ? err.message : String(err) });
