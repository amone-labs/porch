# Contributing

- **Open an issue before a large change.** Bug reports with steps to reproduce help most, and so do requests for another agent that say where it keeps its session records.
- **Build and test** as [AGENTS.md](AGENTS.md) describes: `cargo test`, `cargo clippy --all-targets`, and `pnpm build` in `app/`. Point the installer at copies (`--claude-dir`, `--codex-home`, a temporary `PORCH_HOME`); never test against your real `~/.claude/settings.json`.
- **The rules that hold porch together** are in [docs/design/states.md](docs/design/states.md) and the [decision records](docs/decisions/). Changing a settled decision takes a new decision record that replaces the old one.
- **Security problems** go by email, not in an issue. See [SECURITY.md](SECURITY.md).

## Build from source

Needs Rust (stable), Node 22, pnpm 10 and the Xcode Command Line Tools.

```sh
cargo build --release -p porch-cli -p porch-hook   # porch and porch-hook in target/release
cd app && pnpm install && pnpm tauri dev           # the app
```

`pnpm tauri dev` never checks for updates. For a release build of your own, turn off the updater artifacts, which need porch's signing key:

```sh
cd app && pnpm tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

That build still checks porch's update feed, and installing an update from it replaces your build with the signed release.

## License

Contributions are accepted under the project's license, [Apache-2.0](LICENSE).
