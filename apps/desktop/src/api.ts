import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BrowserInfo,
  ExtensionTarget,
  Config,
  InstalledApp,
  NoteCheck,
  Preset,
  RunningApp,
  Schedule,
  SessionConfig,
  Settings,
  Snapshot,
  StatsSummary,
  UnlockStatus,
} from "./types";

// Thin typed wrappers around the Rust commands in src-tauri/src/commands.rs.
export const api = {
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  startSession: (config: SessionConfig, tasks: string[]) => invoke<void>("start_session", { config, tasks }),
  addTask: (text: string) => invoke<number>("add_task", { text }),
  editTask: (id: number, text: string) => invoke<void>("edit_task", { id, text }),
  tickTask: (id: number, note: string) => invoke<void>("tick_task", { id, note }),
  untickTask: (id: number) => invoke<void>("untick_task", { id }),
  dropTask: (id: number, reason: string) => invoke<void>("drop_task", { id, reason }),
  checkNote: (note: string) => invoke<NoteCheck>("check_note", { note }),
  startPreset: (id: string) => invoke<void>("start_preset", { id }),
  beginUnlock: () => invoke<UnlockStatus>("begin_unlock"),
  unlockStatus: () => invoke<UnlockStatus>("unlock_status"),
  cancelUnlock: () => invoke<void>("cancel_unlock"),
  endSession: (code: string | null) => invoke<void>("end_session", { code }),
  timeUpChoice: (keepGoing: boolean) => invoke<void>("time_up_choice", { keepGoing }),
  savePreset: (preset: Preset) => invoke<Config>("save_preset", { preset }),
  deletePreset: (id: string) => invoke<Config>("delete_preset", { id }),
  saveSchedule: (schedule: Schedule) => invoke<Config>("save_schedule", { schedule }),
  deleteSchedule: (id: string) => invoke<Config>("delete_schedule", { id }),
  saveSettings: (settings: Settings) => invoke<Config>("save_settings", { settings }),
  listRunningApps: () => invoke<RunningApp[]>("list_running_apps"),
  listInstalledApps: () => invoke<InstalledApp[]>("list_installed_apps"),
  getStats: (days: number) => invoke<StatsSummary>("get_stats", { days }),
  normalizeDomain: (input: string) => invoke<string | null>("normalize_domain", { input }),
  quit: () => invoke<void>("quit_app"),
  listBrowsers: () => invoke<BrowserInfo[]>("list_browsers"),
  prepareExtension: (target: ExtensionTarget) => invoke<string>("prepare_extension", { target }),
  revealExtension: (target: ExtensionTarget) => invoke<void>("reveal_extension", { target }),
  resetPairingCode: () => invoke<string>("reset_pairing_code"),
  openExtensionsPage: (browser: string) => invoke<void>("open_extensions_page", { browser }),
};

export function onState(cb: (s: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("state", (e) => cb(e.payload));
}

export function onQuitBlocked(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("quit-blocked", (e) => cb(e.payload));
}

/** Tauri command errors arrive as plain strings. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : "Something went wrong.";
}
