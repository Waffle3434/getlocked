import { useState } from "react";
import { Copy, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { api, errorText } from "../api";
import { ConfigEditor } from "../components/ConfigEditor";
import { Button, ErrorText, Modal } from "../components/ui";
import { blankConfig, describe, newId } from "../format";
import type { Config, Preset } from "../types";

export function PresetsPage({ config, onConfig, running }: { config: Config; onConfig: (c: Config) => void; running: boolean }) {
  const [editing, setEditing] = useState<Preset | null>(null);
  const [error, setError] = useState("");

  const run = async (fn: () => Promise<Config | void>) => {
    setError("");
    try {
      const c = await fn();
      if (c) onConfig(c);
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  };

  const save = async () => {
    if (!editing) return;
    const preset = { ...editing, config: { ...editing.config, label: editing.name.trim() } };
    if (await run(() => api.savePreset(preset))) setEditing(null);
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <header className="flex items-end justify-between gap-6">
        <div>
          <h1 className="text-3xl">Presets</h1>
          <p className="mt-1 text-sm text-ink-400">Saved setups you can start in one click or put on a schedule.</p>
        </div>
        <Button variant="primary" onClick={() => setEditing({ id: newId(), name: "", config: blankConfig() })}>
          <Plus size={16} /> New preset
        </Button>
      </header>
      <ErrorText>{error}</ErrorText>

      {config.presets.length === 0 ? (
        <p className="text-sm text-ink-400">No presets yet.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {config.presets.map((p) => {
            const scheduled = config.schedules.filter((s) => s.presetId === p.id).length;
            return (
              <li key={p.id} className="flex items-center gap-3 rounded-xl border border-ink-800 bg-ink-900 p-4">
                <div className="min-w-0 flex-1">
                  <div className="font-medium">{p.name}</div>
                  <div className="truncate text-xs text-ink-400">
                    {describe(p.config)}
                    {scheduled > 0 && ` · ${scheduled} schedule${scheduled === 1 ? "" : "s"}`}
                  </div>
                </div>
                <Button variant="ghost" title="Edit" onClick={() => setEditing(structuredClone(p))}>
                  <Pencil size={16} />
                </Button>
                <Button
                  variant="ghost"
                  title="Duplicate"
                  onClick={() => run(() => api.savePreset({ ...structuredClone(p), id: newId(), name: `${p.name} copy` }))}
                >
                  <Copy size={16} />
                </Button>
                <Button
                  variant="ghost"
                  title="Delete"
                  onClick={() => {
                    if (confirm(`Delete "${p.name}"? Its schedules will be removed too.`)) void run(() => api.deletePreset(p.id));
                  }}
                >
                  <Trash2 size={16} />
                </Button>
                <Button variant="primary" disabled={running} onClick={() => run(() => api.startPreset(p.id))}>
                  <Play size={16} /> Start
                </Button>
              </li>
            );
          })}
        </ul>
      )}

      {editing && (
        <Modal title={config.presets.some((p) => p.id === editing.id) ? "Edit preset" : "New preset"} onClose={() => setEditing(null)} wide>
          <div className="flex flex-col gap-4">
            <input
              autoFocus
              value={editing.name}
              onChange={(e) => setEditing({ ...editing, name: e.target.value })}
              placeholder="Preset name"
              className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 text-sm outline-none focus:border-accent-500"
            />
            <ConfigEditor value={editing.config} onChange={(c) => setEditing({ ...editing, config: c })} />
            <ErrorText>{error}</ErrorText>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setEditing(null)}>
                Cancel
              </Button>
              <Button variant="primary" onClick={save} disabled={!editing.name.trim()}>
                Save preset
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
