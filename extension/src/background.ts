// Background worker: keeps a WebSocket open to the GetLocked app on this PC
// and turns the session's site rules into declarativeNetRequest rules.
// Works in Chromium (service worker) and Firefox (event page).

import { PORTS, PROTOCOL_VERSION, type BrowserFamily, type FromExtension, type SiteRulesMsg, type ToExtension } from "@getlocked/protocol";
import { buildDnrRules, cachedRulesExpired, hostOf, isBlocked, RULE_IDS } from "./rules";

const BLOCKED_PAGE = chrome.runtime.getURL("blocked.html");
const ALARM = "getlocked-keepalive";

let socket: WebSocket | null = null;
let connecting = false;

interface StoredState {
  rules: SiteRulesMsg | null;
  lastContact: number;
  connected: boolean;
}

async function load(): Promise<StoredState> {
  const s = await chrome.storage.local.get(["rules", "lastContact", "connected"]);
  return {
    rules: (s.rules as SiteRulesMsg | undefined) ?? null,
    lastContact: (s.lastContact as number | undefined) ?? 0,
    connected: (s.connected as boolean | undefined) ?? false,
  };
}

function detectBrowser(): BrowserFamily {
  const ua = navigator.userAgent;
  if (ua.includes("Firefox/")) return "firefox";
  const brands = ((navigator as unknown as { userAgentData?: { brands?: { brand: string }[] } }).userAgentData?.brands ?? []).map(
    (b) => b.brand,
  );
  if (brands.includes("Microsoft Edge") || ua.includes("Edg/")) return "edge";
  if (brands.includes("Opera") || ua.includes("OPR/")) return "opera";
  if (brands.includes("Brave") || "brave" in navigator) return "brave";
  if (ua.includes("Vivaldi")) return "vivaldi";
  if (brands.includes("Google Chrome")) return "chrome";
  return "chromium";
}

async function applyRules(rules: SiteRulesMsg | null) {
  const existing = await chrome.declarativeNetRequest.getDynamicRules();
  await chrome.declarativeNetRequest.updateDynamicRules({
    removeRuleIds: existing.map((r) => r.id).filter((id) => Object.values(RULE_IDS).includes(id as 1 | 2)),
    addRules: buildDnrRules(rules, BLOCKED_PAGE),
  });
  // Pages that were already open when the session started get redirected too.
  if (rules?.active) {
    const tabs = await chrome.tabs.query({});
    for (const tab of tabs) {
      if (tab.id !== undefined && tab.url && isBlocked(tab.url, rules)) {
        const host = hostOf(tab.url) ?? "";
        void chrome.tabs.update(tab.id, { url: `${BLOCKED_PAGE}?d=${encodeURIComponent(host)}` });
      }
    }
  }
}

async function setRules(rules: SiteRulesMsg | null) {
  await chrome.storage.local.set({ rules, lastContact: Date.now() });
  await applyRules(rules);
}

function send(msg: FromExtension) {
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(msg));
}

async function onMessage(raw: string) {
  let msg: ToExtension;
  try {
    msg = JSON.parse(raw);
  } catch {
    return;
  }
  await chrome.storage.local.set({ lastContact: Date.now() });
  switch (msg.type) {
    case "welcome":
      break;
    case "rules": {
      const { type: _type, ...rules } = msg;
      await setRules(rules);
      break;
    }
    case "ping":
      send({ type: "heartbeat" });
      break;
  }
}

function tryPort(port: number): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    const timer = setTimeout(() => {
      ws.close();
      reject(new Error("timeout"));
    }, 2000);
    ws.onopen = () => {
      clearTimeout(timer);
      resolve(ws);
    };
    ws.onerror = () => {
      clearTimeout(timer);
      reject(new Error("refused"));
    };
  });
}

async function connect() {
  if (connecting || socket?.readyState === WebSocket.OPEN || socket?.readyState === WebSocket.CONNECTING) return;
  connecting = true;
  try {
    for (const port of PORTS) {
      try {
        const ws = await tryPort(port);
        socket = ws;
        ws.onmessage = (e) => void onMessage(String(e.data));
        ws.onclose = () => {
          if (socket === ws) socket = null;
          void chrome.storage.local.set({ connected: false });
          setTimeout(() => void connect(), 3000);
        };
        const incognitoAllowed = await chrome.extension.isAllowedIncognitoAccess();
        send({
          type: "hello",
          protocol: PROTOCOL_VERSION,
          browser: detectBrowser(),
          version: chrome.runtime.getManifest().version,
          incognitoAllowed,
        });
        await chrome.storage.local.set({ connected: true, lastContact: Date.now() });
        return;
      } catch {
        // Try the next port.
      }
    }
    await chrome.storage.local.set({ connected: false });
  } finally {
    connecting = false;
  }
}

/** Runs on startup and every alarm: reconnect, and drop rules the app can no longer vouch for. */
async function checkIn() {
  await connect();
  const state = await load();
  if (socket?.readyState !== WebSocket.OPEN && cachedRulesExpired(state.rules, state.lastContact, Date.now())) {
    await setRules(null);
  }
}

chrome.runtime.onMessage.addListener((msg: { type: string; domain?: string }, _sender, reply) => {
  if (msg.type === "blocked" && msg.domain) {
    send({ type: "blocked", domain: msg.domain });
  } else if (msg.type === "status") {
    void load().then((s) => reply({ ...s, connected: socket?.readyState === WebSocket.OPEN }));
    return true;
  }
  return false;
});

chrome.alarms.create(ALARM, { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener((a) => {
  if (a.name === ALARM) void checkIn();
});
chrome.runtime.onStartup.addListener(() => void checkIn());
chrome.runtime.onInstalled.addListener(() => void checkIn());
void checkIn();
