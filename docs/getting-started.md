# Getting started

## 1. Install

Download `GetLocked_x.y.z_x64-setup.exe` from the Releases page and run it. It installs for your user account only, so no admin prompt.

GetLocked lives in the system tray. Closing the window just hides it. Use **Quit GetLocked** in the tray menu or in Settings to exit. You can't quit while a Friction or Hard session is running.

## 2. Add the browser extension (optional)

Website blocking needs the extension. See [Browser extension setup](extension.md). Without it, app blocking still works but websites won't be blocked.

## 3. Start a session

On the **Start** page:

1. **How long?**
   - **Timer:** pick a length. When it runs out, GetLocked asks *I'm done* or *Keep going*. Keep going turns the session into a stopwatch, and you can end it whenever you like.
   - **Stopwatch:** no time limit. It counts up until you end it.

   Only focus time counts. Pomodoro breaks pause the clock, and so does a sleeping PC.
2. **How strict?**

   | | Timer | Stopwatch |
   |---|---|---|
   | **Easy** | End any time | End any time |
   | **Friction** | End early with the unlock code | End with the unlock code |
   | **Hard** | Can't end until the timer runs out | Unlock code, and only after the minimum time if you set one |

   The **unlock code** appears after a 30-second wait. It's a long random code you have to type by hand: copy and paste are blocked, and a wrong guess gets you a new code. The idea is to give you a moment to reconsider.
3. **Apps:** *Block these* closes the listed apps whenever they open. *Allow only these* closes any other app that opens a window. Explorer, the Start menu, Task Manager and GetLocked itself always stay usable.
   - Add apps from **Running now**, **Installed** (your Start Menu), or **Browse** for any `.exe`.
   - **Folder rules** block everything that runs from a folder. Point one at `…\Steam\steamapps\common` to cover every Steam game at once.
4. **Websites:** *Block these* or *Allow only these*. Subdomains are included, so `youtube.com` also covers `m.youtube.com`. In allow-only mode, `localhost` always works so local dev servers keep running.
5. **Pomodoro breaks** (optional): for example, 25 minutes of focus and 5 minutes of break. Blocks lift during breaks.

Press **Lock in**.

## Presets and schedules

- **Save as preset** on the Start page, or manage presets on the **Presets** page.
- On the **Schedule** page, pick a preset, days and a start time. GetLocked has to be running for a schedule to fire, so turn on **Start with Windows** in Settings. A start missed by up to 15 minutes (say the PC was asleep) still fires. Older ones are skipped.

## Stats

The **Stats** page shows focus time per day, your current and best streaks (consecutive days with at least one completed session), what got blocked most, and recent sessions. Everything is stored locally in `%APPDATA%\dev.getlocked.app\stats.sqlite`.
