# How it works

## Blocking apps

While a session is in a focus phase, GetLocked takes a snapshot of running processes about every 0.75 seconds (using the Windows Toolhelp API, which needs no admin rights). For each process in **your** Windows session that it's allowed to open:

- **Block mode:** if the process matches a rule (by `.exe` name, or because it runs from inside a blocked folder), it's closed. This applies even with no window, so launchers can't hide in the tray.
- **Allow-only mode:** if the process shows a window and isn't on your list, it's closed. Background processes without windows are left alone, and so are Windows shell processes (Explorer, the Start menu, search, Task Manager, the lock screen, input methods) and GetLocked itself.

"Closed" means GetLocked first sends the app's windows a normal close request (`WM_CLOSE`, the same thing as clicking ×). If the app is still running 2 seconds later, it's ended (`TerminateProcess`). Store apps hosted by `ApplicationFrameHost` are matched to their real process.

Processes running as administrator, as another user, or as protected system processes are skipped. GetLocked couldn't touch them anyway without admin rights, which it never asks for.

## Blocking websites

The browser extension uses the browser's built-in [declarativeNetRequest](https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest) rules to redirect blocked top-level pages to its own "You're locked in" page. Tabs that were already open on a blocked site are redirected when the session starts. Only full page loads are blocked: embedded videos or images on other sites aren't.

Only browser extensions can connect to the app's local WebSocket. Connections without a `chrome-extension://` or `moz-extension://` origin are rejected, so web pages can't read or change your session.

That origin check doesn't stop other programs on your PC, which can send any header they like. So the app and the extension also share a **pairing code**, made once per install (Settings → Browser extension). Each connection starts with a handshake: both sides swap random nonces and prove they know the code with an HMAC-SHA256 over the nonces and the port the extension dialled. The code itself is never sent. Until the handshake passes, the app sends no rules and doesn't count the browser as connected, and the extension ignores everything it receives. A program that grabs one of GetLocked's ports first can't pose as the app or relay a real handshake to it. (Anything running as your Windows user can still read the code from GetLocked's data folder; this guards against impostors, not against malware you've already run.)

The extension only reports a block to the app when the report comes from its own blocked page, open as a tab's top-level page, for a site the current rules really block.

## Strictness and the unlock code

The Rust core decides whether a session can end (`crates/core/src/end_policy.rs`). The unlock code is generated, shown and checked there too, not in the UI. The UI also blocks copy and paste on the code, but the real friction is the 30-second wait plus typing 32 random characters.

## Tasks and taunts

Task rules live in the Rust core (`crates/core/src/tasks.rs`), not the UI: notes and drop reasons need 3+ words, editing a ticked task unticks it, and dropped tasks stay in your stats. When every remaining task is ticked, the end check frees the session at any strictness.

When you hit a block, GetLocked can roast you with your own record: how many sessions you've ended early since Monday, from your local stats. Turn it off with **Settings → Roast me when I slip**. The browser extension gets a ready-made line with a `{site}` gap, plus your unticked tasks, so the blocked page can show them. Only the paired extension receives them.

## Hard mode

- **Survives app kills:** during a Hard session, GetLocked starts a second copy of itself with `--guard`. Each copy watches the other. If you end the app in Task Manager, the guard relaunches it within about a second, and the session resumes.
- **Survives restarts:** the session is saved to `session.json` every 5 seconds, and Hard sessions turn on **Start with Windows**. After a restart, GetLocked opens and resumes where it left off.
- **Time can't be skipped:** the session counts elapsed focus time on a monotonic clock, so changing the Windows clock doesn't help. Sleep and shutdown pause the session instead of counting.

Hard mode is "reasonable effort", not unbreakable: someone determined can still get around it (for example, *End process tree* in Task Manager followed by deleting the session file). The goal is to make giving in take more effort than just doing the work.

## What GetLocked touches on your PC

| What | Where | When |
|---|---|---|
| Settings, presets, schedules | `%APPDATA%\dev.getlocked.app\config.json` | Always |
| Running session | `%APPDATA%\dev.getlocked.app\session.json` | During a session; deleted when it ends |
| Stats | `%APPDATA%\dev.getlocked.app\stats.sqlite` | Always |
| Start with Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (your user only) | Only if you turn it on, or start a Hard session |
| Local WebSocket | `127.0.0.1:47613` (up to 47617) | While the app runs |

That's it. No system files, no hosts file, no services, no drivers, and nothing installed into other apps.
