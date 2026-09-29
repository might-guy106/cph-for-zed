# cph — Competitive Programming Helper for Zed

A Zed port of the core of [agrawal-d/cph](https://github.com/agrawal-d/cph)
(VS Code): click Competitive Companion in the browser → problem folder with
`sol.cpp` + samples appears in your project → one keybinding compiles and
runs against the samples with verdicts in the terminal.

Status: **M3 done** (receive problems + judge + release plumbing).
Dev install works today; registry submission is next.

## Troubleshooting

- **Daemon not receiving?** It only runs while Zed has a C/C++ buffer open.
  `curl localhost:27121/health` should print `"cph-helper ok"`. If not, open
  any `.cpp` (or run the `cph: serve` task) and retry.
- **Port taken by something else?** The helper exits with a log line naming
  the port; set `CPH_PORT` (daemon) and use that port in Competitive
  Companion's custom ports.
- **Compiles suddenly slow (~500ms)?** Your `cxxflags` no longer match the
  flags the precompiled `bits/stdc++.h.gch` was built with. Rebuild the PCH
  (`make pch` in the dsa folder) or drop `-Winvalid-pch` to see the reason.
- **Language server "cph" failed to start in some other folder?** The dev
  build resolves the helper at `<worktree>/cph/target/debug/cph-helper`,
  which only exists in the dev checkout. Once the extension is published,
  other folders fall back to the downloaded release binary. Until then this
  error in non-dev folders is expected and harmless.
- **Where's the log?** `zed: open log` shows spawn errors; the helper's own
  `[cph]` lines go to its language-server stderr log.

## Uninstall

Remove the dev extension from Zed's Extensions page, then optionally delete
`dsa/.zed/tasks.json` and the helper cache at
`~/Library/Application Support/Zed/extensions/work/cph/`.


## How it works

Zed extensions can't listen on TCP ports, so the Competitive Companion
receiver is a native binary (`cph-helper`) registered as a *language
server* for C/C++ — Zed starts it when a `.cpp` opens and kills it on
exit. The same binary provides the judge.

```
cph-helper serve            # LSP handshake + HTTP receiver on 127.0.0.1:27121
cph-helper judge <file>     # compile + run NN.in vs NN.out, verdicts
cph-helper judge --compile-only <file>
```

## Setup (dev)

1. `rustup target add wasm32-wasip2`
2. `cargo build -p cph-helper`
3. Zed: open your competitive-programming root folder (e.g. `dsa/`)
4. `cmd-shift-p` → **zed: install dev extension** → pick this repo folder
5. Open any `.cpp` → the daemon starts (check: `curl localhost:27121/health`)

Cold start: the daemon only starts when a `.cpp` is open, so keep a scratch
file open before the first Competitive Companion click, or run the
`cph: serve` task.

## Daily flow

1. Browser: click the Competitive Companion `+` on a problem (or on a
   contest page to grab all problems) → folder appears:
   `contests/<group>/<problem>/` (whole contest) or
   `practice/<group>/<problem>/` (single problem), with `sol.cpp`,
   `01.in`/`01.out`…, `problem.url`.
2. Write your solution in `sol.cpp`.
3. `task: spawn` → **cph: test** → verdicts in the terminal.
   Optional keybinding for `~/.config/zed/keymap.json`:

```json
{
  "context": "Workspace",
  "bindings": {
    "alt-b": ["task::Spawn", { "task_name": "cph: test" }]
  }
}
```

Re-parsing a problem refreshes tests but never overwrites your `sol.cpp`.

## Three-pane layout (one keypress)

When a problem arrives, the daemon opens `sol.cpp`, `01.in` and `01.out` as
tabs (in that order, last one active). Press `alt-l` to arrange them into the
classic contest layout — sol left, input right-top, output right-bottom:

```
 alt-l  →  [select last tab (01.out), move it to a bottom split]
        →  [focus returns to the top pane; select its last tab (01.in)]
        →  [01.in moves to a right split]
        →  [focus lands on sol.cpp]
```

This uses Zed's `workspace::SendKeystrokes` macro action plus two
`pane::SplitAndMove*` bindings. The daemon only opens the tabs; the keypress
does the arranging (Zed has no pane API for extensions/CLI). The keymap
entries (already added to `~/.config/zed/keymap.json`):

```json
"alt-l": ["workspace::SendKeystrokes", "ctrl-2 ctrl-alt-m cmd-1 ctrl-1 ctrl-alt-m ctrl-alt-down cmd-1"],
"ctrl-alt-m": ["workspace::MoveItemToPaneInDirection", { "direction": "right" }],
"ctrl-alt-down": "pane::SplitAndMoveDown",
```


## Settings — `cph.toml` at the project root (all optional)

```toml
cxx        = "g++-16"
cxxflags   = "-std=c++17 -O2 -Wall -Winvalid-pch"
timeout_ms = 3000
```

Note: `cxxflags` must match the flags the precompiled `bits/stdc++.h.gch`
was built with, or GCC ignores it and compiles slowly. If you change them,
rebuild the PCH (`make pch` from the dsa Makefile).

## Verdicts

`✓` pass · `WA` wrong answer (first differing line shown) · `RE` nonzero
exit or signal · `TLE` over `timeout_ms` (default 3s) · compile errors
shown raw. Output compare: whole-text trim, line count must match, each
line trimmed (trailing whitespace / CRLF / final newline don't matter).
Exit code 0 iff all pass.

## Development

```bash
cargo build -p cph-helper     # native helper
cargo test -p cph-helper      # unit + end-to-end tests
cargo build --target wasm32-wasip2   # the extension (Zed builds this itself)
```

After changing `src/lib.rs` or `extension.toml`: palette →
**zed: install dev extension** again. Helper changes need no reinstall —
just rebuild.

Layout, milestones and research: `PLAN.md`, `research/`.
