import { useEffect, useState } from "react";
import { Check, Pencil, Trash2 } from "lucide-react";
import { api, errorText } from "../api";
import { wordCount } from "../format";
import type { NoteCheck, Task } from "../types";
import { Button, Card, ErrorText, Modal } from "./ui";

// Same limits as crates/core/src/tasks.rs.
const MAX_TASKS = 20;
const MAX_TASK_LEN = 120;
const MAX_NOTE_LEN = 200;
const MIN_NOTE_WORDS = 3;
const MIN_REAL_WORDS = 2;
/** `check_note` is a cheap local call; this only skips calls mid-word. */
const CHECK_DEBOUNCE_MS = 150;

type Asking = { kind: "tick" | "drop"; task: Task } | null;

/**
 * "What did you do?" (tick) or "Why are you dropping it?" (drop), with live validation:
 * 3 words, 2 of them different real words. The core's `check_note` decides; until its
 * answer for the current text arrives, the local word count fills in and submit stays off.
 */
function NoteDialog({ asking, onClose }: { asking: NonNullable<Asking>; onClose: () => void }) {
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [check, setCheck] = useState<{ text: string; result: NoteCheck } | null>(null);
  const tick = asking.kind === "tick";

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .checkNote(text)
        .then((result) => !cancelled && setCheck({ text, result }))
        .catch(() => {}); // the submit still validates in the core
    }, CHECK_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [text]);

  const current = check?.text === text ? check.result : null;
  const words = current?.words ?? wordCount(text);
  const realWords = current?.realWords ?? 0;
  const ok = current?.ok ?? false;

  const submit = async () => {
    setError("");
    setBusy(true);
    try {
      if (tick) await api.tickTask(asking.task.id, text);
      else await api.dropTask(asking.task.id, text);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal title={tick ? "What did you do?" : "Why are you dropping it?"} onClose={onClose}>
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (ok && !busy) void submit();
        }}
      >
        <p className="text-sm text-ink-300">“{asking.task.text}”</p>
        <input
          autoFocus
          value={text}
          maxLength={MAX_NOTE_LEN}
          onChange={(e) => setText(e.target.value)}
          placeholder={tick ? "e.g. wrote the intro section" : "e.g. moved it to tomorrow"}
          className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 text-sm outline-none focus:border-accent-500"
        />
        <p className={`text-xs ${ok ? "text-ok" : "text-ink-400"}`}>
          {Math.min(words, MIN_NOTE_WORDS)} of {MIN_NOTE_WORDS} words · {Math.min(realWords, MIN_REAL_WORDS)} of{" "}
          {MIN_REAL_WORDS} real
        </p>
        <ErrorText>{error}</ErrorText>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant={tick ? "primary" : "danger"} disabled={!ok || busy}>
            {tick ? "Tick it off" : "Drop task"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

/** The session's tasks: tick (with a note), untick, edit, drop (with a reason), add. */
export function TaskChecklist({ tasks }: { tasks: Task[] }) {
  const [asking, setAsking] = useState<Asking>(null);
  const [editing, setEditing] = useState<{ id: number; text: string } | null>(null);
  const [adding, setAdding] = useState("");
  const [error, setError] = useState("");

  const run = (p: Promise<unknown>) => {
    setError("");
    p.catch((e) => setError(errorText(e)));
  };

  const saveEdit = (t: Task) => {
    if (!editing) return;
    const text = editing.text.trim();
    setEditing(null);
    if (text && text !== t.text) run(api.editTask(t.id, text));
  };

  const add = async () => {
    const text = adding.trim();
    if (!text) return;
    setError("");
    try {
      await api.addTask(text);
      setAdding("");
    } catch (e) {
      setError(errorText(e));
    }
  };

  const live = tasks.filter((t) => t.state.type !== "dropped");
  const done = live.filter((t) => t.state.type === "done").length;
  const full = tasks.length >= MAX_TASKS; // dropped tasks count toward the cap

  return (
    <Card className="w-full">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold">Tasks</h3>
        {live.length > 0 && (
          <span className={`text-xs ${done === live.length ? "text-ok" : "text-ink-400"}`}>
            {done} of {live.length} done
          </span>
        )}
      </div>
      {live.length > 0 && (
        <ul className="flex flex-col gap-1">
          {live.map((t) => {
            const isDone = t.state.type === "done";
            return (
              <li key={t.id} className="group flex items-start gap-3 rounded-lg px-2 py-1.5 hover:bg-ink-850">
                <button
                  type="button"
                  aria-label={isDone ? "Untick" : "Tick"}
                  title={isDone ? "Untick (clears the note)" : "Tick it off"}
                  onClick={() => (isDone ? run(api.untickTask(t.id)) : setAsking({ kind: "tick", task: t }))}
                  className={`mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded border transition-colors ${
                    isDone ? "border-accent-400 bg-accent-600 text-on-accent" : "border-ink-600 hover:border-accent-500"
                  }`}
                >
                  {isDone && <Check size={14} />}
                </button>
                <div className="min-w-0 flex-1">
                  {editing?.id === t.id ? (
                    <form
                      onSubmit={(e) => {
                        e.preventDefault();
                        saveEdit(t);
                      }}
                    >
                      <input
                        autoFocus
                        value={editing.text}
                        maxLength={MAX_TASK_LEN}
                        onChange={(e) => setEditing({ id: t.id, text: e.target.value })}
                        onKeyDown={(e) => e.key === "Escape" && setEditing(null)}
                        onBlur={() => setEditing(null)}
                        className="w-full rounded border border-ink-700 bg-ink-850 px-2 py-0.5 text-sm outline-none focus:border-accent-500"
                      />
                    </form>
                  ) : (
                    <div className={`text-sm break-words ${isDone ? "text-ink-400 line-through" : ""}`}>{t.text}</div>
                  )}
                  {t.state.type === "done" && <div className="text-xs break-words text-ink-400">{t.state.note}</div>}
                </div>
                <button
                  type="button"
                  aria-label="Edit"
                  title={isDone ? "Edit (unticks it)" : "Edit"}
                  onClick={() => setEditing({ id: t.id, text: t.text })}
                  className="mt-0.5 text-ink-400 opacity-0 group-hover:opacity-100 hover:text-ink-100 focus-visible:opacity-100"
                >
                  <Pencil size={14} />
                </button>
                <button
                  type="button"
                  aria-label="Drop"
                  title="Drop task"
                  onClick={() => setAsking({ kind: "drop", task: t })}
                  className="mt-0.5 text-ink-400 opacity-0 group-hover:opacity-100 hover:text-danger focus-visible:opacity-100"
                >
                  <Trash2 size={14} />
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <form
        className={live.length > 0 ? "mt-3" : ""}
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          value={adding}
          maxLength={MAX_TASK_LEN}
          disabled={full}
          onChange={(e) => setAdding(e.target.value)}
          placeholder={full ? `This session has ${MAX_TASKS} tasks, the most it can have` : "+ Add a task"}
          className="w-full rounded-lg border border-ink-800 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-accent-500 disabled:opacity-50"
        />
      </form>
      {error && (
        <div className="mt-3">
          <ErrorText>{error}</ErrorText>
        </div>
      )}
      {asking && <NoteDialog asking={asking} onClose={() => setAsking(null)} />}
    </Card>
  );
}
