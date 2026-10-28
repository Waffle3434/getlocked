// Pure helpers for turning GetLocked site rules into browser behavior.
// Kept free of extension APIs so they can be unit-tested.

import type { SiteRulesMsg } from "@getlocked/protocol";

/** Always reachable in "allow only" mode so local dev servers keep working. */
export const ALWAYS_ALLOWED = ["localhost", "127.0.0.1"];

/** Hostname of an http(s) URL, lowercased, without a leading `www.`. */
export function hostOf(url: string): string | null {
  try {
    const u = new URL(url);
    if (u.protocol !== "http:" && u.protocol !== "https:") return null;
    return u.hostname.toLowerCase().replace(/^www\./, "");
  } catch {
    return null;
  }
}

/** True if `host` is `domain` or a subdomain of it. */
export function matchesDomain(host: string, domain: string): boolean {
  return host === domain || host.endsWith(`.${domain}`);
}

/** Should this URL be blocked under these rules? */
export function isBlocked(url: string, rules: SiteRulesMsg | null): boolean {
  if (!rules?.active || rules.mode === "off") return false;
  const host = hostOf(url);
  if (!host) return false;
  const listed = rules.domains.some((d) => matchesDomain(host, d));
  if (rules.mode === "block") return listed;
  return !listed && !ALWAYS_ALLOWED.some((d) => matchesDomain(host, d));
}

/** Rule ids GetLocked owns; everything is replaced on each update. */
export const RULE_IDS = { redirect: 1, allow: 2 } as const;

/**
 * declarativeNetRequest rules for the given site rules. Blocked top-level
 * page loads are redirected to the extension's blocked page, with the
 * site's host passed along as `?d=`.
 */
export function buildDnrRules(rules: SiteRulesMsg | null, blockedPageUrl: string): chrome.declarativeNetRequest.Rule[] {
  if (!rules?.active || rules.mode === "off") return [];
  if (rules.mode === "block" && rules.domains.length === 0) return [];

  const redirect = {
    type: "redirect",
    redirect: { regexSubstitution: `${blockedPageUrl}?d=\\1` },
  } as chrome.declarativeNetRequest.RuleAction;
  const regexFilter = "^https?://(?:www\\.)?([^/?#:]+)";

  if (rules.mode === "block") {
    return [
      {
        id: RULE_IDS.redirect,
        priority: 1,
        action: redirect,
        condition: {
          regexFilter,
          requestDomains: rules.domains,
          resourceTypes: ["main_frame" as chrome.declarativeNetRequest.ResourceType],
        },
      },
    ];
  }

  return [
    {
      id: RULE_IDS.redirect,
      priority: 1,
      action: redirect,
      condition: { regexFilter, resourceTypes: ["main_frame" as chrome.declarativeNetRequest.ResourceType] },
    },
    {
      id: RULE_IDS.allow,
      priority: 2,
      action: { type: "allow" as chrome.declarativeNetRequest.RuleActionType },
      condition: {
        requestDomains: [...rules.domains, ...ALWAYS_ALLOWED],
        resourceTypes: ["main_frame" as chrome.declarativeNetRequest.ResourceType],
      },
    },
  ];
}

/**
 * Cached rules stay in force while the app is unreachable (it may have been
 * killed or be restarting) until the phase's end time, or, when there is no
 * end time (stopwatch), until 10 minutes after we last heard from the app.
 * That way uninstalling the app can never leave sites blocked forever.
 */
export const STALE_WITHOUT_END_MS = 10 * 60 * 1000;

export function cachedRulesExpired(rules: SiteRulesMsg | null, lastContact: number, now: number): boolean {
  if (!rules?.active) return false;
  const until = rules.endsAt ?? lastContact + STALE_WITHOUT_END_MS;
  return now >= until;
}

/** "41:05" style countdown. */
export function clock(ms: number): string {
  const s = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}
