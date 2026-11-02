import { useState } from "react";
import { Card, Chip, SectionTitle } from "./ui";

// Same limits as crates/core/src/tasks.rs.
const MAX_TASKS = 20;
const MAX_LEN = 120;

/** The Start page's "What are you getting done?" list. Enter adds, × removes. */
export function TaskInput({ tasks, onChange }: { tasks: string[]; onChange: (t: string[]) => void }) {
  const [text, setText] = useState("");
  const full = tasks.length >= MAX_TASKS;

  const add = () => {
    const t = text.trim();
    if (!t || t.length > MAX_LEN || full) return;
    onChange([...tasks, t]);
    setText("");
  };

  return (
    <Card>
      <SectionTitle hint="Optional. Tick every task (with a short note) and you can end the session at any strictness.">
        What are you getting done?
      </SectionTitle>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          value={text}
          maxLength={MAX_LEN}
          disabled={full}
          onChange={(e) => setText(e.target.value)}
          placeholder={full ? `That's ${MAX_TASKS} tasks, the most a session can have` : "e.g. Write the essay intro, then press Enter"}
          className="w-full rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-sm outline-none focus:border-accent-500 disabled:opacity-50"
        />
      </form>
      {tasks.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {tasks.map((t, i) => (
            <Chip key={`${i}:${t}`} onRemove={() => onChange(tasks.filter((_, j) => j !== i))}>
              {t}
            </Chip>
          ))}
        </div>
      )}
    </Card>
  );
}
