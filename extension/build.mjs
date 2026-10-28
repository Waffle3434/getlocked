// Builds the extension for Chromium browsers and Firefox into dist/<target>.
//   node build.mjs           build both
//   node build.mjs --watch   rebuild on change

import { build, context } from "esbuild";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));
const watch = process.argv.includes("--watch");

const base = {
  manifest_version: 3,
  name: "GetLocked",
  version: pkg.version,
  description: "Blocks distracting websites during a GetLocked focus session. Needs the free GetLocked desktop app.",
  icons: { 32: "icons/32.png", 64: "icons/64.png", 128: "icons/128.png" },
  action: { default_popup: "popup.html", default_icon: { 32: "icons/32.png", 64: "icons/64.png" } },
  permissions: ["declarativeNetRequestWithHostAccess", "storage", "alarms", "tabs"],
  host_permissions: ["<all_urls>"],
  web_accessible_resources: [{ resources: ["blocked.html"], matches: ["<all_urls>"] }],
  incognito: "spanning",
};

const targets = {
  chromium: {
    ...base,
    minimum_chrome_version: "116",
    background: { service_worker: "background.js" },
  },
  firefox: {
    ...base,
    background: { scripts: ["background.js"] },
    browser_specific_settings: { gecko: { id: "extension@getlocked.dev", strict_min_version: "128.0" } },
  },
};

const options = (outdir) => ({
  entryPoints: ["src/background.ts", "src/blocked.ts", "src/popup.ts"],
  bundle: true,
  format: "iife",
  target: ["chrome116", "firefox128"],
  outdir,
  logLevel: "info",
});

for (const [name, manifest] of Object.entries(targets)) {
  const outdir = `dist/${name}`;
  rmSync(outdir, { recursive: true, force: true });
  mkdirSync(outdir, { recursive: true });
  cpSync("public", outdir, { recursive: true });
  writeFileSync(`${outdir}/manifest.json`, JSON.stringify(manifest, null, 2));
  if (watch) {
    await (await context(options(outdir))).watch();
  } else {
    await build(options(outdir));
  }
}
