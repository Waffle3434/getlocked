// Toolbar popup: connection status and the current session's site rules.

import type { SiteRulesMsg } from "@getlocked/protocol";
import { clock } from "./rules";

interface Status {
  rules: SiteRulesMsg | null;
  lastContact: number;
  connected: boolean;
}

const $ = (id: string) => document.getElementById(id)!;
const ALL_SITES = { origins: ["<all_urls>"] };

async function render() {
  const status = (await chrome.runtime.sendMessage({ type: "status" })) as Status | undefined;
  const connected = !!status?.connected;
  $("dot").classList.toggle("ok", connected);
  $("conn").textContent = connected ? "Connected to the GetLocked app" : "GetLocked app not found. Is it running?";

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

void render();
setInterval(() => void render(), 1000);
