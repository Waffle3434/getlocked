// Toolbar popup: connection status, pairing, and the current session's site rules.
// Also opened as a tab after a manual install, to ask for the pairing code.

import type { SiteRulesMsg } from "@getlocked/protocol";
import { clock } from "./rules";

interface Status {
  rules: SiteRulesMsg | null;
  lastContact: number;
  connected: boolean;
  pairing: "ok" | "missing" | "rejected";
}

const $ = (id: string) => document.getElementById(id)!;
const ALL_SITES = { origins: ["<all_urls>"] };
let changingCode = false;

async function render() {
  const status = (await chrome.runtime.sendMessage({ type: "status" })) as Status | undefined;
  const connected = !!status?.connected;
  const pairing = status?.pairing ?? "missing";
  $("dot").classList.toggle("ok", connected);
  $("conn").textContent = connected
    ? "Connected to the GetLocked app"
    : pairing === "missing"
      ? "Not paired with the GetLocked app yet"
      : pairing === "rejected"
        ? "The GetLocked app didn't accept the pairing code"
        : "GetLocked app not found. Is it running?";

  if (connected) changingCode = false;
  $("pair").hidden = connected ? !changingCode : pairing === "ok" && !changingCode;
  $("change").hidden = !$("pair").hidden;

  const r = status?.rules;
  const session = $("session");
  if (r?.active) {
    const what = r.mode === "block" ? `Blocking ${r.domains.length} site${r.domains.length === 1 ? "" : "s"}` : `Only ${r.domains.length} site${r.domains.length === 1 ? "" : "s"} allowed`;
    session.innerHTML = "";
    const title = document.createElement("div");
    title.className = "big";
    title.textContent = r.endsAt ? clock(r.endsAt - Date.now()) : "Locked in";
    const sub = document.createElement("div");
    sub.className = "muted";
    sub.textContent = `${r.label ? `${r.label} · ` : ""}${what}`;
    session.append(title, sub);
  } else {
    session.textContent = "No session is blocking sites right now.";
  }

  // Firefox makes host access optional in MV3; without it nothing can be blocked.
  const hasHosts = await chrome.permissions.contains(ALL_SITES);
  $("perm").hidden = hasHosts;
  $("incognito").hidden = await chrome.extension.isAllowedIncognitoAccess();
}

$("grant").addEventListener("click", async () => {
  await chrome.permissions.request(ALL_SITES);
  void render();
});

$("change").addEventListener("click", () => {
  changingCode = true;
  void render();
});

$("pair-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const input = $("code") as HTMLInputElement;
  $("pair-error").textContent = "";
  const { ok } = (await chrome.runtime.sendMessage({ type: "pair", code: input.value })) as { ok: boolean };
  if (ok) {
    input.value = "";
    changingCode = false;
  } else {
    $("pair-error").textContent = "That doesn't look like a pairing code. It has 24 letters and numbers.";
  }
  void render();
});

void render();
setInterval(() => void render(), 1000);
