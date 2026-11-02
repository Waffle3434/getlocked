import { Fragment, useEffect, useState } from "react";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { CheckCircle2, ChevronDown, CircleStop, Flame, Trophy } from "lucide-react";
import { api, errorText } from "../api";
import { Card, ErrorText, Segmented } from "../components/ui";
import { duration } from "../format";
import type { StatsSummary } from "../types";

function Tile({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
      <div className="flex items-center gap-1.5 text-xs text-ink-400">
        {icon}
        {label}
      </div>
      <div className="tabular mt-1 text-2xl font-semibold">{value}</div>
    </div>
  );
}

function dayLabel(iso: string, long = false): string {
  const d = new Date(`${iso}T00:00:00`);
  return d.toLocaleDateString([], long ? { weekday: "long", month: "short", day: "numeric" } : { month: "numeric", day: "numeric" });
}

export function StatsPage() {
  const [days, setDays] = useState<"7" | "30">("7");
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    api.getStats(Number(days)).then(setStats).catch((e) => setError(errorText(e)));
  }, [days]);

  if (error) return <ErrorText>{error}</ErrorText>;
  if (!stats) return <p className="text-sm text-ink-400">Loading…</p>;

  const rangeFocus = stats.days.reduce((n, d) => n + d.focusSecs, 0);
  const chartData = stats.days.map((d) => ({ ...d, minutes: Math.round(d.focusSecs / 60) }));

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-3xl">Your stats</h1>
          <p className="mt-1 text-sm text-ink-400">Stored only on this PC.</p>
        </div>
        <Segmented
          value={days}
          onChange={setDays}
          options={[
            { value: "7", label: "7 days" },
            { value: "30", label: "30 days" },
          ]}
        />
      </header>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <Tile label={`Focus, last ${days} days`} value={duration(rangeFocus)} />
        <Tile label="Current streak" value={`${stats.currentStreak} day${stats.currentStreak === 1 ? "" : "s"}`} icon={<Flame size={14} className="text-streak" />} />
        <Tile label="Best streak" value={`${stats.bestStreak} day${stats.bestStreak === 1 ? "" : "s"}`} icon={<Trophy size={14} className="text-warn" />} />
        <Tile label="Completed" value={String(stats.completed)} icon={<CheckCircle2 size={14} className="text-ok-icon" />} />
        <Tile label="Ended early" value={String(stats.endedEarly)} icon={<CircleStop size={14} className="text-ink-400" />} />
      </div>

      <Card>
        <h3 className="mb-4 text-sm font-semibold">Focus minutes per day</h3>
        <div className="h-64">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={chartData} margin={{ top: 4, right: 4, bottom: 0, left: -12 }} barCategoryGap={days === "7" ? "35%" : "20%"}>
              <CartesianGrid vertical={false} stroke="var(--color-ink-800)" />
              <XAxis
                dataKey="date"
                tickFormatter={(d: string) => dayLabel(d)}
                tick={{ fill: "var(--color-ink-400)", fontSize: 11 }}
                axisLine={{ stroke: "var(--color-ink-700)" }}
                tickLine={false}
                interval={days === "7" ? 0 : "preserveStartEnd"}
              />
              <YAxis allowDecimals={false} tick={{ fill: "var(--color-ink-400)", fontSize: 11 }} axisLine={false} tickLine={false} />
              <Tooltip
                cursor={{ fill: "var(--color-ink-800)", opacity: 0.6 }}
                content={({ active, payload }) => {
                  const p = active && payload?.[0]?.payload;
                  if (!p) return null;
                  return (
                    <div className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 text-xs shadow-xl">
                      <div className="text-ink-300">{dayLabel(p.date, true)}</div>
                      <div className="mt-1 font-semibold text-ink-100">{duration(p.focusSecs)} focused</div>
                      <div className="text-ink-400">
                        {p.sessions} session{p.sessions === 1 ? "" : "s"}
                      </div>
                    </div>
                  );
                }}
              />
              <Bar dataKey="minutes" fill="var(--color-accent-500)" radius={[4, 4, 0, 0]} maxBarSize={36} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </Card>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <h3 className="mb-3 text-sm font-semibold">Most blocked</h3>
          {stats.topBlocked.length === 0 ? (
            <p className="text-sm text-ink-400">Nothing blocked yet in this range.</p>
          ) : (
            <ul className="flex flex-col gap-2">
              {stats.topBlocked.map((b) => (
                <li key={`${b.kind}:${b.target}`} className="flex items-center justify-between text-sm">
                  <span className="truncate">
                    <span className="mr-2 rounded bg-ink-800 px-1.5 py-0.5 text-[10px] text-ink-400 uppercase">{b.kind}</span>
                    {b.target}
                  </span>
                  <span className="tabular text-ink-300">{b.count}×</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
        <Card>
          <h3 className="mb-3 text-sm font-semibold">Recent sessions</h3>
          {stats.recent.length === 0 ? (
            <p className="text-sm text-ink-400">No sessions yet. Go lock in!</p>
          ) : (
            <table className="w-full text-sm">
              <tbody>
                {stats.recent.map((s) => (
                  <Fragment key={s.id}>
                    <tr
                      onClick={() => {
                        if (s.tasks.length) setOpen(open === s.id ? null : s.id);
                      }}
                      className={`border-t border-ink-800 first:border-0 ${s.tasks.length ? "cursor-pointer hover:bg-ink-850" : ""}`}
                    >
                      <td className="py-1.5 pr-2">
                        <div className="truncate">{s.label ?? (s.kind === "timed" ? "Timer" : "Stopwatch")}</div>
                        <div className="flex items-center gap-1 text-xs text-ink-400">
                          {new Date(s.startedAt).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}
                          {s.tasks.length > 0 && (
                            <>
                              {" · "}
                              {s.tasks.length} task{s.tasks.length === 1 ? "" : "s"}
                              <ChevronDown size={12} className={`transition-transform ${open === s.id ? "rotate-180" : ""}`} />
                            </>
                          )}
                        </div>
                      </td>
                      <td className="tabular py-1.5 pr-2 text-right">{duration(s.focusSecs)}</td>
                      <td className="py-1.5 text-right text-xs">
                        {s.outcome === "completed" ? (
                          <span className="text-ok">✓ Done</span>
                        ) : (
                          <span className="text-ink-400">Ended early</span>
                        )}
                      </td>
                    </tr>
                    {open === s.id && (
                      <tr>
                        <td colSpan={3} className="pb-2">
                          <ul className="flex flex-col gap-1 rounded-lg bg-ink-850 p-3 text-xs">
                            {s.tasks.map((t, i) => (
                              <li key={i} className="break-words">
                                <span className={t.state === "dropped" ? "text-ink-400 line-through" : ""}>{t.text}</span>
                                {t.state === "done" && <span className="text-ok"> ✓ {t.note}</span>}
                                {t.state === "dropped" && <span className="text-ink-400"> (dropped: {t.reason})</span>}
                                {t.state === "open" && <span className="text-ink-400"> (not done)</span>}
                              </li>
                            ))}
                          </ul>
                        </td>
                      </tr>
                    )}
                  </Fragment>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </div>
    </div>
  );
}
