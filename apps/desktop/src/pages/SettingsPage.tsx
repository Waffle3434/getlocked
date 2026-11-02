import { useEffect, useRef, useState } from "react";
import { Check, CheckCircle2, Palette, Puzzle, XCircle } from "lucide-react";
import { api, errorText } from "../api";
import { ExtensionSetup, PairingCode } from "../components/ExtensionSetup";
import { Button, Card, ErrorText, NumberField, SectionTitle, Segmented, Toggle } from "../components/ui";
import { ACCENTS, ACCENT_NAMES, applyTheme, baseScale, isHex, normalizeTheme, onAccent } from "../theme";
import type { AccentName, Config, Settings, Snapshot, Theme, ThemeMode } from "../types";

export function SettingsPage({ snapshot, onConfig }: { snapshot: Snapshot; onConfig: (c: Config) => void }) {
  const [error, setError] = useState("");
  const [settingUp, setSettingUp] = useState(false);
  const settings = snapshot.config.settings;
  // Debounced saves run after a re-render; build them on the latest settings.
  const latest = useRef(settings);
  latest.current = settings;

  /** Resolves to whether the save went through. */
  const save = async (patch: Partial<Settings>) => {
    setError("");
    try {
      onConfig(await api.saveSettings({ ...latest.current, ...patch }));
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <header>
        <h1 className="text-3xl">Settings</h1>
      </header>
      <ErrorText>{error}</ErrorText>

      <Appearance
        theme={normalizeTheme(settings.theme)}
        onSave={async (theme) => {
          // Put the saved look back if the new one was refused.
          if (!(await save({ theme }))) applyTheme(latest.current.theme);
        }}
      />

      <Card className="flex flex-col gap-5">
        <Toggle
          checked={settings.autostart}
          onChange={(autostart) => save({ autostart })}
          label="Start with Windows"
          hint="Opens quietly in the tray when you sign in, so schedules run and Hard sessions survive a restart. Hard sessions turn this on automatically."
        />
        <Toggle
          checked={settings.notifications}
          onChange={(notifications) => save({ notifications })}
          label="Notifications"
          hint="Show a toast when something is blocked, a break starts, or time is up."
        />
        <Toggle
          checked={settings.roast}
          onChange={(roast) => save({ roast })}
          label="Roast me when I slip"
          hint="When you hit a block, GetLocked reminds you how many sessions you've bailed on this week."
        />
        <label className="flex items-center justify-between gap-4">
          <span>
            <span className="block text-sm">What should GetLocked call you?</span>
            <span className="block text-xs text-ink-400">Used in roasts. Leave empty to use your Windows name.</span>
          </span>
          <input
            key={settings.displayName ?? ""}
            defaultValue={settings.displayName ?? ""}
            maxLength={40}
            placeholder="Your Windows name"
            onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
            onBlur={(e) => {
              const displayName = e.target.value.trim() || null;
              if (displayName !== settings.displayName) void save({ displayName });
            }}
            className="w-48 shrink-0 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-sm outline-none focus:border-accent-500"
          />
        </label>
        <div className="flex items-center justify-between gap-4">
          <span>
            <span className="block text-sm">Browser grace period</span>
            <span className="block text-xs text-ink-400">
              How long a browser may run without the extension before it's closed (when that option is on).
            </span>
          </span>
          <NumberField
            value={settings.extensionGraceSecs}
            min={3}
            max={120}
            suffix="sec"
            onChange={(extensionGraceSecs) => save({ extensionGraceSecs })}
          />
        </div>
      </Card>

      <Card>
        <div className="flex flex-wrap items-start justify-between gap-3">
          <SectionTitle hint="Website blocking works through a small extension that talks to this app on your PC only.">
            <span className="flex items-center gap-2">
              <Puzzle size={16} /> Browser extension
            </span>
          </SectionTitle>
          <Button variant={snapshot.extensions.length ? "secondary" : "primary"} onClick={() => setSettingUp(true)}>
            {snapshot.extensions.length ? "Add to another browser" : "Set up extension"}
          </Button>
        </div>
        {snapshot.extensions.length === 0 ? (
          <p className="flex items-center gap-2 text-sm text-warn">
            <XCircle size={16} /> Not connected in any browser yet.
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {snapshot.extensions.map((e, i) => (
              <li key={i} className="flex items-center gap-2 text-sm">
                <CheckCircle2 size={16} className="text-ok-icon" />
                <span className="capitalize">{e.browser}</span>
                <span className="text-ink-400">v{e.version}</span>
                {!e.incognitoAllowed && (
                  <span className="text-xs text-warn">· not allowed in private windows (Hard mode needs this)</span>
                )}
              </li>
            ))}
          </ul>
        )}
        <div className="mt-4 flex flex-col gap-2 border-t border-ink-800 pt-4">
          <div className="text-sm">Pairing code</div>
          <p className="text-xs text-ink-400">
            The extension needs this code to connect, so other programs on this PC can't pose as GetLocked or as the
            extension. Setup fills it in for you; paste it into the extension's popup if it asks. A new code disconnects
            every browser until it's paired again.
          </p>
          <PairingCode
            code={snapshot.pairingCode}
            onReset={() => {
              if (window.confirm("Make a new pairing code? Every browser will need to be paired again.")) {
                setError("");
                api.resetPairingCode().catch((e) => setError(errorText(e)));
              }
            }}
          />
        </div>
      </Card>

      <Card className="flex flex-col gap-2 text-sm">
        <SectionTitle>About</SectionTitle>
        <p>
          GetLocked {snapshot.version}. Free and open source under the GPL-3.0. It never modifies other apps or system
          files, and nothing you do here leaves this PC.
        </p>
        <div className="mt-2">
          <Button
            variant="danger"
            onClick={() => api.quit().catch((e) => setError(errorText(e)))}
            disabled={!!snapshot.session && snapshot.session.endPolicy.type !== "free"}
          >
            Quit GetLocked
          </Button>
        </div>
      </Card>

      {settingUp && <ExtensionSetup extensions={snapshot.extensions} pairingCode={snapshot.pairingCode} onClose={() => setSettingUp(false)} />}
    </div>
  );
}

const ACCENT_LABELS: Record<AccentName, string> = {
  terracotta: "Terracotta",
  ocean: "Ocean",
  forest: "Forest",
  violet: "Violet",
  rose: "Rose",
  custom: "Custom",
};

/** Dark/light and the accent color. Changes show at once and save right away
 * (the color picker after it settles for a moment). */
function Appearance({ theme, onSave }: { theme: Theme; onSave: (t: Theme) => Promise<void> }) {
  // The picker's color while dragging, ahead of the saved theme.
  const [draft, setDraft] = useState<string | null>(null);
  const pending = useRef<{ timer: number; theme: Theme } | null>(null);

  const flush = () => {
    const p = pending.current;
    if (!p) return;
    window.clearTimeout(p.timer);
    pending.current = null;
    // Keep showing the picker's color if it moved again while this saved.
    void onSave(p.theme).finally(() => pending.current || setDraft(null));
  };

  // Leaving the page mid-drag still saves the last color.
  const flushRef = useRef(flush);
  flushRef.current = flush;
  useEffect(() => () => flushRef.current(), []);

  const change = (next: Theme, debounce = false) => {
    applyTheme(next); // live preview
    if (pending.current) window.clearTimeout(pending.current.timer);
    pending.current = { theme: next, timer: debounce ? window.setTimeout(flush, 300) : 0 };
    if (!debounce) flush();
  };

  const customColor = draft ?? (isHex(theme.customAccent) ? theme.customAccent : null);
  const swatch = (name: AccentName, color: string | null, icon?: React.ReactNode) => {
    const on = theme.accent === name;
    return (
      <button
        key={name}
        type="button"
        title={ACCENT_LABELS[name]}
        aria-label={ACCENT_LABELS[name]}
        aria-pressed={on}
        onClick={() => {
          if (name !== "custom") return change({ ...theme, accent: name });
          // Start a new custom color from the accent in use.
          change({ ...theme, accent: "custom", customAccent: customColor ?? baseScale(theme)["500"] });
        }}
        style={{
          background: color ?? "conic-gradient(#e05a7a, #e0a83a, #4fa36a, #4a8ad9, #9170de, #e05a7a)",
          color: color ? onAccent(color) : "#ffffff",
        }}
        className={`grid h-8 w-8 place-items-center rounded-full transition-shadow ${
          on ? "ring-2 ring-ink-100 ring-offset-2 ring-offset-ink-900" : "hover:ring-2 hover:ring-ink-600 hover:ring-offset-2 hover:ring-offset-ink-900"
        }`}
      >
        {on ? <Check size={16} strokeWidth={3} /> : icon}
      </button>
    );
  };

  return (
    <Card className="flex flex-col gap-5">
      <div className="flex items-center justify-between gap-4">
        <span>
          <span className="block text-sm font-semibold">Appearance</span>
          <span className="block text-xs text-ink-400">Light or dark, and the color of buttons and highlights.</span>
        </span>
        <Segmented<ThemeMode>
          value={theme.mode}
          options={[
            { value: "dark", label: "Dark" },
            { value: "light", label: "Light" },
          ]}
          onChange={(mode) => mode !== theme.mode && change({ ...theme, mode })}
        />
      </div>
      <div className="flex flex-wrap items-center justify-between gap-4">
        <span className="text-sm">Accent color</span>
        <div className="flex flex-wrap items-center gap-3">
          {ACCENT_NAMES.map((name) => swatch(name, ACCENTS[name]["500"]))}
          {swatch("custom", theme.accent === "custom" ? customColor : null, <Palette size={14} />)}
          {theme.accent === "custom" && (
            <label className="flex items-center gap-2 text-xs text-ink-400">
              <input
                type="color"
                aria-label="Custom accent color"
                value={customColor ?? "#d97757"}
                onChange={(e) => {
                  const hex = e.target.value.toLowerCase();
                  setDraft(hex);
                  change({ ...theme, accent: "custom", customAccent: hex }, true);
                }}
                className="h-8 w-10 cursor-pointer rounded border border-ink-700 bg-ink-850 p-0.5"
              />
              <span className="tabular font-mono">{customColor}</span>
            </label>
          )}
        </div>
      </div>
    </Card>
  );
}
