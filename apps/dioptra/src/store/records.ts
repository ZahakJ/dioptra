// Records for regulated labs: users on this computer, the signed-in user and the state of the
// open project's audit trail. The users file is stored by the shell (a file in the app's
// config directory on desktop, browser storage on the web); the engine checks it.
import { create } from 'zustand';
import type { Engine } from '../engine/bridge';
import { EngineError } from '../engine/bridge';
import type { Command, Response } from '../engine/types';
import { isDesktop } from '../io/files';
import { withEngine } from './engine';

/** Start of the engine's error for a change without a reason. */
export const REASON_REQUIRED = 'A reason is required for every change to a regulated project.';
/** Start of the engine's error for a change without a signed-in user. */
export const SIGN_IN_REQUIRED = 'Sign in to change a regulated project.';

export type Role = 'analyst' | 'reviewer' | 'admin';
export type Meaning = 'authored' | 'reviewed' | 'approved';
export type SignatureStatus = 'valid' | 'superseded' | 'content_changed';

export interface UserView {
  id: string;
  name: string;
  role: Role;
  disabled: boolean;
  locked: boolean;
}

export interface UsersLogEntry {
  seq: number;
  at: string;
  by: string;
  action: string;
  user: string;
  detail: string;
}

export interface UsersInfo {
  users: UserView[];
  current?: UserView;
  loaded: boolean;
  log: UsersLogEntry[];
}

export interface SignatureView {
  seq: number;
  at: string;
  user: string;
  name: string;
  meaning: Meaning;
  version: number;
  content_hash: string;
  status: SignatureStatus;
}

export interface RecordsStatus {
  regulated: boolean;
  intact: boolean;
  entries: number;
  version: number;
  locked: boolean;
  content_matches: boolean;
  first_break?: { line: number; problem: string };
  signatures: SignatureView[];
  user?: UserView;
  has_users: boolean;
}

export interface AuditEntry {
  seq: number;
  at: string;
  kind: 'regulated' | 'change' | 'signature' | 'new_version';
  user: string;
  name: string;
  action: string;
  command?: unknown;
  objects?: string[];
  reason: string;
  version: number;
  content: string;
  signature?: { meaning: Meaning; content_hash: string; audit_head: string };
  prev: string;
  hash: string;
}

export const MEANING_LABEL: Record<Meaning, string> = { authored: 'Authored', reviewed: 'Reviewed', approved: 'Approved' };
export const ROLE_LABEL: Record<Role, string> = { analyst: 'Analyst', reviewer: 'Reviewer', admin: 'Administrator' };

/** The message to show loudly when the records of a regulated project cannot be trusted. */
export function recordsWarning(s: RecordsStatus | null): string | null {
  if (!s?.regulated) return null;
  if (s.first_break)
    return `The audit trail of this project is broken at entry ${s.first_break.line}: ${s.first_break.problem}. The record cannot be trusted.`;
  if (!s.content_matches) return 'This project was changed outside its audit trail. The record cannot be trusted.';
  return null;
}

/** Send a Lab command and return the reply, throwing on `error`. */
export async function labCall<T>(e: Engine, command: Record<string, unknown>): Promise<T> {
  const r = (await e.dispatch(command as unknown as Command)) as Response | { type: string };
  if (r.type === 'error') throw new EngineError((r as { message: string }).message, command as unknown as Command);
  return r as T;
}

const USERS_KEY = 'dioptra.lab.users';
const REASONS_KEY = 'dioptra.records.reasons';

async function readUsersFile(): Promise<string | null> {
  if (isDesktop()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<string | null>('lab_users_read');
  }
  try {
    return localStorage.getItem(USERS_KEY);
  } catch {
    return null;
  }
}

async function writeUsersFile(file: string): Promise<void> {
  if (isDesktop()) {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('lab_users_write', { file });
    return;
  }
  try {
    localStorage.setItem(USERS_KEY, file);
  } catch {
    // Private windows: the users last for this session only.
  }
}

function readReasons(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(REASONS_KEY) ?? '[]') as unknown;
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, 8) : [];
  } catch {
    return [];
  }
}

type UsersReply = UsersInfo & { type: 'lab_users'; file: string; save: boolean };

interface RecordsState {
  users: UsersInfo | null;
  status: RecordsStatus | null;
  /** Error from loading the users file, if it was refused. */
  usersError: string | null;
  recentReasons: string[];
  /** Read the users file and hand it to the engine. */
  load(): Promise<void>;
  /** Run a users command and store the file if it changed. */
  usersCommand(command: Record<string, unknown>): Promise<UsersInfo>;
  /** Fetch the users and store the file if it changed (after a failed sign-in or signature). */
  syncUsers(): Promise<void>;
  refresh(): Promise<RecordsStatus | null>;
  records(command: Record<string, unknown>): Promise<RecordsStatus>;
  rememberReason(reason: string): void;
}

export const useRecords = create<RecordsState>((set, get) => ({
  users: null,
  status: null,
  usersError: null,
  recentReasons: readReasons(),
  async load() {
    const file = await readUsersFile().catch(() => null);
    try {
      const r = await withEngine((e) => labCall<UsersReply>(e, { type: 'lab_users_load', file }));
      set({ users: r, usersError: null });
    } catch (err) {
      set({ usersError: err instanceof Error ? err.message : String(err) });
    }
    await get().refresh();
  },
  async usersCommand(command) {
    try {
      const r = await withEngine((e) => labCall<UsersReply>(e, command));
      if (r.save) await writeUsersFile(r.file);
      set({ users: r });
      void get().refresh();
      return r;
    } catch (err) {
      await get().syncUsers();
      throw err;
    }
  },
  async syncUsers() {
    try {
      const r = await withEngine((e) => labCall<UsersReply>(e, { type: 'lab_users_list' }));
      if (r.save) await writeUsersFile(r.file);
      set({ users: r });
    } catch {
      // Reported by the command that failed.
    }
  },
  async refresh() {
    try {
      const s = await withEngine((e) => labCall<{ type: string } & RecordsStatus>(e, { type: 'lab_verify_records' }));
      set({ status: s });
      return s;
    } catch {
      return null;
    }
  },
  async records(command) {
    try {
      const s = await withEngine((e) => labCall<{ type: string } & RecordsStatus>(e, command));
      set({ status: s });
      return s;
    } finally {
      await get().syncUsers();
    }
  },
  rememberReason(reason) {
    const r = reason.trim();
    if (!r) return;
    const recentReasons = [r, ...get().recentReasons.filter((x) => x !== r)].slice(0, 8);
    set({ recentReasons });
    try {
      localStorage.setItem(REASONS_KEY, JSON.stringify(recentReasons));
    } catch {
      // Not remembered.
    }
  },
}));
