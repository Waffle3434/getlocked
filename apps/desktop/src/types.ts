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
