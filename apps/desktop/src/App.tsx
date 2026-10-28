import { useEffect, useState } from "react";
import { BarChart3, CalendarClock, Layers, Lock, Play, Settings as SettingsIcon } from "lucide-react";
import { api, onQuitBlocked, onState } from "./api";
import { clock } from "./format";
import { PresetsPage } from "./pages/PresetsPage";
import { SchedulePage } from "./pages/SchedulePage";
import { SessionPage } from "./pages/SessionPage";
import { SettingsPage } from "./pages/SettingsPage";
import { StartPage } from "./pages/StartPage";
import { StatsPage } from "./pages/StatsPage";
import type { Config, Snapshot } from "./types";

type Page = "home" | "presets" | "schedule" | "stats" | "settings";

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    api.getSnapshot().then(setSnapshot);
    const unlisten = [onState(setSnapshot), onQuitBlocked(setNotice)];
    return () => unlisten.forEach((p) => p.then((f) => f()));
  }, []);

  // When a session starts (from anywhere, e.g. a schedule), jump to it.
  const sessionId = snapshot?.session?.id;
  useEffect(() => {
    if (sessionId) setPage("home");
  }, [sessionId]);

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(""), 5000);
    return () => clearTimeout(t);
  }, [notice]);

  if (!snapshot) return null;

  const onConfig = (config: Config) => setSnapshot((s) => (s ? { ...s, config } : s));
  const session = snapshot.session;

  const nav: { id: Page; label: string; icon: React.ReactNode }[] = [
    { id: "home", label: session ? "Session" : "Start", icon: session ? <Lock size={18} /> : <Play size={18} /> },
    { id: "presets", label: "Presets", icon: <Layers size={18} /> },
    { id: "schedule", label: "Schedule", icon: <CalendarClock size={18} /> },
    { id: "stats", label: "Stats", icon: <BarChart3 size={18} /> },
    { id: "settings", label: "Settings", icon: <SettingsIcon size={18} /> },
  ];

  return (
    <div className="flex h-full">
      <aside className="flex w-52 shrink-0 flex-col border-r border-ink-800 bg-ink-900/60 p-3">
        <div className="mb-6 flex items-center gap-2 px-2 pt-2">
          <img src="/favicon.svg" alt="" className="h-7 w-7" />
          <span className="text-lg font-semibold tracking-tight">GetLocked</span>
        </div>
        <nav className="flex flex-col gap-1">
          {nav.map((n) => (
            <button
              key={n.id}
              onClick={() => setPage(n.id)}
              className={`flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
                page === n.id ? "bg-ink-800 text-ink-100" : "text-ink-400 hover:bg-ink-850 hover:text-ink-100"
              }`}
            >
              {n.icon}
              {n.label}
            </button>
          ))}
        </nav>
        {session && page !== "home" && (
          <button
            onClick={() => setPage("home")}
            className="mt-auto rounded-xl border border-accent-600/40 bg-accent-600/10 p-3 text-left"
          >
            <div className="text-xs text-accent-300">{session.phase === "break" ? "On a break" : "Locked in"}</div>
            <div className="tabular text-xl font-semibold">
              {clock(
                session.phase === "break"
                  ? (session.phaseRemainingSecs ?? 0)
                  : (session.remainingSecs ?? session.focusSecs),
              )}
            </div>
          </button>
        )}
      </aside>

      <main className="flex-1 overflow-y-auto px-8 py-8">
        {page === "home" &&
          (session ? (
            <SessionPage session={session} snapshot={snapshot} />
          ) : (
            <StartPage snapshot={snapshot} onConfig={onConfig} />
          ))}
        {page === "presets" && <PresetsPage config={snapshot.config} onConfig={onConfig} running={!!session} />}
        {page === "schedule" && <SchedulePage config={snapshot.config} onConfig={onConfig} />}
        {page === "stats" && <StatsPage />}
        {page === "settings" && <SettingsPage snapshot={snapshot} onConfig={onConfig} />}
      </main>

      {notice && (
        <div className="fixed right-6 bottom-6 z-50 rounded-xl border border-amber-500/30 bg-ink-850 px-4 py-3 text-sm text-amber-200 shadow-2xl">
          {notice}
        </div>
      )}
    </div>
  );
}
