import { useState } from "react";
import { CalendarClock, Plus, Trash2 } from "lucide-react";
import { api, errorText } from "../api";
import { Button, ErrorText, Modal, Toggle } from "../components/ui";
import { DAY_NAMES, minuteToTime, newId, timeToMinute } from "../format";
import type { Config, Schedule } from "../types";

export function SchedulePage({ config, onConfig }: { config: Config; onConfig: (c: Config) => void }) {
  const [editing, setEditing] = useState<Schedule | null>(null);
  const [error, setError] = useState("");
  const presetName = (id: string) => config.presets.find((p) => p.id === id)?.name ?? "Missing preset";

  const save = async (s: Schedule, close = false) => {
    setError("");
    try {
      onConfig(await api.saveSchedule(s));
      if (close) setEditing(null);
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Schedule</h1>
          <p className="mt-1 text-sm text-ink-400">
            Start a preset automatically at set times. GetLocked needs to be running (it lives in the tray), so turn on
            Start with Windows in Settings.
          </p>
        </div>
        <Button
          variant="primary"
          disabled={config.presets.length === 0}
          onClick={() =>
            setEditing({ id: newId(), presetId: config.presets[0]?.id ?? "", days: [0, 1, 2, 3, 4], startMinute: 9 * 60, enabled: true })
          }
        >
          <Plus size={16} /> Add schedule
        </Button>
      </header>
      <ErrorText>{error}</ErrorText>
      {config.presets.length === 0 && <p className="text-sm text-ink-400">Create a preset first, then schedule it here.</p>}

      <ul className="flex flex-col gap-2">
        {config.schedules.map((s) => (
          <li key={s.id} className="flex items-center gap-4 rounded-xl border border-ink-800 bg-ink-900 p-4">
            <CalendarClock className={s.enabled ? "text-accent-400" : "text-ink-600"} />
            <button className="min-w-0 flex-1 text-left" onClick={() => setEditing(structuredClone(s))}>
              <div className="font-medium">
                {minuteToTime(s.startMinute)} · {presetName(s.presetId)}
              </div>
              <div className="text-xs text-ink-400">{s.days.map((d) => DAY_NAMES[d]).join(", ")}</div>
            </button>
            <Toggle checked={s.enabled} onChange={(enabled) => save({ ...s, enabled })} label="" />
            <Button
              variant="ghost"
              title="Delete"
              onClick={async () => {
                try {
                  onConfig(await api.deleteSchedule(s.id));
                } catch (e) {
                  setError(errorText(e));
                }
              }}
            >
              <Trash2 size={16} />
            </Button>
          </li>
        ))}
      </ul>

      {editing && (
        <Modal title="Schedule" onClose={() => setEditing(null)}>
          <div className="flex flex-col gap-4">
            <label className="flex flex-col gap-1 text-sm">
              Preset
              <select
                value={editing.presetId}
                onChange={(e) => setEditing({ ...editing, presetId: e.target.value })}
                className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 outline-none focus:border-accent-500"
              >
                {config.presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-sm">
              Start time
              <input
                type="time"
                value={minuteToTime(editing.startMinute)}
                onChange={(e) => setEditing({ ...editing, startMinute: timeToMinute(e.target.value) })}
                className="w-40 rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 outline-none [color-scheme:dark] focus:border-accent-500"
              />
            </label>
            <div className="flex flex-col gap-1 text-sm">
              Days
              <div className="flex flex-wrap gap-1.5">
                {DAY_NAMES.map((name, d) => {
                  const on = editing.days.includes(d);
                  return (
                    <button
                      key={name}
                      type="button"
                      onClick={() =>
                        setEditing({
                          ...editing,
                          days: on ? editing.days.filter((x) => x !== d) : [...editing.days, d].sort(),
                        })
                      }
                      className={`w-12 rounded-lg border py-1.5 text-xs ${on ? "border-accent-500 bg-accent-600/20 text-accent-300" : "border-ink-700 text-ink-400"}`}
                    >
                      {name}
                    </button>
                  );
                })}
              </div>
            </div>
            <ErrorText>{error}</ErrorText>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setEditing(null)}>
                Cancel
              </Button>
              <Button variant="primary" onClick={() => save(editing, true)}>
                Save
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
