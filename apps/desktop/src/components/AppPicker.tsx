import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FileCode2, Search } from "lucide-react";
import { api, errorText } from "../api";
import { ruleKey } from "../format";
import type { AppRule, InstalledApp, RunningApp } from "../types";
import { Button, ErrorText, Modal, Segmented } from "./ui";

type Tab = "running" | "installed" | "browse";

interface Entry {
  key: string;
  title: string;
  subtitle: string;
  rule: AppRule;
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Pick apps from what's running, the Start Menu, or the file system. */
export function AppPicker({
  rules,
  onChange,
  onClose,
}: {
  rules: AppRule[];
  onChange: (rules: AppRule[]) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<Tab>("running");
  const [running, setRunning] = useState<RunningApp[] | null>(null);
  const [installed, setInstalled] = useState<InstalledApp[] | null>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api.listRunningApps().then(setRunning).catch((e) => setError(errorText(e)));
  }, []);

  useEffect(() => {
    if (tab === "installed" && installed === null) {
      api.listInstalledApps().then(setInstalled).catch((e) => setError(errorText(e)));
    }
  }, [tab, installed]);

  const selected = useMemo(() => new Set(rules.map(ruleKey)), [rules]);

  const toggle = (rule: AppRule) => {
    const key = ruleKey(rule);
    onChange(selected.has(key) ? rules.filter((r) => ruleKey(r) !== key) : [...rules, rule]);
  };

  const entries: Entry[] = useMemo(() => {
    const list: Entry[] =
      tab === "running"
        ? (running ?? []).map((a) => ({
            key: a.exeName,
            title: a.exeName,
            subtitle: a.path ?? "",
            rule: { type: "exe", name: a.exeName },
          }))
        : tab === "installed"
          ? (installed ?? []).map((a) => ({
              key: a.path,
              title: a.name,
              subtitle: a.exeName,
              rule: { type: "exe", name: a.exeName },
            }))
          : [];
    const q = query.trim().toLowerCase();
    return q ? list.filter((e) => `${e.title} ${e.subtitle}`.toLowerCase().includes(q)) : list;
  }, [tab, running, installed, query]);

  const browseExe = async () => {
    const path = await open({ multiple: false, filters: [{ name: "Programs", extensions: ["exe"] }] });
    if (typeof path === "string") toggle({ type: "exe", name: baseName(path) });
  };

  const browseFolder = async () => {
    const path = await open({ directory: true, multiple: false });
    if (typeof path === "string") toggle({ type: "folder", path });
  };

  const loading = (tab === "running" && running === null) || (tab === "installed" && installed === null);

  return (
    <Modal title="Choose apps" onClose={onClose} wide>
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <Segmented<Tab>
            value={tab}
            onChange={setTab}
            options={[
              { value: "running", label: "Running now" },
              { value: "installed", label: "Installed" },
              { value: "browse", label: "Browse" },
            ]}
          />
          {tab !== "browse" && (
            <label className="flex items-center gap-2 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5">
              <Search size={14} className="text-ink-400" />
              <input
                autoFocus
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search"
                className="w-48 bg-transparent text-sm outline-none placeholder:text-ink-400"
              />
            </label>
          )}
        </div>
        <ErrorText>{error}</ErrorText>

        {tab === "browse" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <button
              onClick={browseExe}
              className="rounded-xl border border-ink-700 bg-ink-850 p-4 text-left hover:border-accent-500"
            >
              <FileCode2 className="mb-2 text-accent-400" />
              <div className="text-sm font-medium">Choose a program</div>
              <div className="mt-1 text-xs text-ink-400">Pick any .exe. It's matched by file name wherever it runs from.</div>
            </button>
            <button
              onClick={browseFolder}
              className="rounded-xl border border-ink-700 bg-ink-850 p-4 text-left hover:border-accent-500"
            >
              <FolderOpen className="mb-2 text-accent-400" />
              <div className="text-sm font-medium">Choose a folder</div>
              <div className="mt-1 text-xs text-ink-400">
                Everything that runs from inside it, e.g. your <code>steamapps\common</code> folder to cover every Steam
                game.
              </div>
            </button>
          </div>
        ) : loading ? (
          <p className="py-8 text-center text-sm text-ink-400">Looking for apps…</p>
        ) : entries.length === 0 ? (
          <p className="py-8 text-center text-sm text-ink-400">Nothing found.</p>
        ) : (
          <ul className="grid max-h-[50vh] gap-1 overflow-y-auto pr-1">
            {entries.map((e) => {
              const on = selected.has(ruleKey(e.rule));
              return (
                <li key={e.key}>
                  <button
                    onClick={() => toggle(e.rule)}
                    className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left ${on ? "bg-accent-600/20" : "hover:bg-ink-800"}`}
                  >
                    <span
                      className={`grid h-5 w-5 shrink-0 place-items-center rounded border text-xs ${on ? "border-accent-400 bg-accent-600 text-on-accent" : "border-ink-600"}`}
                    >
                      {on && "✓"}
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate text-sm">{e.title}</span>
                      <span className="block truncate text-xs text-ink-400">{e.subtitle}</span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        <div className="flex justify-end">
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        </div>
      </div>
    </Modal>
  );
}
