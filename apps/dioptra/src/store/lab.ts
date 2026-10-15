// Dioptra Lab license: the key is stored by the shell (a file in the app's config directory
// on desktop, browser storage on the web) and checked by the engine. Nothing leaves the machine.
import { create } from "zustand";
import * as cmd from "../engine/commands";
import { engine } from "../engine/engine";
import type { LicenseView } from "../engine/types";
import { isDesktop } from "../io/files";
import { useNotices } from "./notices";

export const PRICING_URL = "https://zahakj.github.io/dioptra/pricing.html";
const STORAGE_KEY = "dioptra.lab.key";

/** Today in the local calendar, `YYYY-MM-DD`. */
export function localToday(now = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${p(now.getMonth() + 1)}-${p(now.getDate())}`;
}

async function readKey(): Promise<string | null> {
  if (isDesktop()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string | null>("license_key_read");
  }
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

async function writeKey(key: string | null): Promise<void> {
  if (isDesktop()) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("license_key_write", { key });
    return;
  }
  try {
    if (key) localStorage.setItem(STORAGE_KEY, key);
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Private windows: the key lasts for this session only.
  }
}

interface LabState {
  view: LicenseView;
  dialogOpen: boolean;
  setDialogOpen(open: boolean): void;
  /** Read the stored key at startup and tell the engine. */
  load(): Promise<void>;
  /** Check and store a key; returns the engine's view (with `error` if refused). */
  activate(key: string): Promise<LicenseView>;
  remove(): Promise<void>;
}

export const useLab = create<LabState>((set) => ({
  view: { lab_enabled: false },
  dialogOpen: false,
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  async load() {
    const key = await readKey().catch(() => null);
    if (!key) return;
    const view = await cmd.setLicense(await engine(), key, localToday());
    set({ view });
    remindIfEnding(view);
  },
  async activate(key) {
    const view = await cmd.setLicense(await engine(), key, localToday());
    if (!view.error) {
      await writeKey(key.trim());
      set({ view });
    }
    return view;
  },
  async remove() {
    await writeKey(null);
    set({ view: await cmd.setLicense(await engine(), null, localToday()) });
  },
}));

/** Whether Lab features are on. */
export const labEnabled = () => useLab.getState().view.lab_enabled;

/** A one-line description of where the license stands. */
export function standingText(view: LicenseView): string {
  const s = view.standing;
  const l = view.license;
  if (!s || !l)
    return "No license key. Lab features are off; everything else works as before.";
  switch (s.state) {
    case "active":
      return s.days_left > 45
        ? `Active until ${l.expires}.`
        : `Active until ${l.expires} (${s.days_left} days left).`;
    case "grace":
      return `Ended on ${l.expires}. Lab features stay on for ${s.days_left} more days.`;
    case "lapsed":
      return `Ended on ${l.expires}. Lab features are paused; your projects open as before.`;
    case "not_yet_valid":
      return `Starts on ${l.issued}.`;
  }
}

function remindIfEnding(view: LicenseView): void {
  const s = view.standing;
  if (!s || (s.state === "active" && s.days_left > 14)) return;
  if (
    s.state === "not_yet_valid" ||
    (s.state === "lapsed" && s.days_since > 60)
  )
    return;
  useNotices.getState().push(
    {
      kind: "info",
      text: `Dioptra Lab: ${standingText(view)}`,
      action: {
        label: "Renew",
        run: () => useLab.getState().setDialogOpen(true),
      },
    },
    12000,
  );
}
