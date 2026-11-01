// Background worker: keeps a WebSocket open to the GetLocked app on this PC
// and turns the session's site rules into declarativeNetRequest rules.
// Works in Chromium (service worker) and Firefox (event page).
//
// Every connection starts with the pairing handshake (see pairing.ts):
// nothing from a peer is trusted until it has proved it knows the pairing
// code, so another program listening on GetLocked's ports gets ignored.

import { PORTS, PROTOCOL_VERSION, type BrowserFamily, type FromExtension, type SiteRulesMsg, type ToExtension } from "@getlocked/protocol";
import { nonce, normalizeCode, proof, verify } from "./pairing";
import { buildDnrRules, cachedRulesExpired, hostOf, isBlocked, isRealBlock, parseRules, RULE_IDS } from "./rules";

const BLOCKED_PAGE = chrome.runtime.getURL("blocked.html");
const ALARM = "getlocked-keepalive";
const HANDSHAKE_TIMEOUT_MS = 3000;

let socket: WebSocket | null = null;
let connecting = false;

/** `missing`: no code yet. `rejected`: the app didn't accept ours. */
type Pairing = "ok" | "missing" | "rejected";

interface StoredState {
  rules: SiteRulesMsg | null;
  lastContact: number;
  connected: boolean;
  pairing: Pairing;
}

async function load(): Promise<StoredState> {
  const s = await chrome.storage.local.get(["rules", "lastContact", "connected", "pairing", "pairingCode"]);
  return {
    rules: parseRules(s.rules),
    lastContact: (s.lastContact as number | undefined) ?? 0,
    connected: (s.connected as boolean | undefined) ?? false,
    pairing: s.pairingCode ? ((s.pairing as Pairing | undefined) ?? "ok") : "missing",
  };
}

/** The code the app put in this extension's folder during in-app setup, if any. */
async function bundledCode(): Promise<string | null> {
  try {
    const res = await fetch(chrome.runtime.getURL("pairing.json"));
    const { code } = (await res.json()) as { code?: unknown };
    return typeof code === "string" ? normalizeCode(code) : null;
  } catch {
    return null;
  }
}

/** The pairing code to use: one typed into the popup, else the bundled one. */
async function pairingCode(): Promise<string | null> {
  const { pairingCode: stored } = await chrome.storage.local.get("pairingCode");
  if (typeof stored === "string") return stored;
  const code = await bundledCode();
  if (code) await chrome.storage.local.set({ pairingCode: code });
  return code;
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

/** Messages from the app, once the handshake has passed. */
async function onMessage(msg: ToExtension) {
  await chrome.storage.local.set({ lastContact: Date.now() });
  switch (msg.type) {
    case "rules": {
      const rules = parseRules(msg);
      if (rules) await setRules(rules);
      break;
    }
    case "ping":
      send({ type: "heartbeat" });
      break;
  }
}

/**
 * Connects to `port` and runs the handshake. Resolves with the open socket
 * once the app has proved itself, or `"rejected"` if it says our code is
 * wrong; rejects if nothing answers or the peer can't prove itself.
 */
function tryPort(port: number, code: string): Promise<WebSocket | "rejected"> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    const extNonce = nonce();
    let appNonce: string | null = null;
    let paired = false;
    const fail = (why: string) => {
      clearTimeout(timer);
      ws.close();
      reject(new Error(why));
    };
    const timer = setTimeout(() => fail("timeout"), HANDSHAKE_TIMEOUT_MS);

    // One message at a time: the rules can arrive while the welcome is still being checked.
    let queue = Promise.resolve();
    const step = async (raw: string) => {
      let msg: ToExtension;
      try {
        msg = JSON.parse(raw);
      } catch {
        return paired ? undefined : fail("bad message");
      }
      if (paired) return onMessage(msg);
      if (appNonce === null) {
        if (msg.type !== "challenge" || typeof msg.nonce !== "string") return fail("no challenge");
        appNonce = msg.nonce;
        ws.send(JSON.stringify({ type: "auth", nonce: extNonce, proof: await proof(code, "extension", appNonce, extNonce, port) }));
      } else if (msg.type === "welcome" && (await verify(code, "app", appNonce, extNonce, port, msg.proof))) {
        paired = true;
        clearTimeout(timer);
        resolve(ws);
      } else if (msg.type === "pairingFailed") {
        clearTimeout(timer);
        ws.close();
        resolve("rejected");
      } else {
        fail("peer couldn't prove it's the GetLocked app");
      }
    };
    ws.onmessage = (e) => {
      queue = queue.then(() => step(String(e.data)));
    };
    ws.onerror = () => fail("refused");
    ws.onclose = () => fail("closed");
  });
}

async function connect() {
  if (connecting || socket?.readyState === WebSocket.OPEN || socket?.readyState === WebSocket.CONNECTING) return;
  connecting = true;
  try {
    let code = await pairingCode();
    if (!code) {
      await chrome.storage.local.set({ connected: false });
      return;
    }
    for (let attempt = 0; attempt < 2; attempt++) {
      let rejected = false;
      for (const port of PORTS) {
        let ws: WebSocket | "rejected";
        try {
          ws = await tryPort(port, code);
        } catch {
          continue; // Nothing there, or not the real app: try the next port.
        }
        if (ws === "rejected") {
          rejected = true;
          continue;
        }
        socket = ws;
        ws.onerror = null;
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
        await chrome.storage.local.set({ connected: true, pairing: "ok", lastContact: Date.now() });
        return;
      }
      // The code may have been reset in the app, which also refreshes the
      // copy in the folder this extension was loaded from. Try that once.
      const bundled = rejected ? await bundledCode() : null;
      if (!bundled || bundled === code) {
        await chrome.storage.local.set({ connected: false, ...(rejected ? { pairing: "rejected" } : {}) });
        return;
      }
      code = bundled;
      await chrome.storage.local.set({ pairingCode: code });
    }
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

/** Code typed into the popup. Replaces the old one and reconnects. */
async function pair(input: string): Promise<boolean> {
  const code = normalizeCode(input);
  if (!code) return false;
  await chrome.storage.local.set({ pairingCode: code, pairing: "ok" });
  const old = socket;
  socket = null;
  if (old) {
    old.onclose = null;
    old.close();
  }
  await connect();
  return true;
}

chrome.runtime.onMessage.addListener((msg: { type: string; domain?: unknown; code?: unknown }, sender, reply) => {
  // Only our own pages may talk to the background.
  if (sender.id !== chrome.runtime.id) return false;
  if (msg.type === "blocked" && typeof msg.domain === "string") {
    // Only the blocked page, open as a tab's top-level page, reports blocks,
    // and only for a site the current rules really block. blocked.html is
    // web-accessible, so any site can open it with a made-up `?d=`.
    const domain = msg.domain;
    const fromBlockedPage = !!sender.tab && sender.frameId === 0 && !!sender.url?.startsWith(BLOCKED_PAGE);
    if (fromBlockedPage) {
      void load().then((s) => {
        if (isRealBlock(domain, s.rules)) send({ type: "blocked", domain });
      });
    }
  } else if (msg.type === "status") {
    void load().then((s) => reply({ ...s, connected: socket?.readyState === WebSocket.OPEN }));
    return true;
  } else if (msg.type === "pair" && typeof msg.code === "string") {
    void pair(msg.code).then((ok) => reply({ ok }));
    return true;
  }
  return false;
});

chrome.alarms.create(ALARM, { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener((a) => {
  if (a.name === ALARM) void checkIn();
});
chrome.runtime.onStartup.addListener(() => void checkIn());
chrome.runtime.onInstalled.addListener(async ({ reason }) => {
  // Installed by hand rather than from GetLocked's setup: ask for the code.
  if (reason === "install" && !(await pairingCode())) {
    void chrome.tabs.create({ url: chrome.runtime.getURL("popup.html") });
  }
  void checkIn();
});
void checkIn();
