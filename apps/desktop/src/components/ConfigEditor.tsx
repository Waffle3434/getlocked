import { useState, type ReactNode } from "react";
import { Hourglass, Lock, Shield, ShieldAlert, Timer, Unlock } from "lucide-react";
import { api } from "../api";
import { duration, ruleKey, ruleLabel } from "../format";
import type { ListMode, SessionConfig, Strictness } from "../types";
import { AppPicker } from "./AppPicker";
import { Button, Card, Chip, NumberField, SectionTitle, Segmented, Toggle } from "./ui";

const QUICK_DURATIONS = [25, 50, 90, 120];
const SITE_SUGGESTIONS = ["youtube.com", "reddit.com", "tiktok.com", "instagram.com", "x.com", "twitch.tv", "netflix.com"];

const STRICTNESS: { value: Strictness; title: string; body: string; icon: ReactNode }[] = [
  { value: "easy", title: "Easy", body: "End the session any time.", icon: <Unlock size={18} /> },
  {
    value: "friction",
    title: "Friction",
    body: "To end early, wait 30 seconds and type a long random code by hand.",
    icon: <Shield size={18} />,
  },
  {
    value: "hard",
    title: "Hard",
    body: "No ending early. Survives closing the app and restarting the PC.",
    icon: <ShieldAlert size={18} />,
  },
];

const MODE_OPTIONS: { value: ListMode; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "block", label: "Block these" },
  { value: "allow", label: "Allow only these" },
];

function Option({
  active,
  onClick,
  icon,
  title,
  body,
}: {
  active: boolean;
  onClick: () => void;
  icon: ReactNode;
  title: string;
  body: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex flex-1 flex-col gap-1 rounded-xl border p-4 text-left transition-colors ${
        active ? "border-accent-500 bg-accent-600/15" : "border-ink-700 bg-ink-850 hover:border-ink-600"
      }`}
    >
      <span className={`flex items-center gap-2 text-sm font-semibold ${active ? "text-accent-300" : ""}`}>
        {icon}
        {title}
      </span>
      <span className="text-xs leading-relaxed text-ink-400">{body}</span>
    </button>
  );
}

/** Edits everything about a session: used by Start and Presets. */
export function ConfigEditor({ value, onChange }: { value: SessionConfig; onChange: (c: SessionConfig) => void }) {
  const [picking, setPicking] = useState(false);
  const [siteInput, setSiteInput] = useState("");
  const [siteError, setSiteError] = useState("");
  const set = (patch: Partial<SessionConfig>) => onChange({ ...value, ...patch });

  const timedMins = value.kind.type === "timed" ? Math.round(value.kind.durationSecs / 60) : 50;
  const minMins = value.kind.type === "stopwatch" && value.kind.minSecs ? Math.round(value.kind.minSecs / 60) : null;

  const addSites = async (raw: string) => {
    const parts = raw.split(/[\s,]+/).filter(Boolean);
    const added: string[] = [];
    for (const p of parts) {
      const d = await api.normalizeDomain(p);
      if (d) added.push(d);
    }
    if (parts.length && !added.length) {
      setSiteError(`"${raw.trim()}" doesn't look like a website. Try something like youtube.com.`);
      return;
    }
    setSiteError("");
    setSiteInput("");
    set({ sites: { ...value.sites, domains: [...new Set([...value.sites.domains, ...added])] } });
  };

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <SectionTitle hint="Only focus time counts. Pomodoro breaks pause the clock.">How long?</SectionTitle>
        <div className="flex flex-col gap-3 sm:flex-row">
          <Option
            active={value.kind.type === "timed"}
            onClick={() => set({ kind: { type: "timed", durationSecs: timedMins * 60 } })}
            icon={<Timer size={18} />}
            title="Timer"
            body="Count down. When it hits zero you choose: done, or keep going."
          />
          <Option
            active={value.kind.type === "stopwatch"}
            onClick={() => set({ kind: { type: "stopwatch", minSecs: null } })}
            icon={<Hourglass size={18} />}
            title="Stopwatch"
            body="No time limit. Count up until the work is done, then end it."
          />
        </div>
        {value.kind.type === "timed" ? (
          <div className="mt-4 flex flex-wrap items-center gap-2">
            {QUICK_DURATIONS.map((m) => (
              <button
                key={m}
                type="button"
                onClick={() => set({ kind: { type: "timed", durationSecs: m * 60 } })}
                className={`rounded-lg border px-3 py-1.5 text-sm ${
                  timedMins === m ? "border-accent-500 bg-accent-600/20 text-accent-300" : "border-ink-700 hover:border-ink-600"
                }`}
              >
                {duration(m * 60)}
              </button>
            ))}
            <span className="mx-1 text-ink-600">|</span>
            <NumberField
              value={timedMins}
              min={1}
              max={24 * 60}
              suffix="minutes"
              onChange={(m) => set({ kind: { type: "timed", durationSecs: m * 60 } })}
            />
          </div>
        ) : (
          value.strictness === "hard" && (
            <div className="mt-4 flex flex-wrap items-center gap-3">
              <Toggle
                checked={minMins !== null}
                onChange={(on) => set({ kind: { type: "stopwatch", minSecs: on ? 30 * 60 : null } })}
                label="Minimum time"
                hint="In Hard mode, the session can't end before this much focus time, and after that it needs the unlock code."
              />
              {minMins !== null && (
                <NumberField
                  value={minMins}
                  min={1}
                  max={24 * 60}
                  suffix="minutes"
                  onChange={(m) => set({ kind: { type: "stopwatch", minSecs: m * 60 } })}
                />
              )}
            </div>
          )
        )}
      </Card>

      <Card>
        <SectionTitle>How strict?</SectionTitle>
        <div className="flex flex-col gap-3 sm:flex-row">
          {STRICTNESS.map((s) => (
            <Option
              key={s.value}
              active={value.strictness === s.value}
              onClick={() => set({ strictness: s.value })}
              icon={s.icon}
              title={s.title}
              body={s.body}
            />
          ))}
        </div>
        {value.strictness === "hard" && (
          <p className="mt-3 flex items-start gap-2 rounded-lg bg-warn-tint/10 px-3 py-2 text-xs text-warn-soft">
            <Lock size={14} className="mt-0.5 shrink-0" />
            {value.kind.type === "timed"
              ? "Hard mode can't be stopped until the timer runs out."
              : "A Hard stopwatch can only be ended with the unlock code (after the minimum, if set)."}{" "}
            GetLocked will turn on "Start with Windows" so the session survives a restart.
          </p>
        )}
      </Card>

      <Card>
        <div className="flex flex-wrap items-start justify-between gap-3">
          <SectionTitle
            hint={
              value.apps.mode === "allow"
                ? "Any other app that opens a window gets closed. Explorer, the Start menu, Task Manager and GetLocked itself always stay usable."
                : value.apps.mode === "block"
                  ? "These get closed whenever they open during the session."
                  : "Apps aren't restricted."
            }
          >
            Apps
          </SectionTitle>
          <Segmented value={value.apps.mode} options={MODE_OPTIONS} onChange={(mode) => set({ apps: { ...value.apps, mode } })} />
        </div>
        {value.apps.mode !== "off" && (
          <div className="flex flex-wrap items-center gap-2">
            {value.apps.rules.map((r) => (
              <Chip
                key={ruleKey(r)}
                onRemove={() =>
                  set({ apps: { ...value.apps, rules: value.apps.rules.filter((x) => ruleKey(x) !== ruleKey(r)) } })
                }
              >
                {ruleLabel(r)}
              </Chip>
            ))}
            <Button onClick={() => setPicking(true)} className="py-1 text-xs">
              + Add apps
            </Button>
          </div>
        )}
      </Card>

      <Card>
        <div className="flex flex-wrap items-start justify-between gap-3">
          <SectionTitle
            hint={
              value.sites.mode === "off"
                ? "Websites aren't restricted."
                : "Works through the GetLocked browser extension. Subdomains are included."
            }
          >
            Websites
          </SectionTitle>
          <Segmented
            value={value.sites.mode}
            options={MODE_OPTIONS}
            onChange={(mode) => set({ sites: { ...value.sites, mode } })}
          />
        </div>
        {value.sites.mode !== "off" && (
          <div className="flex flex-col gap-3">
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                void addSites(siteInput);
              }}
            >
              <input
                value={siteInput}
                onChange={(e) => setSiteInput(e.target.value)}
                placeholder={value.sites.mode === "block" ? "youtube.com" : "docs.google.com"}
                className="flex-1 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-sm outline-none focus:border-accent-500"
              />
              <Button type="submit">Add</Button>
            </form>
            {siteError && <p className="text-xs text-danger">{siteError}</p>}
            <div className="flex flex-wrap gap-2">
              {value.sites.domains.map((d) => (
                <Chip
                  key={d}
                  onRemove={() => set({ sites: { ...value.sites, domains: value.sites.domains.filter((x) => x !== d) } })}
                >
                  {d}
                </Chip>
              ))}
            </div>
            {value.sites.mode === "block" && (
              <div className="flex flex-wrap gap-1.5 text-xs text-ink-400">
                Quick add:
                {SITE_SUGGESTIONS.filter((s) => !value.sites.domains.includes(s)).map((s) => (
                  <button key={s} type="button" onClick={() => void addSites(s)} className="text-accent-400 hover:underline">
                    {s}
                  </button>
                ))}
              </div>
            )}
            {value.strictness !== "easy" && (
              <Toggle
                checked={value.requireExtension}
                onChange={(requireExtension) => set({ requireExtension })}
                label="Close browsers that don't have the extension"
                hint="Stops the easy workaround of switching to a browser without GetLocked installed."
              />
            )}
          </div>
        )}
      </Card>

      <Card>
        <Toggle
          checked={value.pomodoro !== null}
          onChange={(on) => set({ pomodoro: on ? { focusSecs: 25 * 60, breakSecs: 5 * 60 } : null })}
          label="Pomodoro breaks"
          hint="Work in focus blocks with short breaks. Blocks lift during breaks and come back after."
        />
        {value.pomodoro && (
          <div className="mt-3 flex flex-wrap gap-4">
            <NumberField
              value={Math.round(value.pomodoro.focusSecs / 60)}
              min={1}
              max={240}
              suffix="min focus"
              onChange={(m) => set({ pomodoro: { ...value.pomodoro!, focusSecs: m * 60 } })}
            />
            <NumberField
              value={Math.round(value.pomodoro.breakSecs / 60)}
              min={1}
              max={120}
              suffix="min break"
              onChange={(m) => set({ pomodoro: { ...value.pomodoro!, breakSecs: m * 60 } })}
            />
          </div>
        )}
      </Card>

      {picking && (
        <AppPicker
          rules={value.apps.rules}
          onChange={(rules) => set({ apps: { ...value.apps, rules } })}
          onClose={() => setPicking(false)}
        />
      )}
    </div>
  );
}
