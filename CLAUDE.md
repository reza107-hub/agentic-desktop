# agentic-desktop

Rust CLI + C library that lets AI agents see and operate desktop apps through the OS accessibility tree. It is a tool agents call, not an agent: no LLM inside, no GUI. Reference clone of the upstream project lives at `../reference/agent-desktop-upstream`.

## Rules
- Core (`crates/core`) never depends on a platform crate. Backends implement `PlatformAdapter`; every method defaults to "not supported".
- Refs are snapshot-qualified (`@<snapshot>:e<n>`). Never accept a bare `@eN`. Zero live matches is `STALE_REF`; two or more is `AMBIGUOUS_TARGET`.
- One JSON envelope per command. Exit codes: 0 ok, 1 error, 2 bad arguments.
- No `unwrap`/`expect`/`panic!` outside tests. Keep Rust files under 400 lines.
- Use `AGENTIC_DESKTOP_FIXTURE=1` to run the CLI against the fake desktop.

## Environment
- Source is on an NTFS drive: keep build output off it. Copy `.cargo/config.toml.example` to `.cargo/config.toml` and set `target-dir` for your machine (that file is git-ignored).
- Use pnpm if a Node package manager is ever needed.

## Roadmap
Phase 1 core + CLI (done) -> 2 Linux AT-SPI -> 3 Windows UIA -> 4 macOS AX -> 5 input, screenshots, clipboard, C library.
