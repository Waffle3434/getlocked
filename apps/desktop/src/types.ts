// Mirrors the serde types in crates/core (camelCase JSON).

export type Strictness = "easy" | "friction" | "hard";
export type SessionKind =
  | { type: "timed"; durationSecs: number }
  | { type: "stopwatch"; minSecs: number | null };
export type ListMode = "off" | "block" | "allow";
export type AppRule = { type: "exe"; name: string } | { type: "folder"; path: string };

export interface AppRules {
  mode: ListMode;
  rules: AppRule[];
}

export interface SiteRules {
  mode: ListMode;
  domains: string[];
}

export interface PomodoroCfg {
  focusSecs: number;
  breakSecs: number;
}

export interface SessionConfig {
  label: string | null;
  kind: SessionKind;
  strictness: Strictness;
  apps: AppRules;
  sites: SiteRules;
  pomodoro: PomodoroCfg | null;
  requireExtension: boolean;
}

export interface Preset {
  id: string;
  name: string;
  config: SessionConfig;
}

export interface Schedule {
  id: string;
  presetId: string;
  /** 0 = Monday … 6 = Sunday */
  days: number[];
  /** Minutes after local midnight */
  startMinute: number;
  enabled: boolean;
}

export interface Settings {
  autostart: boolean;
  notifications: boolean;
  extensionGraceSecs: number;
  /** What taunts call the user. Null: the Windows account name. */
  displayName: string | null;
  /** Roast the user when they hit a block. */
  roast: boolean;
  theme: Theme;
}

export type ThemeMode = "dark" | "light";
export type AccentName = "terracotta" | "ocean" | "forest" | "violet" | "rose" | "custom";

export interface Theme {
  mode: ThemeMode;
  accent: AccentName;
  /** `#rrggbb`, used when `accent` is "custom" (kept otherwise). */
  customAccent: string | null;
}

export interface Config {
  presets: Preset[];
  schedules: Schedule[];
  settings: Settings;
}

export type Phase = "focus" | "break" | "timeUp";

export type EndPolicy =
  | { type: "free" }
  | { type: "needsCode" }
  | { type: "lockedUntil"; focusSecs: number; thenNeedsCode: boolean };

export interface SessionView {
  id: string;
  label: string | null;
  config: SessionConfig;
  startedAt: string;
  phase: Phase;
  focusSecs: number;
  remainingSecs: number | null;
  phaseRemainingSecs: number | null;
  overtime: boolean;
  overtimeSecs: number;
  blocking: boolean;
  endPolicy: EndPolicy;
  blockedCount: number;
  tasks: Task[];
  /** At least one task is left (not dropped) and every one left is done. */
  allTasksDone: boolean;
}

export type TaskState = { type: "open" } | { type: "done"; note: string } | { type: "dropped"; reason: string };

export interface Task {
  id: number;
  text: string;
  state: TaskState;
}

/** Live check of a tick note or drop reason (`tasks::check_note` in the core). */
export interface NoteCheck {
  words: number;
  /** Different words found in the word list. */
  realWords: number;
  /** Whether a tick or drop would accept it. */
  ok: boolean;
}

/** A task as stored in the stats. */
export interface TaskRow {
  text: string;
  state: "open" | "done" | "dropped";
  note: string | null;
  reason: string | null;
}

export interface ExtensionClient {
  browser: string;
  version: string;
  incognitoAllowed: boolean;
}

export interface Snapshot {
  session: SessionView | null;
  config: Config;
  extensions: ExtensionClient[];
  bridgePort: number | null;
  /** Typed into the extension's popup to pair it with this app. */
  pairingCode: string;
  dataDir: string;
  version: string;
}

export type UnlockStatus =
  | { type: "none" }
  | { type: "coolingDown"; secsLeft: number }
  | { type: "ready"; code: string };

export interface RunningApp {
  exeName: string;
  path: string | null;
}

export interface InstalledApp {
  name: string;
  exeName: string;
  path: string;
}

export interface DayStat {
  date: string;
  focusSecs: number;
  sessions: number;
}

export interface BlockedTarget {
  kind: "app" | "site";
  target: string;
  count: number;
}

export interface SessionRow {
  id: string;
  label: string | null;
  kind: "timed" | "stopwatch";
  strictness: Strictness;
  startedAt: string;
  focusSecs: number;
  overtimeSecs: number;
  outcome: "completed" | "endedEarly";
  blockedCount: number;
  tasks: TaskRow[];
}

export interface StatsSummary {
  days: DayStat[];
  totalFocusSecs: number;
  completed: number;
  endedEarly: number;
  currentStreak: number;
  bestStreak: number;
  topBlocked: BlockedTarget[];
  recent: SessionRow[];
}

export type ExtensionTarget = "chromium" | "firefox";

export interface BrowserInfo {
  id: string;
  name: string;
  target: ExtensionTarget;
  extensionsUrl: string;
  installed: boolean;
}
