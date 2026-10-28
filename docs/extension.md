# Browser extension setup

**Easiest way:** open GetLocked → **Settings → Browser extension → Set up extension**. It finds your browsers, opens the right page, and gives you the folder to load. The steps below are the same thing done by hand.

GetLocked blocks websites through a small extension that connects to the GetLocked app on your own PC (`ws://127.0.0.1:47613`). The extension never talks to the internet. It only receives "block these sites until X" from the app and redirects those pages to a "You're locked in" screen.

Each release includes two zips:

- `getlocked-extension-chromium.zip` for Chrome, Edge, Brave, Opera and Vivaldi
- `getlocked-extension-firefox.zip` for Firefox

> Store listings (Chrome Web Store, Edge Add-ons, Firefox Add-ons) are planned. Until then, install them as shown below.

## Chrome, Edge, Brave, Opera, Vivaldi

1. Unzip `getlocked-extension-chromium.zip` somewhere permanent, e.g. `Documents\GetLocked\extension`. The browser loads it from that folder, so don't delete it.
2. Open the extensions page: `chrome://extensions`, `edge://extensions`, `brave://extensions`, `opera://extensions` or `vivaldi://extensions`.
3. Turn on **Developer mode**.
4. Click **Load unpacked** and choose the unzipped folder.
5. Optional, but needed for Hard mode: open the extension's **Details** and turn on **Allow in Incognito** (*Allow in InPrivate* in Edge). Otherwise private windows aren't blocked.

## Firefox

1. Open `about:debugging#/runtime/this-firefox`.
2. Click **Load Temporary Add-on…** and choose `manifest.json` inside the unzipped `getlocked-extension-firefox` folder.
3. Click the GetLocked toolbar icon. If it says it needs access to all sites, click **Grant access**.

Temporary add-ons are removed when Firefox restarts. A signed version that stays installed will come with the Firefox Add-ons listing.

## Check it's working

- The toolbar popup should say **Connected to the GetLocked app**.
- In GetLocked → **Settings → Browser extension**, your browser should be listed.

## "Close browsers that don't have the extension"

In Friction and Hard sessions with website rules, GetLocked can close any supported browser that's running *without* a connected extension. That stops the obvious workaround of opening a different browser. You can turn this off per session. The grace period (default 10 seconds) is in Settings.

In Hard mode, the extension also needs private-window access to count as connected.

## If the app isn't running

The extension keeps enforcing the last rules it got until that block ends on its own. For stopwatch sessions with no end time, it keeps them for up to 10 minutes after it last heard from the app. After that it clears them, so sites can never stay blocked forever, even if GetLocked is uninstalled.

## Building it yourself

```bash
npm install
npm run ext:build   # → extension/dist/chromium and extension/dist/firefox
```
