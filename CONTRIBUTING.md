# Contributing to Launchpad

Thanks for helping! Launchpad is a small Windows launcher, and the most useful contributions are bug reports from real setups: unusual shortcuts, apps whose icons don't load, multi-monitor and high-DPI quirks.

## Development setup

1. Install stable Rust via [rustup](https://rustup.rs) (1.82 or newer) with the MSVC toolchain.
2. Clone and run:

   ```bash
   git clone https://github.com/X4Lo/LaunchPad.git
   cd LaunchPad
   cargo run
   ```

3. Useful while developing:

   ```bash
   cargo test                                  # unit tests
   cargo run -- --minimized                    # start in the tray
   RUST_LOG=launchpad=debug cargo run          # verbose logs (icon loading, hotkey, config)
   ```

   Tip: put a `config.json` next to `target/debug/launchpad.exe` to use a throwaway portable config instead of your real one.

Read [docs/technical-reference.md](docs/technical-reference.md) before changing `src/app.rs` or the icon code.

## Coding style

- `cargo fmt` (config in `rustfmt.toml`) and `cargo clippy --all-targets -- -D warnings` must pass.
- Keep Win32 / OS code in `src/platform/`. `app.rs` and the UI never call Windows APIs directly.
- Item-tree changes go through `src/commands/items.rs` and get unit tests next to the code (`#[cfg(test)] mod tests`).
- Keep `config.json` backwards compatible: new fields need `#[serde(default)]`, and old configs must keep loading.
- No `unwrap()`/`expect()` on OS or I/O calls in app code. Surface errors to the user via the status banner.
- UI: edit `ui/*.slint`. Use `Theme` tokens from `ui/theme.slint` instead of hard-coded colours, and set accessibility properties (`accessible-role`, `accessible-label`) on interactive elements.
- Keep it light: no polling while the window is hidden, no webviews or GPU renderers.

## Branches and pull requests

- Branch from `master`: `feat/<short-name>`, `fix/<short-name>`, `docs/<short-name>`.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org): `feat(ui): …`, `fix(icons): …`, `docs: …`.
- Keep PRs focused: one feature or fix per PR.
- In the PR description, say what you tested by hand (e.g. "Windows 11 24H2, 150 % scaling, two monitors").
- Update the docs when behaviour changes (README, `docs/`).

## Reporting bugs

Please include:

- **Windows version** (e.g. Windows 11 24H2) and display scaling.
- **Launchpad version**, shown in Settings → About.
- **Portable or standard mode** (also in Settings → About).
- **For icon problems:** the item's path, and whether Explorer shows an icon for it.
- **Logs:** run with `RUST_LOG=launchpad=debug`. For release builds, redirect to a file: `launchpad.exe > log.txt 2>&1`.
- What you did, what you expected, and what happened.

## Security

Please report security issues privately via GitHub's "Report a vulnerability" feature rather than a public issue.
