// Messages between the GetLocked desktop app and the browser extension.
// Mirrors crates/core/src/protocol.rs. Keep them in sync and bump
// PROTOCOL_VERSION on breaking changes.

export const PROTOCOL_VERSION = 1;

/** The app listens on the first free port in this range on 127.0.0.1. */
export const PORTS = [47613, 47614, 47615, 47616, 47617] as const;

export type ListMode = "off" | "block" | "allow";

export interface SiteRulesMsg {
  /** False when no session is blocking (idle or on a break). */
  active: boolean;
  mode: ListMode;
  /** Bare domains such as `youtube.com`; subdomains are included. */
  domains: string[];
  /** Unix ms when the current blocking phase ends on its own, if known. */
  endsAt: number | null;
  label: string | null;
}

/** App → extension. */
export type ToExtension =
  | { type: "welcome"; protocol: number }
  | ({ type: "rules" } & SiteRulesMsg)
  | { type: "ping" };

export type BrowserFamily = "chrome" | "edge" | "brave" | "opera" | "vivaldi" | "firefox" | "chromium";

/** Extension → app. */
export type FromExtension =
  | { type: "hello"; protocol: number; browser: BrowserFamily; version: string; incognitoAllowed: boolean }
  | { type: "heartbeat" }
  | { type: "blocked"; domain: string };
