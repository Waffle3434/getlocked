import { useState } from "react";
import { CheckCircle2, Puzzle, XCircle } from "lucide-react";
import { api, errorText } from "../api";
import { ExtensionSetup } from "../components/ExtensionSetup";
import { Button, Card, ErrorText, NumberField, SectionTitle, Toggle } from "../components/ui";
import type { Config, Settings, Snapshot } from "../types";

export function SettingsPage({ snapshot, onConfig }: { snapshot: Snapshot; onConfig: (c: Config) => void }) {
  const [error, setError] = useState("");
  const [settingUp, setSettingUp] = useState(false);
  const settings = snapshot.config.settings;

  const save = async (patch: Partial<Settings>) => {
    setError("");
    try {
      onConfig(await api.saveSettings({ ...settings, ...patch }));
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <header>
        <h1 className="text-3xl">Settings</h1>
      </header>
      <ErrorText>{error}</ErrorText>

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
          <p className="flex items-center gap-2 text-sm text-amber-300">
            <XCircle size={16} /> Not connected in any browser yet.
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {snapshot.extensions.map((e, i) => (
              <li key={i} className="flex items-center gap-2 text-sm">
                <CheckCircle2 size={16} className="text-emerald-400" />
                <span className="capitalize">{e.browser}</span>
                <span className="text-ink-400">v{e.version}</span>
                {!e.incognitoAllowed && (
                  <span className="text-xs text-amber-300">· not allowed in private windows (Hard mode needs this)</span>
                )}
              </li>
            ))}
          </ul>
        )}
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

      {settingUp && <ExtensionSetup extensions={snapshot.extensions} onClose={() => setSettingUp(false)} />}
    </div>
  );
}
