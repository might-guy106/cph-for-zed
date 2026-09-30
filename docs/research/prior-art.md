# Prior Art: Zed Extension for Competitive Programming (CPH-like)

Research date: 2026-09-28. Scope: existing CP extensions for Zed, reference extensions for binary-download/daemon/tasks/commands patterns, freshness against current `zed_extension_api`.

---

## 1. Verdict: Existing CP Extensions for Zed

**No mature, registry-published CP extension exists. The niche is effectively open.** Three relevant artifacts:

### 1a. `ashusevim/cph-zed` — the only direct prior art (GitHub-only, NOT in registry)
- Repo: https://github.com/ashusevim/cph-zed (created 2026-01-18, single commit 2026-01-20, **3 stars**)
- Registry check: **NOT present** in `zed-industries/extensions` `extensions.toml` (grep for `cph|competitive|codeforces|atcoder|leetcode` = 0 matches); `https://zed.dev/extensions/cph` → **404**. Its README's "Install from Zed Extensions" instructions are aspirational.
- Architecture (from source, `src/lib.rs` + `extension.toml`, 580 LOC total):
  - `zed_extension_api = "0.5.0"`, WASM extension with **only 4 slash commands** (`/cph-fetch`, `/cph-run`, `/cph-test`, `/cph-server`).
  - `/cph-fetch <json>` parses pasted competitive-companion JSON (`problem.rs`) and generates files (`file_generator.rs` with hardcoded C++/Python/Rust/Java templates).
  - `/cph-run` and `/cph-test` **do not run anything** — they return static text telling the user to run `g++ ... && ./solution < tests/1.in` / a `diff` loop in their own terminal.
  - The actual localhost HTTP receiver is a **separate native binary `cph-server`** (default port 10045) that the user must `cargo install` and run manually. **`cph-server` does not exist on crates.io** (API returns nothing) — so the auto-fetch flow is currently broken/unfinished.
  - No compilation, no verdicts, no submission. Essentially a skeleton.
- Port note: competitive-companion POSTs to default ports `[1327, 4244, 6174, 10042, 10043, 10045, 27121]` (see `hosts.ts` below); 27121 = VS Code CPH, 10045 = CP Editor. cph-zed chose 10045.

### 1b. `TheComputerM/lazycph` — CLI tool with documented Zed integration (not an extension)
- https://github.com/TheComputerM/lazycph (PyPI `lazycph`, actively versioned, e.g. v1.2.2)
- Python TUI/CLI; Zed "integration" = user adds a `tasks.json` entry (`"command": "uvx", "args": ["lazycph", "$ZED_FILE"]`) plus a keybinding to `task::Spawn`. No WASM extension involved. This is the pragmatic workaround the community actually uses today.

### 1c. `harpy-cp` (PyPI) — MCP server usable from Zed's Agent panel
- https://pypi.org/project/harpy-cp/ — exposes MCP tools (`setup_problem`, `oracle_generate_tests`, `sync_cph`, `test_solution`) to any MCP-capable editor incl. Zed; syncs to VS Code CPH on port 27121. Agent-oriented, not a judge UI.

**Community demand signal**: r/ZedEditor thread "Competitive Programming On Zed" (https://www.reddit.com/r/ZedEditor/comments/1s01uk4/) — users miss CPH; one reply: *"u have to make a language server for cp helper and one extension"* (the fake-LSP daemon pattern, confirmed below as the standard trick).

---

## 2. Reference Extensions by Pattern

| Pattern | Extension | Exact path / link | Freshness |
|---|---|---|---|
| (a) Download prebuilt binary from GH Releases + cache + spawn (LSP) | **zed-extensions/odin** | `src/odin.rs` — https://github.com/zed-extensions/odin/blob/main/src/odin.rs (+ `src/logic.rs`) | pushed 2026-08-13; `extension.toml` v0.3.16 |
| (a′) Same pattern, minimal canonical sample | **zed-industries/zed `extensions/test-extension`** | https://github.com/zed-industries/zed/blob/main/extensions/test-extension/src/test_extension.rs | last touched 2026-09-17 (tracks zed main; the API's own test fixture) |
| (a″) Same pattern, also registers fake-LSP-for-all-languages | **xhyrom/zed-discord-presence** | `src/discord_presence.rs` — https://github.com/xhyrom/zed-discord-presence/blob/main/src/discord_presence.rs | pushed 2026-08-12; 465★ |
| (b) Long-running background daemon from an extension | **xhyrom/zed-discord-presence** (fake LSP trick) | `extension.toml` registers `[language_servers.discord_presence]` for ~150 languages — https://github.com/xhyrom/zed-discord-presence/blob/main/extension.toml | 2026-08-12 |
| (b′) Daemon via MCP/context-server (officially supported) | **zed-extensions/postgres-context-server** | `src/postgres_model_context.rs` — https://github.com/zed-extensions/postgres-context-server/blob/main/src/postgres_model_context.rs | 2026-06-07; 207★; but MCP-server extensions are **slated for deprecation** in favor of the official MCP registry |
| (c) Tasks/runnables contributed by an extension | Only via **debug adapters**: build-task templates + `dap_locator_create_scenario` | odin `src/odin.rs` fns `dap_locator_create_scenario` (builds `BuildTaskTemplate`/`TaskTemplate`) + `[debug_locators.odin-main]` in extension.toml | 2026-08-13 |
| (c′) Plain `task: spawn` entries | **NOT possible from extensions.** WIT API (`since_v0.8.0/extension.wit`) has no task-provisioning export. Tasks come only from user `~/.config/zed/tasks.json`, project `.zed/tasks.json`, language-provided runnables (tree-sitter–based, e.g. Rust `#[test]`), or debug build tasks. | WIT: https://github.com/zed-industries/zed/blob/main/crates/extension_api/wit/since_v0.8.0/extension.wit (exports list: language-server-*, slash-command, context-server, dap-*, docs-indexing — nothing else) | current main |
| (d) Command-palette commands | **NOT possible.** Open issue zed-industries/zed#18043 ("Command palette/projects/text buffer extension API"), staff response 2025-07-23: extension API work deferred until core stabilizes. Workarounds: slash commands, tasks+keybindings, fake LSP. | https://github.com/zed-industries/zed/issues/18043 | issue still open |
| (d′) Closest available: slash commands (Assistant panel) | `[slash_commands.*]` in extension.toml + `run_slash_command` impl. Official `slash-commands-example` was **removed from zed repo 2026-03-31** (PR #52835); docs page is now the reference. cph-zed itself is a working 0.5.0 example. | https://zed.dev/docs/extensions/slash-commands ; cph-zed `src/lib.rs` | docs current |

### Key API-surface facts (verified against zed main, 2026-09)
- Full extension capability = what the WIT `world extension` exports: `language-server-command`, `run-slash-command`, `context-server-command`, `get-dap-binary`/`dap-locator-*`, docs indexing. No hooks for: command palette, editor UI, buffer access, task templates, HTTP **listening**.
- **Capability system** (https://zed.dev/docs/extensions/capabilities): extensions declare `[[capabilities]]` in `extension.toml`; kinds are `process:exec` (spawn child processes via `zed_extension_api::process::Command`), `download_file` (host/path-restricted), `npm:install`. Users can restrict via `granted_extension_capabilities` setting. There is **no network-listen capability** — WASM extensions cannot bind a localhost port.
- WASM sandbox is `wasm32-wasip2`; `zed_extension_api` latest = **0.7.0** (crates.io); WIT dirs exist up to `since_v0.8.0`; Zed 0.192.x accepts API ≤ 0.6.0. cph-zed's 0.5.0 compiles today but the examples above target 0.6.0+.
- Extensions' working dir is a private per-extension dir; files written there persist (that's how binary caching works). Worktree file writes go through std `fs` (allowed) but relative paths resolve to the extension dir, not the project.

---

## 3. "Steal This Pattern" Recommendations

### 1. Binary download + version cache → copy **odin**'s two-layer design
`language_server_binary_path()` → checks settings override → `worktree.which()` PATH escape hatch → in-memory `cached_binary` → on-disk version dir → 24h rate-limit record file (`LAST_RELEASE_CHECK_FILE`) → `zed::latest_github_release(repo, GithubReleaseOptions{require_assets:true, pre_release:false})` → pick per-platform asset → `zed::download_file(url, dir, DownloadedFileType::Zip|GzipTar)` → `zed::make_file_executable()` → `zed::set_language_server_installation_status(...CheckingForUpdate/Downloading)` for UI feedback.
Files: https://github.com/zed-extensions/odin/blob/main/src/odin.rs (host/IO) + https://github.com/zed-extensions/odin/blob/main/src/logic.rs (pure decision logic, unit-testable — nice separation worth copying). Minimal variant: test-extension (`test_extension.rs` lines ~59-120) does the same in ~60 lines with no frills: https://github.com/zed-industries/zed/blob/main/extensions/test-extension/src/test_extension.rs
**Use for**: downloading a companion `cph-server` binary (or `cf-tool`/`oj` for submission) from your own GitHub releases on first activation.

### 2. Localhost HTTP daemon (the Competitive Companion receiver) → **fake-LSP pattern from zed-discord-presence**
Since WASM can't listen on sockets, ship a tiny native Rust binary that (a) speaks just enough LSP over stdio to stay alive and (b) runs the real HTTP server on a companion port (10045/27121/custom). Register it in `extension.toml` as `[language_servers.cph_server] languages = ["C", "C++", "Rust", "Python", ...]` so **Zed auto-starts it whenever a CP file opens and kills it on exit** — free lifecycle management, no user setup. Download it with pattern #1. Precedent (465★, works in practice): https://github.com/xhyrom/zed-discord-presence (`src/discord_presence.rs` download flow + `lsp/` native server; `extension.toml` registers against ~150 languages). Alternative officially-sanctioned daemon: MCP `context_server_command` returning a `zed::Command` (postgres-context-server: https://github.com/zed-extensions/postgres-context-server/blob/main/src/postgres_model_context.rs) — but it only runs while the Agent panel uses it and the MCP-extension mechanism is marked for deprecation, so the fake-LSP route fits CPH better.
**Alternative without any LSP shim**: declare `[[capabilities]] kind = "process:exec" command = "cph-server" args = ["**"]` and spawn the daemon directly via `zed_extension_api::process::Command` — but the WASM extension has no startup hook, so something (slash command / LSP activation) must trigger the spawn; the fake-LSP auto-start is strictly better UX.

### 3. Compile+run+verdict → **tasks.json templates written by the server + `task::Spawn` keybinding flow (lazycph model), not extension tasks**
Extensions can't inject tasks. Have your native server **write a `.zed/tasks.json`** into the problem folder (or document a global one) with entries like `{"label": "CP: test", "command": "g++ -std=c++20 -O2 $ZED_STEM -o /tmp/sol && for f in tests/*.in; ...", "reveal": "always"}` using `$ZED_FILE`/`$ZED_STEM` variables, then tell the user to bind `task::Spawn` (exactly how lazycph does it: https://github.com/TheComputerM/lazycph#integration-with-zed). Verdict display options, in order of feasibility: (1) task output in the integrated terminal (works today, zero API); (2) slash-command output sections rendered in the Agent panel (cph-zed's `/cph-test` approach, but slash commands **cannot run processes unless you add the process:exec capability and spawn `sh -c ...` yourself** — doable: return formatted `SlashCommandOutput` with PASS/FAIL per test); (3) debug-adapter build tasks (`dap_locator_create_scenario`, odin) only if you want F5-debugging of the solution, which also gets you a `task: spawn`-visible build task for free.
Task format docs: https://zed.dev/docs/tasks ; tree-sitter runnable blog: https://zed.dev/blog/zed-decoded-tasks

### 4. User-facing "commands" → **slash commands now, and name them identically to future palette commands**
With #18043 unresolved, `[slash_commands.*]` + `run_slash_command` is the only extension-triggered UX. cph-zed's `extension.toml` + `src/lib.rs` is a minimal working template (https://github.com/ashusevim/cph-zed/blob/main/extension.toml) — but note its `/cph-run` only prints instructions; with `process:exec` capability you can actually execute and report. Keep the official docs example bookmarked since the in-repo `slash-commands-example` was deleted 2026-03-31 (https://zed.dev/docs/extensions/slash-commands).

### 5. Submission to judges → **shell out to existing CLIs (`cf-tool`, `oj`) via process:exec rather than reimplementing**
No Zed-specific prior art exists for submission; the pragmatic route (matching what the VS Code CPH in `./repo/cph` does via its own server, and what lazycph/harpy-cp do) is to invoke `cf tool` / `oj submit` as child processes from the native server component, surfacing results via task output or slash-command sections. Competitive-companion POST format + ports reference (your server must accept POST `/` on one of the default ports or a user-configured custom port): https://github.com/jmerle/competitive-companion/blob/master/src/hosts/hosts.ts and format docs at https://github.com/jmerle/competitive-companion#custom-tools.

---

## Contradictions
- cph-zed README claims "Install from Zed Extensions (Recommended)" but it is absent from `extensions.toml` and zed.dev → README is inaccurate (direct check, high confidence).
- `crates/extension_api/README.md` compatibility table stops at `zed_extension_api 0.6.0` / Zed 0.192.x, yet crates.io already serves **0.7.0** and the zed repo carries a `since_v0.8.0` WIT — the README table lags reality; treat 0.6.0 as the safe max for broad install-base compatibility.
- Zed docs (https://zed.dev/docs/extensions/mcp-extensions) say MCP-server extensions "plan to deprecate" in favor of the official MCP registry, while the same docs still teach building them — relevant only if considering pattern (b′).

## Missing evidence
- No benchmark/UX data on how many users run cph-zed (3★ repo, zero registry presence ⇒ effectively none).
- Did not verify whether a WASM extension's `process:exec`-spawned daemon survives Zed restarts/reloads (fake-LSP route sidesteps this; flagged as inference, not documented fact).
- `cph-server` (cph-zed's companion) has no published crate or source found — its actual implementation could not be reviewed.

## Sources
- Kept: ashusevim/cph-zed (https://github.com/ashusevim/cph-zed) — the direct prior art, warts and all
- Kept: zed-extensions/odin `src/odin.rs`, `src/logic.rs`, `extension.toml` — best binary-download + debug-task exemplar, 2026-08
- Kept: xhyrom/zed-discord-presence (https://github.com/xhyrom/zed-discord-presence) — proven fake-LSP daemon pattern
- Kept: zed-industries/zed `extensions/test-extension` — canonical minimal download/capability fixture, tracks main
- Kept: WIT API surface (https://github.com/zed-industries/zed/blob/main/crates/extension_api/wit/since_v0.8.0/extension.wit) — ground truth on what extensions can/cannot do
- Kept: Extension capabilities doc (https://zed.dev/docs/extensions/capabilities) — process:exec/download_file/npm:install
- Kept: Issue #18043 (https://github.com/zed-industries/zed/issues/18043) — command palette impossible; staff position + workarounds
- Kept: zed-extensions/postgres-context-server — official-style context-server daemon reference
- Kept: TheComputerM/lazycph (https://github.com/TheComputerM/lazycph) — tasks.json integration pattern users actually run
- Kept: jmerle/competitive-companion `hosts.ts` — port list & POST contract
- Kept: r/ZedEditor CP thread (https://www.reddit.com/r/ZedEditor/comments/1s01uk4/) — demand signal
- Rejected: zedhub.dev docs mirror — stale duplicate of zed.dev docs
- Rejected: markaicode.com MCP setup article — SEO content farm, partially inaccurate
- Rejected: harpy-cp (PyPI) — agent/MCP-oriented, different product shape; noted only for awareness

## Next steps
1. Prototype the fake-LSP `cph-server` (single Rust binary: stdio-LSP keepalive + HTTP POST receiver on 10045/27121 + file generator) and validate Zed auto-start on `.cpp` open.
2. Verify `process:exec` behavior from slash commands on a dev extension (can `/cph-test` actually spawn `g++` and stream verdicts into `SlashCommandOutput`?).
3. If debuggability matters, spike a `debug_locator` (odin pattern) so F5 builds+runs the current problem under LLDB with test input piped in.
