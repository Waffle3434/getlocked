// Messages between the GetLocked desktop app and the browser extension.
// Mirrors crates/core/src/protocol.rs. Keep them in sync and bump
// PROTOCOL_VERSION on breaking changes.

export const PROTOCOL_VERSION = 2;

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

/**
 * App → extension. A connection starts with the pairing handshake: the app
 * sends `challenge`, the extension answers `auth`, and the app replies
 * `welcome` with its own proof, or `pairingFailed` and hangs up.
 */
export type ToExtension =
  | { type: "challenge"; protocol: number; nonce: string }
  | { type: "welcome"; protocol: number; proof: string }
  | { type: "pairingFailed" }
  | ({ type: "rules" } & SiteRulesMsg)
  | { type: "ping" };

export type BrowserFamily = "chrome" | "edge" | "brave" | "opera" | "vivaldi" | "firefox" | "chromium";

/** Extension → app. */
export type FromExtension =
  | { type: "auth"; nonce: string; proof: string }
  | { type: "hello"; protocol: number; browser: BrowserFamily; version: string; incognitoAllowed: boolean }
  | { type: "heartbeat" }
  | { type: "blocked"; domain: string };
