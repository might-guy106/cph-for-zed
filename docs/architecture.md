# Architecture

How cph fits into Zed's extension constraints. For the *why*, see the
research reports in `research/`.

## Overview

```
Browser                    Zed                                Host
┌────────────┐    POST     ┌──────────────┐  spawn/manage    ┌────────────────────────┐
│ Competitive │───────────▶│ cph WASM ext │─────────────────▶│ cph-helper (native)    │
│ Companion   │ :27121     │ (fake LSP    │  stdio LSP       │ serve: LSP handshake   │
└────────────┘            │  registry)   │                  │        + HTTP :27121   │
                          │              │  one-shot (task) │ judge: compile+run+    │
                          │              │─────────────────▶│        verdicts        │
                          └──────────────┘                  └────────────────────────┘
```

Zed extensions (WASM) cannot listen on TCP, draw UI, add palette commands,
or ship keybindings. So all real work lives in a native binary, and the
extension is a thin shell that starts it.

## Components

### `cph` WASM extension (`src/lib.rs`, `extension.toml`)

- Registers `cph-helper` as a `[language_servers]` entry for C/C++ files
  ("fake LSP"). Zed auto-starts it when a `.cpp` opens and kills it on
  exit (`kill_on_drop`), which gives the daemon a managed lifecycle.
- Resolves the helper binary in order: `lsp.cph.binary.path` setting →
  `cph-helper` on PATH → dev checkout (`<worktree>/cph/target/debug/…`,
  detected by reading `cph/extension.toml` in the worktree) → download of
  the latest GitHub release asset into the extension work dir (24h cache).

### `cph-helper` (`crates/helper/`)

- `serve` — minimal stdio LSP (initialize/initialized/shutdown/exit, null
  for everything else) + HTTP server on `127.0.0.1:27121` in a thread.
  `POST /` writes the problem folder; `GET /health` is used for
  port-collision detection (second instance exits quietly).
- `judge [--compile-only] <file>` — compiles with `cxx`/`cxxflags` from the
  nearest `cph.toml`, runs every `NN.in` vs `NN.out` with a timeout kill,
  prints PASS/WA/RE/TLE verdicts, and on a full pass shows a bold banner
  and fires a macOS notification (`notify = false` disables).

## Flows

**New problem (Competitive Companion click):**
`POST /` → parse payload → `contests/<group>/<problem>/` (batch > 1) or
`practice/<group>/<problem>/` → `sol.cpp` from template (only if missing;
never overwritten), `NN.in`/`NN.out` refreshed, `problem.url`, then the
files open in Zed as tabs.

**Test run (`cph: test` task):**
Zed task (written into `.zed/tasks.json` by the daemon on first run) →
`cph-helper judge $ZED_FILE` → verdicts in the integrated terminal. Task
stays open (`"hide": "never"`) on both success and failure.

**Pane layout (`alt-l`):**
Zed has no pane API for extensions or the CLI, so the daemon only opens
tabs; a `workspace::SendKeystrokes` keybinding macro creates the splits
(empty panes) and moves tabs with indexed `MoveItemToPane` — deterministic,
no focus-tracking. Details in README's layout section.

## Settings and ports

| Thing | Where | Default |
|---|---|---|
| Compiler / flags / timeout / notify | `cph.toml` (walks up from the file) | `g++-16`, `-std=c++17 -O2 -Wall -Winvalid-pch`, 3000ms, on |
| CC listener port | `CPH_PORT` env | 27121 |
| Tasks | `.zed/tasks.json` (auto-written) | `cph: test`, `cph: compile`, `cph: serve` |
| Layout key | `~/.config/zed/keymap.json` | `alt-l` |

## Repo layout

```
extension.toml     # Zed manifest: id, fake-LSP registration, capabilities
src/lib.rs         # the WASM extension (binary resolution + LSP command)
crates/helper/     # native binary: main, lsp, http, problem, judge, tasks, config
templates/         # sol.cpp templates (single / multi-test)
docs/              # this folder
```
