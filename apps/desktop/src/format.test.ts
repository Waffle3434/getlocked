import { describe as group, expect, it } from "vitest";
import { blankConfig, clock, describe, duration, endButton, minuteToTime, ruleLabel, timeToMinute, wordCount } from "./format";

group("clock", () => {
  it("formats minutes and hours", () => {
    expect(clock(65)).toBe("1:05");
    expect(clock(3725)).toBe("1:02:05");
    expect(clock(-3)).toBe("0:00");
  });
});

group("duration", () => {
  it("rounds to minutes", () => {
    expect(duration(0)).toBe("0m");
    expect(duration(600)).toBe("10m");
    expect(duration(3600)).toBe("1h");
    expect(duration(5400)).toBe("1h 30m");
  });
});

group("endButton", () => {
  it("follows the end policy", () => {
    expect(endButton({ type: "free" }, 0)).toEqual({ label: "End session", disabled: false });
    expect(endButton({ type: "needsCode" }, 0).label).toMatch(/code/);
    expect(endButton({ type: "lockedUntil", focusSecs: 600, thenNeedsCode: true }, 540)).toEqual({
      label: "Locked for 1:00 more",
      disabled: true,
    });
  });
});

group("describe", () => {
  it("summarizes a config", () => {
    const c = blankConfig();
    c.apps.rules = [{ type: "exe", name: "steam.exe" }];
    c.sites.domains = ["youtube.com", "reddit.com"];
    expect(describe(c)).toBe("50m · Friction · blocks 1 app, blocks 2 sites");
    c.kind = { type: "stopwatch", minSecs: null };
    c.sites.mode = "allow";
    c.pomodoro = { focusSecs: 1500, breakSecs: 300 };
    expect(describe(c)).toBe("Stopwatch · Friction · blocks 1 app, allows only 2 sites · Pomodoro 25m/5m");
  });
});

group("helpers", () => {
  it("converts times", () => {
    expect(minuteToTime(570)).toBe("09:30");
    expect(timeToMinute("09:30")).toBe(570);
  });
  it("labels folder rules by last segment", () => {
    expect(ruleLabel({ type: "folder", path: "C:\\Games\\Steam\\steamapps\\common\\" })).toBe("common (folder)");
  });
});

group("wordCount", () => {
  it("counts words with a letter or digit, like the app does", () => {
    expect(wordCount("wrote the intro")).toBe(3);
    expect(wordCount("  done ")).toBe(1);
    expect(wordCount("- . ! ?")).toBe(0);
    expect(wordCount("a - b . c")).toBe(3);
  });
});
