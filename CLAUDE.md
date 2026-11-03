# agentic-desktop

Rust CLI + C library that lets AI agents see and operate desktop apps through the OS accessibility tree. It is a tool agents call, not an agent: no LLM inside, no GUI. Reference clone of the upstream project lives at `../reference/agent-desktop-upstream`.

## Rules
- Core (`crates/core`) never depends on a platform crate. Backends implement `PlatformAdapter`; every method defaults to "not supported".
- Refs are snapshot-qualified (`@<snapshot>:e<n>`). Never accept a bare `@eN`. Zero live matches is `STALE_REF`; two or more is `AMBIGUOUS_TARGET`. Looks-alike elements are found by identity plus tree position, and the action result says `matched_by: position`.
- Generic text actions (`clipboard.*`, `selection.*`, `menu.popup`, `link.copy`) never earn a ref on their own.
- A backend that cannot read the whole window marks cut nodes `subtree_truncated`; snapshots then report `complete: false`.
- One JSON envelope per command. Exit codes: 0 ok, 1 error, 2 bad arguments.
- No `unwrap`/`expect`/`panic!` outside tests. Keep Rust files under 400 lines.
- Commands that send real keys or mouse input (`press`, `type-text`, `mouse-*`) must require `--headed` (`POLICY_DENIED` otherwise). Never add input commands without that guard. Shortcuts in `blocked_combo_reason` (alt+f4, meta+l ...) also need `--force`.
- Never claim a platform "blocks" something without checking upstream's source in `../reference/agent-desktop-upstream`. Wayland keys work on GNOME through `org.gnome.Mutter.RemoteDesktop` (see `crates/linux/src/wayland_keys.rs`); only the pointer is X11-only.
- Snapshots list `set-value` and `type` in `available_actions` only for elements with an editable-text interface. Brave's address bar has none, so it needs click + `type-text`.
- In tests, never build input text with `$(printf '\n')`: command substitution strips it, and the test then types real text into the user's focused window.
- The CLI is a library (`src/lib.rs`, crate `agentic_desktop_cli`) plus a thin `main.rs`; the C library calls `run_to_envelope`. Add new commands in `src/lib.rs` only.
- Use `AGENTIC_DESKTOP_FIXTURE=1` to run the CLI against the fake desktop.

## Linux notes
- AT-SPI cache (`Cache.GetItems`) is trusted only for name, role number, interfaces and states. GTK4 reports wrong child counts and indices in it, so the tree shape always comes from `GetChildren`.
- Live checks are `#[ignore]` tests: `cargo test -p agentic-desktop-linux -- --ignored --nocapture cache_matches_slow_walk` (needs Nautilus running).
- Never `pkill -f` or `pgrep -x` long names in scripts: `-f` can match your own shell.

## Windows notes
- Backend is `crates/windows/src/uia.rs`; role mapping in `roles.rs` is pure and tested everywhere. The app is a synthetic root and its windows are its children, so `path[0]` is a window index.
- Check from Linux with `cargo check -p agentic-desktop --target x86_64-pc-windows-msvc` (run `rustup target add` first). CI builds and tests it on `windows-latest`.

## macOS notes
- Backend calls the C Accessibility API through plain `extern` declarations in `crates/macos/src/ffi.rs` (no objc crates). `Cf` owns every CoreFoundation object; wrap borrowed ones with `retained`.
- Apps are found from process paths (`Name.app/Contents/MacOS/`); nested helper apps are ignored. Needs the Accessibility permission, else `PERM_DENIED`.
- Check from Linux with `cargo check -p agentic-desktop --target aarch64-apple-darwin`.

## Checks before every commit
`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, then clippy with `-p agentic-desktop-windows -p agentic-desktop -p agentic-desktop-ffi --target x86_64-pc-windows-msvc` and `-p agentic-desktop-macos ... --target aarch64-apple-darwin`. Always read the exit code, not just the output: a clean-looking empty output once hid a failed build. `scripts/ffi-smoke.sh` tests the C library.

## Environment
- Source is on an NTFS drive: keep build output off it. Copy `.cargo/config.toml.example` to `.cargo/config.toml` and set `target-dir` for your machine (that file is git-ignored).
- Use pnpm if a Node package manager is ever needed.

## Roadmap
Done: 1 core + CLI, 2 Linux AT-SPI (+2b speed, positional refs), 3 Windows UIA, 4 macOS AX.
Windows and macOS backends compile and lint from Linux but have NOT run on real machines.
Phase 5 done in code: clipboard, keys (X11 and GNOME Wayland), mouse (X11 only), screenshots, C library (`crates/ffi`, one `ad_execute(json_args)` entry point).
Not live-tested: Linux clipboard (no tool installed), Linux X11 input (no X server), key delivery into a real focused app on Wayland (the session accepts a Shift tap; nothing more confirmed), Wayland screenshot portal (permission prompt), everything on Windows and macOS.
The fake desktop keeps its clipboard in memory, so it does not persist between CLI calls (tests cover the round trip).
