import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api, onState } from "./api";
import { applyTheme } from "./theme";

/** Sent by src-tauri/src/popup.rs. `id` goes back in `popup_ready`. */
type PopupMessage = { id: number; title: string; body: string };

/**
 * The small "X is blocked" card GetLocked shows at the bottom-right of the
 * screen, in its own borderless window. Rust shows the window once the new
 * message has rendered, and hides it after a few seconds or on click.
 */
export default function Popup() {
  const [msg, setMsg] = useState<PopupMessage | null>(null);

  useEffect(() => {
    // The window is transparent; only the rounded card is drawn.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    const unlisten = [
      listen<PopupMessage>("popup", (e) => setMsg(e.payload)),
      // Follow the app's theme: once now, then on every change (the app
      // emits "state" when settings are saved).
      onState((s) => applyTheme(s.config.settings.theme)),
    ];
    api
      .getSnapshot()
      .then((s) => applyTheme(s.config.settings.theme))
      .catch(() => {});
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, []);

  useEffect(() => {
    // The new message is in the DOM now, so the window can appear.
    if (msg) invoke("popup_ready", { id: msg.id }).catch(() => {});
  }, [msg]);

  if (!msg) return null;

  return (
    <div
      role="alert"
      title="Click to dismiss"
      onClick={() => invoke("popup_dismiss").catch(() => {})}
      className="flex h-full cursor-pointer items-center gap-3 overflow-hidden rounded-xl border border-ink-700 bg-ink-900 px-4 py-3"
    >
      <img src="/favicon.svg" alt="" className="h-10 w-10 shrink-0" />
      <div className="min-w-0">
        <div className="truncate text-sm font-semibold text-ink-100">{msg.title}</div>
        <div className="mt-0.5 line-clamp-3 text-[13px] leading-snug text-ink-300">{msg.body}</div>
      </div>
    </div>
  );
}
