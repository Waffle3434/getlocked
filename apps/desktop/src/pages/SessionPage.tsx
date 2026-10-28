import { useEffect, useRef, useState } from "react";
import { Coffee, Lock, ShieldCheck } from "lucide-react";
import { api, errorText } from "../api";
import { Button, Chip, ErrorText, Modal } from "../components/ui";
import { clock, duration, endButton, ruleKey, ruleLabel } from "../format";
import type { SessionView, Snapshot, UnlockStatus } from "../types";

/** Progress ring around the big timer. */
function Ring({ progress, children, dim }: { progress: number | null; children: React.ReactNode; dim: boolean }) {
  const r = 140;
  const c = 2 * Math.PI * r;
  return (
    <div className="relative grid h-80 w-80 place-items-center">
      <svg viewBox="0 0 320 320" className="absolute inset-0 -rotate-90">
        <circle cx="160" cy="160" r={r} fill="none" stroke="var(--color-ink-800)" strokeWidth="10" />
        {progress !== null && (
          <circle
            cx="160"
            cy="160"
            r={r}
            fill="none"
            stroke={dim ? "var(--color-ink-600)" : "var(--color-accent-500)"}
            strokeWidth="10"
            strokeLinecap="round"
            strokeDasharray={c}
            strokeDashoffset={c * (1 - Math.min(1, Math.max(0, progress)))}
            style={{ transition: "stroke-dashoffset 0.5s linear" }}
          />
        )}
      </svg>
      <div className="relative flex flex-col items-center">{children}</div>
    </div>
  );
}

function UnlockDialog({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<UnlockStatus>({ type: "none" });
  const [typed, setTyped] = useState("");
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.beginUnlock().then(setStatus).catch((e) => setError(errorText(e)));
    const t = setInterval(() => api.unlockStatus().then(setStatus).catch(() => {}), 500);
    return () => clearInterval(t);
  }, []);

  const ready = status.type === "ready";
  useEffect(() => {
    if (ready) inputRef.current?.focus();
  }, [ready]);

  const cancel = () => {
    void api.cancelUnlock();
    onClose();
  };

  const submit = async () => {
    try {
      await api.endSession(typed);
    } catch (e) {
      setError(errorText(e));
      setTyped("");
    }
  };

  const block = (e: React.SyntheticEvent) => e.preventDefault();

  return (
    <Modal title="End early?" onClose={cancel}>
      <div className="flex flex-col gap-4">
        <p className="text-sm text-ink-300">
          If the work's done, great. If you're just tempted, close this and keep going.
        </p>
        {status.type === "coolingDown" && (
          <div className="rounded-xl bg-ink-850 py-8 text-center">
            <div className="tabular text-4xl font-semibold">{status.secsLeft}</div>
            <div className="mt-1 text-xs text-ink-400">seconds until your unlock code appears</div>
          </div>
        )}
        {status.type === "ready" && (
          <>
            <div
              className="rounded-xl bg-ink-850 px-4 py-5 text-center font-mono text-lg tracking-wider break-all text-accent-300 select-none"
              onCopy={block}
              onContextMenu={block}
              onDragStart={block}
            >
              {status.code}
            </div>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void submit();
              }}
              className="flex flex-col gap-3"
            >
              <input
                ref={inputRef}
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                onPaste={block}
                onDrop={block}
                autoComplete="off"
                spellCheck={false}
                placeholder="Type the code exactly (no pasting)"
                className="rounded-lg border border-ink-700 bg-ink-850 px-3 py-2 font-mono text-sm outline-none focus:border-accent-500"
              />
              <ErrorText>{error}</ErrorText>
              <div className="flex justify-between">
                <Button type="button" variant="ghost" onClick={cancel}>
                  Keep going
                </Button>
                <Button type="submit" variant="danger" disabled={!typed.trim()}>
                  End session
                </Button>
              </div>
            </form>
          </>
        )}
        {status.type !== "ready" && <ErrorText>{error}</ErrorText>}
      </div>
    </Modal>
  );
}

function TimeUpDialog({ session }: { session: SessionView }) {
  const [error, setError] = useState("");
  const choose = (keepGoing: boolean) => api.timeUpChoice(keepGoing).catch((e) => setError(errorText(e)));
  return (
    <Modal title="Time's up!">
      <div className="flex flex-col gap-4">
        <p className="text-sm text-ink-300">
          You focused for <strong className="text-ink-100">{duration(session.focusSecs)}</strong>. Is the work done?
          Blocks stay on until you choose.
        </p>
        <ErrorText>{error}</ErrorText>
        <div className="flex flex-col gap-2 sm:flex-row sm:justify-end">
          <Button onClick={() => choose(true)}>Keep going (stopwatch)</Button>
          <Button variant="primary" onClick={() => choose(false)}>
            I'm done
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export function SessionPage({ session, snapshot }: { session: SessionView; snapshot: Snapshot }) {
  const [unlocking, setUnlocking] = useState(false);
  const [error, setError] = useState("");
  const c = session.config;
  const end = endButton(session.endPolicy, session.focusSecs);

  const onBreak = session.phase === "break";
  const big = onBreak
    ? clock(session.phaseRemainingSecs ?? 0)
    : session.remainingSecs !== null
      ? clock(session.remainingSecs)
      : clock(session.focusSecs);
  const caption = onBreak
    ? "break left"
    : session.phase === "timeUp"
      ? "time's up"
      : session.remainingSecs !== null
        ? "left"
        : session.overtime
          ? `focused (${duration(session.overtimeSecs)} extra)`
          : "focused";
  const progress =
    c.kind.type === "timed" && !session.overtime
      ? session.focusSecs / c.kind.durationSecs
      : onBreak && c.pomodoro
        ? 1 - (session.phaseRemainingSecs ?? 0) / c.pomodoro.breakSecs
        : null;

  const onEnd = async () => {
    setError("");
    if (session.endPolicy.type === "needsCode") {
      setUnlocking(true);
      return;
    }
    try {
      await api.endSession(null);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const sitesUnprotected = c.sites.mode !== "off" && snapshot.extensions.length === 0;

  return (
    <div className="mx-auto flex max-w-3xl flex-col items-center gap-8 py-4">
      <div className="text-center">
        <div className="mb-2 inline-flex items-center gap-2 rounded-full border border-ink-700 bg-ink-900 px-3 py-1 text-xs text-ink-300">
          {onBreak ? <Coffee size={14} className="text-emerald-400" /> : <Lock size={14} className="text-accent-400" />}
          {onBreak ? "On a break: blocks are lifted" : "Locked in"} · {c.strictness[0].toUpperCase() + c.strictness.slice(1)}
        </div>
        <h1 className="text-2xl">{session.label ?? (c.kind.type === "timed" ? "Focus session" : "Stopwatch session")}</h1>
      </div>

      <Ring progress={progress} dim={onBreak}>
        <span className="tabular text-6xl font-semibold tracking-tight">{big}</span>
        <span className="mt-1 text-sm text-ink-400">{caption}</span>
        {c.pomodoro && !onBreak && session.phaseRemainingSecs !== null && session.phase === "focus" && (
          <span className="mt-3 text-xs text-ink-400">Break in {clock(session.phaseRemainingSecs)}</span>
        )}
      </Ring>

      <div className="flex flex-col items-center gap-2">
        <Button variant={end.disabled ? "secondary" : "danger"} onClick={onEnd} disabled={end.disabled}>
          {end.disabled && <Lock size={14} />}
          {end.label}
        </Button>
        <ErrorText>{error}</ErrorText>
      </div>

      <div className="grid w-full gap-3 sm:grid-cols-3">
        <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
          <div className="text-xs text-ink-400">Focus time</div>
          <div className="tabular mt-1 text-xl font-semibold">{duration(session.focusSecs)}</div>
        </div>
        <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
          <div className="text-xs text-ink-400">Distractions blocked</div>
          <div className="tabular mt-1 text-xl font-semibold">{session.blockedCount}</div>
        </div>
        <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
          <div className="text-xs text-ink-400">Started</div>
          <div className="mt-1 text-xl font-semibold">
            {new Date(session.startedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
          </div>
        </div>
      </div>

      <div className="grid w-full gap-3 sm:grid-cols-2">
        {c.apps.mode !== "off" && (
          <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
            <div className="mb-2 text-xs text-ink-400">{c.apps.mode === "block" ? "Blocked apps" : "Allowed apps (only these)"}</div>
            <div className="flex flex-wrap gap-1.5">
              {c.apps.rules.length ? c.apps.rules.map((r) => <Chip key={ruleKey(r)}>{ruleLabel(r)}</Chip>) : <span className="text-xs text-ink-400">None</span>}
            </div>
          </div>
        )}
        {c.sites.mode !== "off" && (
          <div className="rounded-xl border border-ink-800 bg-ink-900 p-4">
            <div className="mb-2 text-xs text-ink-400">{c.sites.mode === "block" ? "Blocked sites" : "Allowed sites (only these)"}</div>
            <div className="flex flex-wrap gap-1.5">
              {c.sites.domains.length ? c.sites.domains.map((d) => <Chip key={d}>{d}</Chip>) : <span className="text-xs text-ink-400">None</span>}
            </div>
            <div className={`mt-3 flex items-center gap-1.5 text-xs ${sitesUnprotected ? "text-amber-300" : "text-emerald-300"}`}>
              <ShieldCheck size={14} />
              {sitesUnprotected
                ? "No browser extension connected: sites aren't being blocked."
                : `Extension connected in ${snapshot.extensions.map((e) => e.browser).join(", ")}`}
            </div>
          </div>
        )}
      </div>

      {unlocking && <UnlockDialog onClose={() => setUnlocking(false)} />}
      {session.phase === "timeUp" && <TimeUpDialog session={session} />}
    </div>
  );
}
