# Contributing to GetLocked

Thanks for helping! Bug reports, feature ideas, docs fixes and code are all welcome.

## Ground rules

- **Never modify the user's system or other apps.** GetLocked's promise is that it only closes apps and redirects pages during a session. Changes that need admin rights, edit system files, install services or inject into other processes won't be accepted.
- **No telemetry, no network calls** beyond the local app ⇄ extension connection.
- Keep platform-independent logic in `crates/core` with unit tests. Keep OS-specific code behind the `Platform` trait.

## Setup

Prerequisites: Rust stable, Node.js 22+, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run ext:build      # once, before any cargo command: the app bundles the extension
npm run dev            # desktop app with hot reload (also builds the extension)
npm run ext:build      # extension → extension/dist/{chromium,firefox}
npm run watch -w extension   # rebuild the extension on change
```

Load `extension/dist/chromium` as an unpacked extension (see [docs/extension.md](docs/extension.md)) to test website blocking against the dev app.

**Careful when testing allow-only mode:** it closes every windowed app that isn't on the list. Save your work first, or test with Easy strictness so you can stop it right away.

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck
npm test
```

CI runs the same checks on Windows.

## Changing the extension protocol

The messages live in two places that must stay in sync:
`crates/core/src/protocol.rs` and `packages/protocol/src/index.ts`. Bump `PROTOCOL_VERSION` in both for breaking changes.

## Releasing (maintainers)

1. Bump the version in `Cargo.toml` (`workspace.package`), `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json` and `extension/package.json`.
2. Tag `vX.Y.Z` and push the tag. The Release workflow builds the installer and the extension zips into a draft GitHub release.
3. Review the draft and publish it.

## License

By contributing, you agree that your contributions are licensed under the GPL-3.0, the same as the project.
