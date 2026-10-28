# FAQ

**Is it safe? Will it break my apps?**
GetLocked doesn't modify apps or system files. It closes blocked apps the same way you would: it first asks them to close, as if you'd clicked ×, and only ends them if they don't respond within 2 seconds. Apps that handle closing normally will offer to save your work. If an app holds unsaved work and ignores close requests, that work can be lost, so block apps before you start using them, not halfway through.

**I'm stuck in a Hard session. How do I get out?**
Wait for the timer, which is the point of Hard mode. For a Hard stopwatch, reach the minimum time and then type the unlock code. If you really must, Hard mode is "reasonable effort", not a prison (see [How it works](how-it-works.md#hard-mode)). If you need guaranteed escape hatches, use Friction instead.

**Does it need admin rights?**
No. It runs as your user and only touches your own processes.

**Why does my browser keep closing?**
In Friction and Hard sessions with website rules, browsers without the GetLocked extension connected are closed after a short grace period. Install the [extension](extension.md) in that browser, or turn off "Close browsers that don't have the extension" for the session.

**Does Hard mode block private/incognito windows?**
Only if the extension is allowed in them. In Hard mode GetLocked requires that permission and closes the browser otherwise.

**Do Pomodoro breaks count toward the timer?**
No. Only focus time counts, so a 2-hour timer with breaks takes longer than 2 hours of wall-clock time.

**What happens if my PC sleeps or restarts mid-session?**
The session pauses and resumes. Sleep doesn't count as focus time. After a restart, a Hard session reopens automatically. For other strictness levels, it resumes the next time you open GetLocked, or at sign-in if Start with Windows is on.

**Can I use it on macOS or Linux?**
Not yet. The blocking code is behind a small per-OS interface, so ports are very doable. See [Adding a new OS](architecture.md#adding-a-new-os).

**Does any data leave my PC?**
No. There's no account, no telemetry, no update pings. Stats live in a local SQLite file.

**How do I uninstall it?**
Uninstall it from Windows Settings → Apps, remove the browser extension, and optionally delete `%APPDATA%\dev.getlocked.app`.
