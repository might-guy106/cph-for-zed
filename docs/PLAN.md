# Plan: `cph` — a Competitive Programming Helper for Zed

## 1. Goal

Replicate the core of the VS Code extension [agrawal-d/cph](https://github.com/agrawal-d/cph) for Zed:

1. Click Competitive Companion in the browser → problem folder with `sol.cpp` (from template) + sample tests appears, file opens in Zed.
2. One keybinding → compile and run against samples → verdicts (PASS / WA / RE / TLE / CE) with an output diff, shown in the integrated terminal.

Research reports (full details, source-cited): `research/cph-features.md`, `research/zed-api.md`, `research/prior-art.md`.

## 2. What the research established

**Zed extension hard limits** (from `zed-api.md`, verified against the WIT API and Zed v1.21.0 source):
- No TCP listen, no custom UI (no webviews/panels), no palette commands, no shipped keybindings, no dynamic task API.
- Extensions CAN: spawn blocking child processes (`process:exec` capability), download binaries from GitHub releases, register language servers (Zed-owned long-lived processes), define assistant slash commands. Static `tasks.json` in a language dir exists but only for languages the extension itself defines — verified unusable for built-in C++ (see §5).
- Verdict surface: no custom UI → verdicts go to the integrated terminal via tasks.

**The pattern that makes this work** (from `prior-art.md`): the "fake LSP" — register a native helper binary as a `[language_servers]` entry for C/C++ files. Zed auto-starts it when a `.cpp` opens and kills it on exit. The binary speaks a minimal LSP handshake over stdio and runs the real HTTP receiver in a thread. Proven in the wild (zed-discord-presence, 465★).

**CPH internals worth copying** (from `cph-features.md`):
- Port **27121** is CC's default CPH port; CC POSTs one JSON object per problem; no extension-side config needed.
- Compile appends `-D DEBUG -D CPH`; effective TLE = user setting, default **3000 ms**; **no MLE** (memory limit never enforced); correctness = line-count match + per-line whitespace trim; verdicts = Passed/Failed/Timed-out/CE.
- CPH has no stress testing, no AI, no contest concept. The results webview is the only truly non-portable piece.
- The Codeforces submit handshake (browser add-on polls `localhost:27121/getSubmit`) is editor-agnostic — out of scope for v1 per review, but an easy future add since our daemon owns :27121.

## 3. Architecture

```
Browser                    Zed (1.21)                          Host
┌────────────┐    POST     ┌─────────────────┐  spawn/manage  ┌──────────────────────────┐
│ Competitive │───────────▶│  cph WASM ext   │───────────────▶│ cph-helper (native Rust) │
│ Companion   │ :27121     │  fake-LSP reg.  │  stdio LSP     │  serve = LSP handshake   │
└────────────┘            │                 │                │        + HTTP :27121     │
                          │                 │  one-shot      │  judge = compile+run+    │
                          │                 │───────────────▶│        verdicts          │
                          └─────────────────┘                └──────────────────────────┘
```

Two components in one cargo workspace under `cph/`:

- **`crates/extension`** — `cdylib` → `wasm32-wasip2`, `zed_extension_api = 0.7.0`. Registers the fake LSP and (for releases) downloads the helper binary. Thin by design: all logic lives in the helper, so iteration on the judge/receiver doesn't need Zed's slow "reinstall dev extension" loop.
- **`crates/helper`** — normal native binary (`aarch64-apple-darwin`; cross-compile for releases). Subcommands:
  - `cph-helper serve` — minimal LSP over stdio (initialize/initialized/shutdown/exit; respond `null` to everything else) + HTTP server on 27121: `POST /` (CC problem) + `GET /health`. On `EADDRINUSE`, health-check the port: if another `cph-helper` owns it, exit quietly (first instance wins); else log a clear warning.
  - `cph-helper judge <file>` — one-shot: compile (`g++-16` default) + run each `NN.in` vs `NN.out` + print verdicts + token diff. No daemon needed. Exit 0 iff all pass.

## 4. UX design (within Zed's limits)

| CPH feature (VS Code) | Zed equivalent |
|---|---|
| Sidebar webview with verdicts + diff | Terminal task output: colored `✓/✗` per test, expected-vs-got diff, `3/5 passed` summary. |
| `Ctrl+Alt+B` run testcases | Task `cph: test` → user binds `["task::Spawn", {"task_name":"cph: test"}]` (one snippet in README). |
| Compile-only command | Task `cph: compile`. |
| Companion POST → file created | Helper daemon (fake-LSP) — starts when a `.cpp` is open. |
| Settings `cph.*` | `cph.toml` at project root (`cxx`, `flags`, `timeout_ms`, `port`, template paths). |
| Add/edit tests in webview | Tests are plain `NN.in`/`NN.out` files — edit directly, re-run. |

**Cold start (review feedback):** the fake-LSP only starts when a C/C++ buffer is open, but Competitive Companion needs the daemon *already* listening to create the first `sol.cpp`. Two handles, both documented in README + quickstart:
- Keep any scratch `.cpp` open in the worktree before the first click (`dsa/temp.cpp` already exists for exactly this).
- Or run task `cph: serve` — starts the daemon as a long-running terminal task, no `.cpp` needed.
Port collisions are benign: the second instance health-checks :27121 and exits quietly if a `cph-helper` already owns it.

**Folder layout** (created by the receiver, relative to the Zed worktree root):
```
contests/<group-slug>/<problem-slug>/   # when CC parsed a whole contest (batch.size > 1)
practice/<group-slug>/<problem-slug>/   # single problem
  sol.cpp        # from template, only if missing — re-parse never overwrites code
  01.in 01.out … # samples, refreshed on re-parse
  problem.url    # source URL (quick way back to the statement)
```
Problem slug = CC's `languages.java.taskClass` when present (filename-safe), else slugified name. Template ships in the repo and is user-editable (`templates/single.cpp`, `templates/multi.cpp`; multi chosen when `testType == "multiNumber"`). Compile defaults: `g++-16 -std=c++17 -O2 -Wall -Winvalid-pch` (matches the existing system-wide `bits/stdc++.h.gch` on this machine), overridable in `cph.toml`.

## 5. Tasks wiring — resolved against Zed v1.21.0 source

The two research reports disagreed, so this was checked directly in `crates/extension_host/src/extension_host.rs` at tag `v1.21.0` (the installed version):

- Plugin language dirs DO support `tasks.json`: `load_plugin_language()` loads `language_path.join(TaskTemplates::FILE_NAME)` optionally (`.ok()`) and wraps it in `ContextProviderWithTasks`. The mechanism exists.
- **But** `config.toml` is mandatory in the same function: `futures::try_join!(config, …)?` — a tasks-only dir (no `config.toml`) fails the whole language load.
- **And** duplicate language names are dropped: `if !registered { continue; }` — first registration wins, and built-in C++ registers before extensions. Redefining "C++" from our extension gets skipped.
- (Also confirmed the zed-api report's example was inaccurate: zed-extensions/csharp ships no `tasks.json`.)

**Conclusion: extension-shipped tasks cannot attach to the existing C++ language.** The fallback is the design:

- On first `POST /` (or first `judge` run), the helper ensures `.zed/tasks.json` exists in the worktree root with `cph: test`, `cph: compile`, `cph: serve` entries. If the file already exists: append only missing `cph:*` entries via a jsonc-tolerant parse; if unparseable, write nothing and print a ready-to-paste snippet to the log. Never clobbers user content.
- README documents the one-line keybinding: `"alt-b": ["task::Spawn", { "task_name": "cph: test" }]`.

## 6. Repo layout + toolchain

```
cph/
  PLAN.md                 # this file
  research/               # the three reports (kept)
  extension.toml          # id "cph", [language_servers.cph]
  Cargo.toml              # ROOT package = extension cdylib + workspace
                          # (default-members = ["."]: Zed's dev build parses
                          # [package].name and runs plain `cargo build` here,
                          # so the helper must stay out of the default build)
  src/lib.rs              # the WASM extension
  crates/helper/          # native binary (own package)
    src/{main,serve,lsp,http,judge,problem,tasks}.rs
  templates/{single,multi}.cpp
  tests/                  # helper unit + integration tests (cargo test)
```

- Toolchain present: rustc 1.97.1 (rustup). Need: `rustup target add wasm32-wasip2` (note: wasip2, not wasip1 — older docs are stale).
- Dev loop: `cargo build`/`cargo test` for the helper (fast, no Zed); palette → `zed: install dev extension` → pick `cph/` for extension changes; logs via `zed: open log` / `zed --foreground`.
- Distribution (M3): GitHub releases with per-OS helper binaries (extension downloads via `zed::download_file`, odin pattern); PR to `zed-industries/extensions` (git submodule + `extensions.toml` entry). Registry currently has no CP extension — niche is open.

## 7. Milestones

| # | Scope | Acceptance |
|---|---|---|
| M1 | Workspace scaffold; `extension.toml` + `lib.rs` fake-LSP; helper `serve` (LSP handshake + `POST /` receiver writing folders from templates + `GET /health` + EADDRINUSE quiet-exit); `.zed/tasks.json` writer | Dev-install in Zed; open `temp.cpp` → daemon up; `curl` a CC payload → correct folder/files + tasks.json; second instance exits quietly; Zed log clean |
| M2 | `judge` subcommand: compile + run + verdicts + token diff + TLE (3s default) + CE handling; keybinding snippet in README | End-to-end: CC click on a real problem → folder → `cph: test` → verdicts in terminal; wrong solution → WA + diff; infinite loop → TLE; exit codes correct; `cargo test` green |
| M3 | Polish: README (setup + keybindings + settings + cold-start), release CI (helper binaries), registry PR; optional: `/cph-*` slash commands, custom checker (Python exit-code), debug adapter (F5 with test input) | Docs complete; release artifacts build; PR submitted |

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Cold start: no `.cpp` open → daemon not running → first CC POST lost | Documented UX (keep scratch `.cpp` open; `cph: serve` task); health-check + quiet exit on port collision |
| tasks.json merge into an existing user file | Create-if-missing; jsonc-tolerant append of only `cph:*` entries; on parse failure write nothing + print snippet |
| Zed rejects/suffocates a minimal LSP server | discord-presence precedent; implement correct initialize/shutdown; verify in M1 |
| Port 27121 collision (e.g. VS Code CPH also running) | `cph.toml` port setting; EADDRINUSE → health-check → quiet exit if ours |
| Daemon lifecycle (Zed kills on window close) | Acceptable — contests happen inside Zed; helper restarts on next `.cpp` open |
| Zed auto-installs wasm target / cargo requirements | `rustup target add wasm32-wasip2` documented; dev-install handles the rest |
| `.prob` incompatibility with VS Code CPH ecosystem | Accepted: our layout is file-based and simpler |

## 9. Out of scope (v1)

Custom results UI (impossible in Zed), **submission to judges** (dropped in review — the cph-submit handshake stays documented as a future add), stress testing/brute-force compare, AI features, Kattis `submit.py`, MLE enforcement (CPH doesn't have it either), multi-platform dev (helper code is cross-platform, but only `aarch64-apple-darwin` is built/tested in M1–M3).

## 10. Decisions I need from you

1. **Folder layout**: proposed per-problem folders under `contests/` + `practice/` (vs CPH's flat workspace + `.cph/*.prob`). Recommended as proposed — matches how you described using `dsa/`.
2. **Extension id** `cph` (free in the Zed registry) — ok, or prefer another name to avoid confusion with the abandoned `ashusevim/cph-zed`?
3. **Default compiler config** in `cph.toml`: `g++-16 -std=c++17 -O2 -Wall -Winvalid-pch` (matches your current setup) as shipped default — ok?
