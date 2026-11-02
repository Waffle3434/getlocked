import type { AppRule, EndPolicy, SessionConfig } from "./types";

/** 3725 → "1:02:05", 65 → "1:05" */
export function clock(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** 5400 → "1h 30m", 600 → "10m", 0 → "0m" */
export function duration(secs: number): string {
  const mins = Math.round(secs / 60);
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

export interface EndButton {
  label: string;
  disabled: boolean;
}

/** What the End button says and whether it can be pressed. */
export function endButton(policy: EndPolicy, focusSecs: number): EndButton {
  switch (policy.type) {
    case "free":
      return { label: "End session", disabled: false };
    case "needsCode":
      return { label: "End early (code required)", disabled: false };
    case "lockedUntil": {
      const left = Math.max(0, policy.focusSecs - focusSecs);
      return { label: `Locked for ${clock(left)} more`, disabled: true };
    }
  }
}

export function ruleLabel(rule: AppRule): string {
  if (rule.type === "exe") return rule.name;
  const parts = rule.path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return `${parts[parts.length - 1] || rule.path} (folder)`;
}

export function ruleKey(rule: AppRule): string {
  return rule.type === "exe" ? `exe:${rule.name.toLowerCase()}` : `folder:${rule.path.toLowerCase()}`;
}

/** One-line summary, e.g. "50m · Friction · blocks 3 apps, 5 sites". */
export function describe(c: SessionConfig): string {
  const parts: string[] = [];
  parts.push(c.kind.type === "timed" ? duration(c.kind.durationSecs) : "Stopwatch");
  parts.push(c.strictness[0].toUpperCase() + c.strictness.slice(1));
  const lists: string[] = [];
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
  if (c.apps.mode !== "off") {
    lists.push(`${c.apps.mode === "block" ? "blocks" : "allows only"} ${plural(c.apps.rules.length, "app")}`);
  }
  if (c.sites.mode !== "off") {
    lists.push(`${c.sites.mode === "block" ? "blocks" : "allows only"} ${plural(c.sites.domains.length, "site")}`);
  }
  if (lists.length) parts.push(lists.join(", "));
  if (c.pomodoro) parts.push(`Pomodoro ${duration(c.pomodoro.focusSecs)}/${duration(c.pomodoro.breakSecs)}`);
  return parts.join(" · ");
}

/**
 * Same rule as the app: whitespace-separated words containing a letter or
 * digit. `\p{Alphabetic}` + `\p{N}` is exactly Rust's `char::is_alphanumeric`.
 */
export function wordCount(s: string): number {
  return s.split(/\s+/).filter((w) => /[\p{Alphabetic}\p{N}]/u.test(w)).length;
}

export const DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/** 570 → "09:30" */
export function minuteToTime(minute: number): string {
  return `${String(Math.floor(minute / 60)).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}`;
}

/** "09:30" → 570 */
export function timeToMinute(time: string): number {
  const [h, m] = time.split(":").map(Number);
  return (h || 0) * 60 + (m || 0);
}

export function newId(): string {
  return crypto.randomUUID();
}

export function blankConfig(): SessionConfig {
  return {
    label: null,
    kind: { type: "timed", durationSecs: 50 * 60 },
    strictness: "friction",
    apps: { mode: "block", rules: [] },
    sites: { mode: "block", domains: [] },
    pomodoro: null,
    requireExtension: true,
  };
}
