import { useState } from "react";
import { Play, Save } from "lucide-react";
import { api, errorText } from "../api";
import { ConfigEditor } from "../components/ConfigEditor";
import { Button, ErrorText, Modal } from "../components/ui";
import { blankConfig, describe, newId } from "../format";
import type { Config, SessionConfig, Snapshot } from "../types";

export function StartPage({ snapshot, onConfig }: { snapshot: Snapshot; onConfig: (c: Config) => void }) {
  const presets = snapshot.config.presets;
  const [config, setConfig] = useState<SessionConfig>(() => presets[0]?.config ?? blankConfig());
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [presetName, setPresetName] = useState("");

  const sitesNeedExtension = config.sites.mode !== "off" && snapshot.extensions.length === 0;

  const start = async () => {
    setError("");
    if (config.apps.mode === "allow" && config.apps.rules.length === 0) {
      setError("Add at least one app to allow, or every app you open will be closed.");
      return;
    }
    try {
      await api.startSession(config);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const savePreset = async () => {
    try {
      const name = presetName.trim();
      onConfig(await api.savePreset({ id: newId(), name, config: { ...config, label: name } }));
      setSaving(false);
      setPresetName("");
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div>
      <div className="mx-auto flex max-w-3xl flex-col gap-6 pb-6">
      <header className="text-center">
        <h1 className="text-4xl">Time to lock in</h1>
        <p className="mt-2 text-sm text-ink-400">Open what you need, choose what to block, and get it done.</p>
      </header>

      {presets.length > 0 && (
        <div className="flex flex-col items-center gap-2">
          <span className="text-xs font-medium tracking-wide text-ink-400 uppercase">Start from a preset</span>
          <div className="flex flex-wrap justify-center gap-2">
            {presets.map((p) => (
              <button
                key={p.id}
                onClick={() => setConfig({ ...p.config, label: p.config.label ?? p.name })}
                className="rounded-xl border border-ink-700 bg-ink-900 px-4 py-2.5 text-left hover:border-accent-500"
              >
                <span className="block text-sm font-medium">{p.name}</span>
                <span className="block text-xs text-ink-400">{describe(p.config)}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <ConfigEditor value={config} onChange={setConfig} />

      {sitesNeedExtension && (
        <p className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-200">
          No browser extension is connected yet, so websites won't be blocked. Set it up in Settings → Browser extension.
        </p>
      )}
      <ErrorText>{error}</ErrorText>
      </div>

      {/* Full-width bar pinned to the bottom of the window until the page ends. */}
      <div className="sticky bottom-0 -mx-8 border-t border-ink-800 bg-ink-950 px-8 py-4">
        <div className="mx-auto flex max-w-3xl items-center justify-between gap-3">
          <Button variant="ghost" onClick={() => setSaving(true)}>
            <Save size={16} /> Save as preset
          </Button>
          <div className="flex min-w-0 items-center gap-4">
            <span className="hidden truncate text-xs text-ink-400 md:inline">{describe(config)}</span>
            <Button variant="primary" onClick={start} className="px-6 py-2.5 text-base">
              <Play size={18} /> Lock in
            </Button>
          </div>
        </div>
      </div>

      {saving && (
        <Modal title="Save as preset" onClose={() => setSaving(false)}>
          <form
            className="flex flex-col gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              void savePreset();
            }}
          >
            <input
              autoFocus
              value={presetName}
              onChange={(e) => setPresetName(e.target.value)}
              placeholder="e.g. Coding, Study, Essay"
              className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 text-sm outline-none focus:border-accent-500"
            />
            <div className="flex justify-end">
              <Button variant="primary" type="submit" disabled={!presetName.trim()}>
                Save
              </Button>
            </div>
          </form>
        </Modal>
      )}
    </div>
  );
}
