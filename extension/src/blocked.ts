// The page shown instead of a blocked site.

import type { SiteRulesMsg } from "@getlocked/protocol";
import { clock } from "./rules";

const QUOTES = [
  "The work you're avoiding is the work that matters.",
  "Future you will be glad you stayed.",
  "One focused hour beats a distracted day.",
  "You opened this tab on autopilot. Back to it.",
  "Finish first. The internet will still be here.",
];

const params = new URLSearchParams(location.search);
const domain = (params.get("d") ?? "").replace(/^www\./, "");
const $ = (id: string) => document.getElementById(id)!;

if (domain) {
  $("domain").textContent = domain;
  document.title = `${domain} is blocked · GetLocked`;
  void chrome.runtime.sendMessage({ type: "blocked", domain });
}
$("quote").textContent = QUOTES[Math.floor(Math.random() * QUOTES.length)];
$("close").addEventListener("click", () => window.close());

async function render() {
  const { rules } = (await chrome.storage.local.get("rules")) as { rules?: SiteRulesMsg };
  if (rules?.label) $("label").textContent = `You're locked in: ${rules.label}.`;
  const show = !!rules?.endsAt;
  $("time").hidden = !show;
  $("time-caption").hidden = !show;
  if (rules?.endsAt) $("time").textContent = clock(rules.endsAt - Date.now());
}

void render();
setInterval(() => void render(), 1000);
