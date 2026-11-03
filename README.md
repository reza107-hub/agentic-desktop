# agentic-desktop

A command-line tool that lets an AI agent see and operate desktop apps through the operating system's accessibility tree. It runs on Linux, Windows and macOS.

The tool contains no AI model. Your agent (Claude Code, Codex, Antigravity or anything that can run shell commands) runs the loop:

1. `snapshot` reads an app and returns JSON. Every element the agent can act on gets a ref.
2. The agent acts on a ref (`click`, `type`, `set-value`).
3. The agent takes a new `snapshot` to check the result.

It does not use screenshots or pixel matching. It cannot see apps that expose no accessibility tree, and it does not automate web pages.

## Set it up by pasting this link into your AI agent

Paste this repository link into Claude Code, Codex or Antigravity and ask it to set the tool up:

```text
Set up https://github.com/reza107-hub/agentic-desktop for me.
Follow the "Instructions for AI agents" section of its README.
```

### Instructions for AI agents

If a user gave you this repository link and asked you to set it up, do these steps in order. Stop and tell the user if a step fails. Do not skip step 5.

1. Check that Rust 1.89 or newer is installed (`rustc --version`). If it is missing, tell the user and stop. Do not install it without asking.
2. Clone the repository into a folder the user agrees to, or use `cargo install --git https://github.com/reza107-hub/agentic-desktop.git agentic-desktop` to skip the clone.
3. Build and install it: `cargo install --path src` (from the clone). Make sure `~/.cargo/bin` is on the user's `PATH`.
4. Check it works: run `agentic-desktop version`, then `agentic-desktop status`. Tell the user which platform backend it reports. Also read "Platform setup" below and tell the user about any permission they must grant themselves (macOS Accessibility permission, a Linux AT-SPI session).
5. **Add the usage instructions to the project's instruction file.** Copy the whole block from "Use it with an AI agent" below into the instruction file of the project the user is working in. Use the file your own tool reads:
   - Claude Code: `CLAUDE.md` in the project root.
   - Codex: `AGENTS.md` in the project root.
   - Antigravity: `AGENTS.md` in the project root (or a file in `.agents/rules/`).

   If the file already exists, add the block at the end and leave the rest alone. If the block is already there, do not add it twice. If there is no project, ask the user whether to use their global file instead. Show the user what you added.
6. Do not change permission or sandbox settings, and do not allow the command to run without asking, unless the user asks you to.
7. Finish by telling the user what you installed, where you added the instructions, and what they still need to do themselves.

## Status

| | Linux | Windows | macOS |
|---|:---:|:---:|:---:|
| Backend | AT-SPI | UI Automation | Accessibility (AX) |
| List apps, snapshot | tested on a real desktop | compiles, **not run yet** | compiles, **not run yet** |
| click, type, set-value | tested (GNOME Calculator) | compiles, **not run yet** | compiles, **not run yet** |
| Keyboard, mouse, screenshots, clipboard | not built | not built | not built |

"Compiles" means it builds, passes lint and passes its unit tests on a real GitHub runner for that OS (see the CI workflow). It does not mean it has controlled a real app. Treat Windows and macOS as untested against real apps until someone runs them.

## Install

You need Rust 1.89 or newer.

```bash
git clone https://github.com/reza107-hub/agentic-desktop.git
cd agentic-desktop
cargo install --path src
agentic-desktop version
```

The binary is named `agentic-desktop`. `cargo install` puts it in `~/.cargo/bin`, so make sure that folder is on your `PATH`.

If you build from an NTFS drive on Linux, keep build output off it. Copy `.cargo/config.toml.example` to `.cargo/config.toml` and set `target-dir` to a folder on your Linux disk.

### Platform setup

- **Linux:** needs a graphical session with the AT-SPI service (`at-spi2-core`) running. It worked on GNOME with Wayland. If an app shows no elements, turn on accessibility support for your desktop and restart the app.
- **Windows:** no grant needed for apps at your terminal's permission level. To control an app that runs as administrator, run your terminal as administrator too.
- **macOS:** open System Settings, Privacy & Security, Accessibility, and allow your terminal (and your agent, if it runs the tool itself). Without this, every command returns `PERM_DENIED`.

## Quick start

```bash
agentic-desktop list-apps
agentic-desktop snapshot --app gnome-calculator -i
agentic-desktop click '@s8f3k2p9:e8'
agentic-desktop snapshot --app gnome-calculator -i
```

Use the app name exactly as `list-apps` prints it (letter case does not matter). On Windows and PowerShell, put quotes around every ref, because a bare `@name` has a special meaning there.

Try it without a real desktop:

```bash
AGENTIC_DESKTOP_FIXTURE=1 agentic-desktop snapshot --app Demo -i
```

## Commands

| Command | What it does |
|---|---|
| `version` | Print the version |
| `status` | Show the platform backend in use |
| `list-apps` | List apps that can be observed |
| `snapshot --app NAME [-i]` | Read an app's tree. `-i` keeps only elements with refs and the path to them |
| `click REF` | Press, activate or toggle an element |
| `type REF TEXT` | Add text to the end of a text field |
| `set-value REF TEXT` | Replace a text field's whole value |

## Output

Every command prints one JSON line:

```json
{"version":"0.1","ok":true,"command":"snapshot","data":{"app":"Demo","snapshot_id":"s8f3k2p9","ref_count":3,"complete":true,"tree":{}}}
```

```json
{"version":"0.1","ok":false,"command":"click","error":{"code":"STALE_REF","message":"...","suggestion":"...","requires_fresh_snapshot":true}}
```

- Exit code `0` means success, `1` means an error, `2` means bad arguments.
- **Refs belong to their snapshot.** `@s8f3k2p9:e8` is valid. A bare `@e8` is rejected with `INVALID_ARGS`.
- `STALE_REF` means the element is gone. `AMBIGUOUS_TARGET` means more than one element matches. In both cases, take a new snapshot.
- `complete: false` means the app was too large or too slow to read in full. Some branches are marked `subtree_truncated`.
- Elements that look identical (such as repeated rows) are found by their position, and the action result says `"matched_by": "position"`.
- Error codes: `PERM_DENIED`, `ELEMENT_NOT_FOUND`, `APP_NOT_FOUND`, `ACTION_FAILED`, `ACTION_NOT_SUPPORTED`, `STALE_REF`, `AMBIGUOUS_TARGET`, `WINDOW_NOT_FOUND`, `PLATFORM_NOT_SUPPORTED`, `TIMEOUT`, `INVALID_ARGS`, `SNAPSHOT_NOT_FOUND`, `INTERNAL`.
- Snapshots are saved in `~/.agentic-desktop` (set `AGENTIC_DESKTOP_HOME` to move them).

## Use it with an AI agent

Every agent needs two things: the `agentic-desktop` binary on its `PATH`, and short instructions telling it how to use the tool. The instructions go in the **project's** instruction file, so the agent reads them in that project. Paste this block there (the file name depends on the tool, see below):

```markdown
## Desktop control

Use `agentic-desktop` to see and operate desktop apps. Run it as a shell command; every command prints one JSON line.

1. `agentic-desktop list-apps` to find the exact app name.
2. `agentic-desktop snapshot --app "<name>" -i` to get elements. Each actionable element has a `ref_id` like `@s8f3k2p9:e8`.
3. Act with `click`, `type` or `set-value` using that full ref. Never shorten a ref.
4. Take a new snapshot after every action to check the result. Never reuse refs from an older snapshot after the screen changed.
5. If the error is `STALE_REF` or `AMBIGUOUS_TARGET`, take a new snapshot and pick the element again.
6. If `complete` is false, the app was only partly read. Do not assume a missing element does not exist.
7. Snapshots can contain private on-screen content. Do not paste them into public places.
```

### Claude Code

1. Install the binary (see Install) and check that `agentic-desktop version` works in the terminal you start Claude Code from.
2. Add the block above to your project's `CLAUDE.md`. To use it in every project, add it to `~/.claude/CLAUDE.md` instead.
3. To stop Claude Code asking before every call, allow the command in `.claude/settings.json`:

   ```json
   {
     "permissions": {
       "allow": ["Bash(agentic-desktop:*)"]
     }
   }
   ```

   Only do this if you are happy for the agent to click and type in your apps without asking each time.
4. Ask in plain words, for example: "Open the calculator, work out 7 times 6, and tell me the answer."

### Codex

1. Install the binary and check it from the terminal that starts Codex.
2. Add the block above to `AGENTS.md` in your project root. Codex reads `~/.codex/AGENTS.md` first, then each `AGENTS.md` from the repository root down to your current folder, so a project file works.
3. Codex runs commands inside a sandbox. Its setting `sandbox_mode` is `read-only`, `workspace-write` or `danger-full-access`, and `approval_policy` controls when it asks you. `codex exec` runs with approval `never` and a read-only sandbox by default. The tool must reach your desktop session (on Linux, the D-Bus session; on macOS, the Accessibility permission), and a sandbox may block that. If every call fails with a permission or "cannot reach the bus" error, change the settings in `~/.codex/config.toml` (or pass `--sandbox` and `--ask-for-approval`) only as far as you are comfortable with. Avoid `--dangerously-bypass-approvals-and-sandbox` (`--yolo`) unless you understand the risk.

### Antigravity

1. Install the binary and check it from the terminal Antigravity uses.
2. Add the block above to `AGENTS.md` in your project root. Antigravity reads `AGENTS.md` as well as `GEMINI.md` (reported from version 1.20.5). You can also put it in a file under `.agents/rules/` in the project (older versions used `.agent/rules/`). Each rules file is limited to 12,000 characters. For all projects, use `~/.gemini/GEMINI.md`.
3. Terminal permission is a setting in the IDE, not part of the rules file. Add `agentic-desktop` to the **Allow List Terminal Commands** setting if you want it to run without asking, or leave the policy on "Request review" to approve each call. Matching uses the start of the command, so keep the entry short.
4. Newer versions run terminal commands in a sandbox limited to your workspace and with no network. That may block the tool from reaching your desktop session. If calls fail with permission errors, check the sandbox settings.

### How sure am I?

- **Claude Code:** written from its documented behaviour.
- **Codex and Antigravity:** checked against web search results on 2026-11-03, not by running those tools. The Codex details come from OpenAI's [configuration reference](https://developers.openai.com/codex/config-reference) and [approvals and security](https://developers.openai.com/codex/agent-approvals-security) pages. The Antigravity details come from third-party guides ([thepromptshelf.dev](https://thepromptshelf.dev/blog/google-antigravity-agents-md-rules-guide-2026/), [aidevhub.net](https://aidevhub.net/blog/google-antigravity-custom-rules-tuning), [cybedefend.com](https://www.cybedefend.com/en/blog/is-google-antigravity-safe)), because I did not find Google's own documentation. Settings and file names change between versions, so confirm them in each tool's own docs.

## Privacy and safety

- Snapshots can contain anything visible in an app: messages, names, passwords typed into visible fields. Keep them out of public logs and uploads.
- The agent can click and type in any app it can see. Only point it at apps you are happy for it to operate, and keep an eye on what it does.
- The tool never sends data anywhere. Whatever your agent does with the output is up to that agent.

## Development

```bash
cargo test                                   # unit and flow tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo check -p agentic-desktop --target x86_64-pc-windows-msvc   # needs: rustup target add
cargo check -p agentic-desktop --target aarch64-apple-darwin
```

Live Linux checks need a running desktop and are ignored by default:

```bash
cargo test -p agentic-desktop-linux -- --ignored --nocapture cache_matches_slow_walk
```

Layout:

- `crates/core` holds the shared types, refs, JSON output and the `PlatformAdapter` trait. It never depends on a platform crate.
- `crates/linux`, `crates/windows`, `crates/macos` hold one backend each.
- `src` holds the command-line program.

See `CLAUDE.md` for the project rules and roadmap.

## Licence

Apache-2.0.
