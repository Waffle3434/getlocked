import { describe, expect, it } from "vitest";
import type { SiteRulesMsg } from "@getlocked/protocol";
import { buildDnrRules, cachedRulesExpired, clock, hostOf, isBlocked, STALE_WITHOUT_END_MS } from "./rules";

const rules = (mode: SiteRulesMsg["mode"], domains: string[], active = true): SiteRulesMsg => ({
  active,
  mode,
  domains,
  endsAt: null,
  label: null,
});

describe("hostOf", () => {
  it("handles http(s) only and strips www", () => {
    expect(hostOf("https://www.YouTube.com/watch?v=1")).toBe("youtube.com");
    expect(hostOf("http://m.reddit.com:8080/r/x")).toBe("m.reddit.com");
    expect(hostOf("chrome://settings")).toBeNull();
    expect(hostOf("not a url")).toBeNull();
  });
});

describe("isBlocked", () => {
  it("blocks listed domains and their subdomains", () => {
    const r = rules("block", ["youtube.com"]);
    expect(isBlocked("https://youtube.com", r)).toBe(true);
    expect(isBlocked("https://music.youtube.com/x", r)).toBe(true);
    expect(isBlocked("https://notyoutube.com", r)).toBe(false);
    expect(isBlocked("https://github.com", r)).toBe(false);
  });

  it("allow mode blocks everything else but keeps localhost", () => {
    const r = rules("allow", ["github.com"]);
    expect(isBlocked("https://gist.github.com", r)).toBe(false);
    expect(isBlocked("https://youtube.com", r)).toBe(true);
    expect(isBlocked("http://localhost:5173", r)).toBe(false);
    expect(isBlocked("chrome://extensions", r)).toBe(false);
  });

  it("does nothing when inactive or off", () => {
    expect(isBlocked("https://youtube.com", rules("block", ["youtube.com"], false))).toBe(false);
    expect(isBlocked("https://youtube.com", rules("off", ["youtube.com"]))).toBe(false);
    expect(isBlocked("https://youtube.com", null)).toBe(false);
  });
});

describe("buildDnrRules", () => {
  const page = "chrome-extension://abc/blocked.html";

  it("block mode is one redirect rule scoped to the domains", () => {
    const out = buildDnrRules(rules("block", ["youtube.com", "x.com"]), page);
    expect(out).toHaveLength(1);
    expect(out[0].condition.requestDomains).toEqual(["youtube.com", "x.com"]);
    expect(out[0].action.redirect?.regexSubstitution).toBe(`${page}?d=\\1`);
  });

  it("allow mode is a catch-all redirect plus a higher-priority allow", () => {
    const out = buildDnrRules(rules("allow", ["github.com"]), page);
    expect(out).toHaveLength(2);
    const allow = out.find((r) => r.action.type === "allow")!;
    expect(allow.priority).toBeGreaterThan(out.find((r) => r.action.type === "redirect")!.priority!);
    expect(allow.condition.requestDomains).toContain("github.com");
    expect(allow.condition.requestDomains).toContain("localhost");
  });

  it("no rules when inactive or empty blocklist", () => {
    expect(buildDnrRules(rules("block", []), page)).toEqual([]);
    expect(buildDnrRules(rules("block", ["a.com"], false), page)).toEqual([]);
  });

  it("the redirect regex captures the host", () => {
    const re = new RegExp(buildDnrRules(rules("block", ["a.com"]), page)[0].condition.regexFilter!);
    expect("https://www.a.com/path".match(re)?.[1]).toBe("a.com");
    expect("http://sub.a.com:443/".match(re)?.[1]).toBe("sub.a.com");
  });
});

describe("cachedRulesExpired", () => {
  it("uses endsAt when known", () => {
    const r = { ...rules("block", ["a.com"]), endsAt: 1000 };
    expect(cachedRulesExpired(r, 0, 999)).toBe(false);
    expect(cachedRulesExpired(r, 0, 1000)).toBe(true);
  });
  it("falls back to last contact for stopwatch sessions", () => {
    const r = rules("block", ["a.com"]);
    expect(cachedRulesExpired(r, 5000, 5000 + STALE_WITHOUT_END_MS - 1)).toBe(false);
    expect(cachedRulesExpired(r, 5000, 5000 + STALE_WITHOUT_END_MS)).toBe(true);
  });
});

describe("clock", () => {
  it("formats", () => {
    expect(clock(61_000)).toBe("1:01");
    expect(clock(3_600_000)).toBe("1:00:00");
  });
});
