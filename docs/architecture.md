# Architecture

```
getlocked/
  crates/core/                 Platform-independent logic (pure Rust, fully unit-tested)
    model.rs                   Config, presets, rules, strictness, session kinds (serde, camelCase JSON)
    session.rs                 Session state machine: focus/break/time-up, ticks, ending
    end_policy.rs              Can this session end now? Free / needs code / locked
    unlock.rs                  Unlock code generation and checking
    matcher.rs                 Which processes break the rules; shell allowlist; browser exes
    scheduler.rs               Which schedules fire between two checks
    stats.rs                   SQLite history, summaries, streaks
    protocol.rs                App ⇄ extension messages
    pairing.rs                 Pairing code and the handshake proofs (mirrored in extension/src/pairing.ts)
    sites.rs, persist.rs       Domain cleanup; atomic JSON files
  apps/desktop/                Tauri 2 desktop app
    src/                       React + TypeScript UI (Vite, Tailwind, Recharts)
    src-tauri/src/
      lib.rs                   Tauri setup: plugins, tray, window behavior
      runtime.rs               App state and the background loop (tick, enforce, bridge, schedule)
      commands.rs              Commands the UI calls
      enforcer/mod.rs          Shared enforcement logic + the `Platform` trait
      enforcer/windows.rs      Windows implementation
      bridge.rs                Localhost WebSocket server for the extension
      guard.rs                 Hard-mode watchdog (`--guard`)
      apps_index.rs            Installed apps from Start Menu shortcuts
  extension/                   MV3 extension for Chromium + Firefox (esbuild)
    src/background.ts          WebSocket client, DNR rule sync, offline cache
    src/rules.ts               Pure rule helpers (unit-tested)
    src/blocked.ts, popup.ts   Blocked page and toolbar popup
  packages/protocol/           Shared TypeScript protocol types
```

## How a session flows

1. The UI calls `start_session` with a `SessionConfig`. `runtime::AppState::start_session` starts the `Engine` and saves `session.json`.
2. `runtime::spawn_ticker` runs every 500 ms:
   - feeds the elapsed time into `Engine::tick`, which returns events (break started, time up…)
   - saves the session every 5 s
   - pushes the site rules to every connected extension (`Bridge::set_rules`, only when they change)
   - runs `Enforcer::enforce` with the app rules
   - fires due schedules every 30 s
   - emits a `state` event that the UI renders
3. Ending goes through `Engine::end`, which applies the end policy and the unlock code. `AppState::finish` writes stats, deletes `session.json`, stops the guard and clears the extension rules.

Everything time-related in the engine is fed in by the caller, so tests run instantly without sleeping.

## Adding a new OS

The OS-specific code is just the `Platform` trait in `apps/desktop/src-tauri/src/enforcer/mod.rs`:

```rust
pub trait Platform: Send {
    fn snapshot(&mut self) -> Vec<ProcessInfo>;   // processes the user owns
    fn request_close(&mut self, pid: u32) -> bool; // polite close; false = nothing to ask
    fn kill(&mut self, pid: u32);
}
```

plus `process_alive(pid)` (used by the guard) and, optionally, an installed-apps lister in `apps_index.rs`.

To add macOS or Linux:

1. Add `enforcer/macos.rs` (or `linux.rs`) implementing `Platform`. The `sysinfo` crate is a good base. For a polite close, use `NSRunningApplication.terminate()` on macOS or `SIGTERM` on Linux.
2. Return it from `enforcer::platform()` and implement `process_alive` behind the matching `#[cfg]`.
3. Extend `SHELL_ALLOW` and `BROWSERS` in `crates/core/src/matcher.rs` with that OS's shell processes and browser executable names. Matching is by file name, so `Firefox` on macOS needs its own entry.
4. Add the bundle targets in `tauri.conf.json` and a CI job.

Nothing else should need to change.

## Tests

- `cargo test --workspace`: core logic (rules, session engine, end policy, unlock, scheduler, stats) and the enforcer against a fake platform.
- `npm test`: UI formatting helpers and the extension's rule builder.
