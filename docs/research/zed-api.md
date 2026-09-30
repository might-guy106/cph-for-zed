# Zed Extension API — Capability Map for a Competitive-Programming Helper

Target: Zed **1.21.0** (macOS), `zed_extension_api` crate **0.7.0** (latest on crates.io, published 2025-09-12; WIT ABI versions 0.6.0–0.8.0 accepted by current Zed hosts).

## Summary

Zed extensions are Rust→WASM (`wasm32-wasip2`) plugins sandboxed to their own work dir, plus a declarative `extension.toml` + static files. There is **no command-palette/menu/UI API** and **no TCP-listen or arbitrary-fs API in the WASM sandbox**; the only ways to run code on the host are (a) block-until-done child processes (`process:exec` capability), and (b) Zed-managed long-lived stdio processes (language servers, MCP context servers, debug adapters). A native helper binary shipped by the extension *can* listen on TCP once Zed spawns it (it is a normal host process), but the extension WASM itself cannot. Tasks are contributed **only** as static `tasks.json` inside a language directory — no dynamic/task-trait API exists.

## Capability matrix (for this project's needs)

| Need | Verdict | How / exact API |
|---|---|---|
| Spawn background daemon (HTTP listener) | **PARTIAL** | WASM can only `process::Command::output()` — host **waits for process exit** (`run-command` in WIT; host impl `util::command::new_command(..).output().await`). A blocking listener would hang the extension call. Workarounds: (1) register the binary as a `[language_servers]`/`[context_servers]` entry so **Zed spawns & manages it as a long-lived stdio process** (the native binary may then also open a TCP listener — nothing prevents it); (2) fire-and-detach via `sh -c 'nohup daemon &'` through `run-command` (unmanaged; survives workspace close/Zed quit — *researcher inference* from host impl: no kill-on-drop/process-group kill). |
| Contribute runnable tasks ("task: spawn", inline runnables) | **SUPPORTED (static only)** | Ship `languages/<lang>/tasks.json` in the extension. Loaded via `load_plugin_language` (`TaskTemplates::FILE_NAME = "tasks.json"`) and wrapped in `ContextProviderWithTasks`. They appear in the `task: spawn` modal when a buffer of that language is active; `"tags": [...]` binds them to inline runnable indicators. **No dynamic task generation**: `ContextProvider::associated_tasks` is not exposed over WIT (confirmed in zed discussion #56280; current WIT has no `provide-tasks` export). |
| Command-palette commands | **NOT SUPPORTED** | `Extension` trait (19 methods) and `extension.wit` exports contain no `run_command`/`execute_command`/palette hook. Manifest has no `commands` section. Closest substitutes: tasks, assistant slash commands (`[slash_commands]` + `run_slash_command`), LSP code actions via a language server. |
| Create files in user's project | **PARTIAL** | No worktree-write API: `worktree` resource exposes only `id`, `root-path`, `read-text-file`, `which`, `shell-env` (read-only). WASI preopen is limited to the extension's own work dir. Writes to the project must go through (a) `run-command` with `process:exec` capability (e.g. `sh -c 'cat > $ZED_WORKTREE_ROOT/…'`), (b) a task the user runs, or (c) an LSP workspace/applyEdit from your language server. |
| Listen on TCP port (from WASM) | **NOT SUPPORTED** | WIT imports are `context-server, dap, github, http-client, platform, process, nodejs`. `http-client` has only outbound `fetch`/`fetch-stream`. No socket/listen API. (The spawned native binary may listen — see daemon row.) |
| Download helper binary | **SUPPORTED** | `zed::download_file(url, path, DownloadedFileType::{Gzip,GzipTar,Zip,Uncompressed})` → saved/extracted under the extension work dir; `zed::make_file_executable(path)`; helpers `latest_github_release`, `github_release_by_tag_name`, `current_platform() -> (Os, Architecture)`. Requires `download_file` capability in manifest. |
| Custom UI (panels, webviews, decorations) | **NOT SUPPORTED** | No UI surface in WIT/trait. Extensions can only influence labels (`label_for_completion`, `label_for_symbol`), themes, icon themes. |
| Env vars / PATH lookup | **SUPPORTED (via host calls)** | `worktree.shell_env() -> EnvVars`, `worktree.which(binary_name)`. Direct `std::env::var` in WASM does **not** see the user env (WASI ctx sets only `PWD`, `RUST_BACKTRACE` — `build_wasi_ctx` in `wasm_host.rs`). |
| Read project files | **SUPPORTED** | `worktree.read_text_file(path)` (worktree-relative). WASI fs itself is scoped to the extension work dir only. |
| Keybindings shipped by extension | **NOT SUPPORTED** | No manifest field, no loader code. Users must bind keys themselves (e.g. `["task::Spawn", {"task_name": …}]`). |
| Language server | **SUPPORTED** | `[language_servers.<id>]` + `language_server_command()` returning `zed::Command`; Zed owns lifecycle (start on project open, stop on close). |
| MCP / context server | **SUPPORTED** | `[context_servers.<id>]` + `context_server_command()`; long-lived Zed-managed stdio process. (Note: Zed plans to deprecate MCP extensions in favor of the official MCP registry — tracking issue zed#59351.) |
| Debug adapter | **SUPPORTED** | `[debug_adapters.<name>]` (+ optional `schema_path`) and `[debug_locators.<name>]`; trait `get_dap_binary`, `dap_request_kind`, `dap_config_to_scenario`, `dap_locator_create_scenario`, `run_dap_locator`. |

## 1. `extension.toml` — full schema

From `crates/extension/src/extension_manifest.rs` (`ExtensionManifest`, serde-parsed; unknown fields tolerated):

```toml
# --- required ---
id = "my-cph"                # unique id; lowercase-with-dashes convention; used as work-dir name
name = "My CPH"              # display name
version = "0.1.0"            # semver
schema_version = 1           # current = 1 (0 = legacy extension.json mapping)
# --- optional metadata ---
description = "..."
repository = "https://github.com/you/my-cph"
authors = ["You <you@example.com>"]
# --- optional capability sections (all serde-default) ---
themes = ["themes/x.json"]           # Vec<RelPathBuf>
icon_themes = ["icons/x.json"]       # Vec<RelPathBuf>
languages = ["languages/mylang"]     # Vec<RelPathBuf> — dirs with config.toml (+ tasks.json, *.scm)
snippets = "snippets.json"           # ExtensionSnippets: single path OR list of paths
capabilities = [                     # Vec<ExtensionCapability>, serde tag = "kind"
  { kind = "process:exec", command = "*", args = ["**"] },      # args support "*" (one arg) and "**" (rest) wildcards
  { kind = "download_file", host = "github.com", path = ["**"] },
  { kind = "npm:install", package = "*" },
]

[lib]                                # LibManifestEntry { kind: Option<Rust>, version: Option<Version> } — filled by Zed at build time; you normally omit it
[grammars.mygrammar]                 # { repository = "…", rev|commit = "…", path? = "…" }
[language_servers.myls]              # { languages = ["MyLang"] } (+ deprecated `language`, + `language_ids`, `code_action_kinds`)
[context_servers.mymcp]              # ContextServerManifestEntry — currently empty struct
[slash_commands.mycmd]               # { description = "…", requires_argument = bool }
[debug_adapters.mydap]               # { schema_path? = "debug_adapter_schemas/mydap.json" }
[debug_locators.mylocator]           # empty struct
[language_model_providers.myprov]    # { name = "…", icon? = "…" }
```

**There is NO** `commands`, `tasks`, `keymap`, or `menus` section. Tasks come from static `tasks.json` files placed inside each language directory (`languages/<lang>/tasks.json`), discovered by filename, not declared in the manifest. (`AgentServerManifestEntry` exists in the same source file for external agent servers but is **not** a field of `ExtensionManifest`.)

## 2. `Extension` trait — all 19 methods (zed_extension_api 0.7.0)

Required: `new()`. Provided (override as needed):

- Language servers: `language_server_command`, `language_server_initialization_options`, `language_server_workspace_configuration`, `language_server_additional_initialization_options`, `language_server_additional_workspace_configuration`
- Labels: `label_for_completion`, `label_for_symbol`
- Slash commands (assistant): `complete_slash_command_argument`, `run_slash_command` → returns `SlashCommandOutput` rendered in the Agent panel
- MCP: `context_server_command`, `context_server_configuration`
- Docs: `suggest_docs_packages`, `index_docs` (via `KeyValueStore`)
- Debug: `get_dap_binary`, `dap_request_kind`, `dap_config_to_scenario`, `dap_locator_create_scenario`, `run_dap_locator`

**No method exists for palette commands or task provision.** Free functions in the crate: `current_platform`, `download_file`, `make_file_executable`, `latest_github_release`, `github_release_by_tag_name`, `node_binary_path`, `npm_install_package`, `npm_package_installed_version`, `npm_package_latest_version`, `resolve_tcp_template` (DAP only), `set_language_server_installation_status`; modules `http_client` (`fetch`, `fetch_stream`), `process`, `lsp`, `settings` (`LspSettings::for_worktree`); `KeyValueStore` for persistence; `register_extension!(Type)`.

## 3. Process execution & lifecycle

- API: `zed::process::Command::new(program).arg(...).args(...).env(k,v).envs(...)` then `.output() -> Result<Output{status, stdout, stderr}, String>` (builder over the WIT `process.command` record). Manifest **must** declare `{ kind = "process:exec", command = …, args = […] }` or the host call fails (`capability_granter.grant_exec` → `allow_exec` against manifest; users can further restrict via `granted_extension_capabilities` setting).
- Host impl (`wasm_host/wit/since_v0_8_0.rs`): spawns via `util::command::new_command(...).output().await` — **blocks until the child exits and collects all output**. No streaming, no `spawn` handle, no stdin pipe exposed to WASM.
- Long-lived managed processes exist only through the three Zed-owned spawners: `language_server_command`, `context_server_command`, `get_dap_binary`. Zed (the `project`) starts these when the project needs them and stops them when the project/window closes (*interpretation of Zed's LSP/MCP lifecycle management*).
- Detached-daemon hack (`sh -c 'nohup bin &'`): viable because the returned command exits immediately; Zed does not reaping-kill grandchildren (no kill-on-drop/process-group in the host path) — *researcher inference, not documented*. Risks: un-managed orphans, port collisions, no restart/kill story; you must implement pidfile/health-check yourself via `run-command` + `http_client::fetch`.

## 4. WASM sandbox limits

- Target: `wasm32-wasip2` (component model; older docs/`wasm32-wasi` are outdated).
- FS: WASI ctx preopens **only** `~/Library/Application Support/Zed/extensions/work/<extension-id>/` (macOS) mapped at `"."` and its absolute path, `FsPerms::ReadWrite` (`build_wasi_ctx`). `std::fs` works in that dir. No preopen for `$HOME` or the worktree; worktree access is read-only via `worktree.read_text_file`. Download destinations are canonicalized and must stay inside the work dir (`writeable_path_from_extension`, symlink-safe since 2025 advisories).
- Network: outbound HTTP(S) only via `http_client::fetch`/`fetch_stream` (GET/POST/PUT/…, redirect policies) — **no listen/accept, no raw TCP** in WIT.
- Env: WASM env = `{PWD, RUST_BACKTRACE}` only; use `worktree.shell_env()`.
- Persistence: `KeyValueStore` (insert key/value; used by `index_docs`) and files in the work dir.

## 5. Shipping a helper binary

- `zed::download_file(url, "bin/helper-v1.2.3", DownloadedFileType::Gzip)` — downloads **into the extension work dir** and auto-extracts per file type; then `zed::make_file_executable("bin/helper")`. Path is relative to the work dir; absolute paths are rejected unless inside it.
- **No built-in cache/invalidation**: every call downloads. Standard pattern (see zed-extensions/odin): pick asset name from `zed::current_platform()` (`Os::{Mac,Linux,Windows}` × `Architecture::{Aarch64,X8664,X86}`), call `latest_github_release(repo, GithubReleaseOptions{prelease, require_assets})`, keep the extracted binary plus a version marker file in the work dir, `fs::read_to_string`/`metadata` to skip re-download, honor user override via `LspSettings::for_worktree(...).binary.path` or `worktree.which(...)`.
- Storage: `~/Library/Application Support/Zed/extensions/work/<id>/…` (`paths::extensions_dir() = data_dir()/extensions`; `ExtensionStore` sets `work_dir = extensions_dir/work`).
- Download happens when your trait method runs (e.g. first `language_server_command` call); report progress with `set_language_server_installation_status(Downloading/CheckingForUpdate/Failed)`.

## 6. Dev workflow

1. `rustup target add wasm32-wasip2` (Zed auto-installs it if Rust came from rustup; grammar extensions also need wasi-sdk, auto-downloaded).
2. `Cargo.toml`: `[lib] crate-type = ["cdylib"]`, `zed_extension_api = "0.7.0"`.
3. Command palette → **`zed: install dev extension`** (or Extensions page → "Install Dev Extension") → select the extension folder. **Zed itself runs cargo** and compiles the WASM.
4. Logs: `zed: open log` (`~/Library/Logs/Zed/Zed.log`); extension `println!`/`dbg!` go to Zed's stdout — relaunch with `zed --foreground` to see them.
5. After code changes: re-run **Install Dev Extension** (no auto-rebuild/reload action exists). Installing a dev extension overrides the published one ("Overridden by dev extension").

## 7. Distribution

- PR to **`zed-industries/extensions`**: add your repo as a git submodule at `extensions/<id>` (HTTPS URL, public, non-detached commit), add `[<id>] submodule = "…" version = "…"` to top-level `extensions.toml` (version must match `extension.toml`), run `pnpm sort-extensions`. CI (`.github/workflows/ci.yml`) packages/validates; rules: one extension per PR, ≤3 open PRs, respond within 3 weeks. Merged → published to the registry API consumed by Zed's Extensions page.

## 8. Minimal skeleton (one assistant slash command — the only "command" primitive extensions get)

```toml
# extension.toml
id = "my-cph"
name = "My CP Helper"
version = "0.1.0"
schema_version = 1
authors = ["You <you@example.com>"]
description = "Competitive programming helper"
repository = "https://github.com/you/my-cph"

capabilities = [
  { kind = "process:exec", command = "*", args = ["**"] },
  { kind = "download_file", host = "github.com", path = ["**"] },
]

[slash_commands.fetch-problem]
description = "Fetch a competitive-programming problem"
requires_argument = true

# Optional: register the helper as a context server so Zed spawns/keeps it alive
[context_servers.cph-helper]
```

```rust
// src/lib.rs
use zed_extension_api as zed;

struct MyCph;

impl zed::Extension for MyCph {
    fn new() -> Self { Self }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<zed::SlashCommandOutput, String> {
        match command.name.as_str() {
            "fetch-problem" => {
                let out = zed::process::Command::new("echo")
                    .args(args)
                    .output()
                    .map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                Ok(zed::SlashCommandOutput {
                    text: text.clone(),
                    sections: vec![zed::SlashCommandOutputSection {
                        range: (0..text.len()).into(),
                        label: "problem".into(),
                    }],
                })
            }
            other => Err(format!("unknown command: {other}")),
        }
    }

    fn context_server_command(
        &mut self,
        _id: &zed::ContextServerId,
        _project: &zed::Project,
    ) -> zed::Result<zed::Command> {
        // Zed spawns this as a long-lived managed process (stdio MCP);
        // the native binary itself may open your HTTP listener.
        Ok(zed::Command {
            command: "bin/cph-helper".into(), // downloaded earlier via zed::download_file
            args: vec!["serve".into()],
            env: vec![],
        })
    }
}

zed::register_extension!(MyCph);
```

```toml
# Cargo.toml
[package]
name = "my-cph"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zed_extension_api = "0.7.0"
```

## Contradictions / caveats

- Compat table in `crates/extension_api/README.md` (main) stops at "Zed 0.192.x ↔ API ≤0.6.0" yet crates.io's newest is 0.7.0 and Zed is at 1.21.0 — the table is stale; docs say "use the latest `zed_extension_api` from crates.io", and the host keeps compat shims back to WIT 0.6.0 (`MIN_VERSION 0.6.0` in `since_v0_6_0.rs`; latest host `since_v0_8_0` pins 0.8.0). Practical guidance: build with 0.7.0.
- zed repo's own `extensions/` dir (glsl, html, proto, workflows, test-extension) no longer contains rich binary-download examples; real-world patterns live in the `zed-extensions` org (odin, csharp).
- MCP/context-server extensions are marked for future deprecation in favor of the official MCP registry (zed#59351) — using `[context_servers]` as the daemon-keeper works today but may need migration.

## Missing evidence

- Exact kill semantics of Zed-managed language-server/context-server processes on workspace close vs. app quit (documented lifecycle guarantees not found; behavior inferred from architecture).
- Whether `run-command` children are terminated if the calling WASM invocation is cancelled (no cancellation API exists; likely the call simply awaits exit).
- `KeyValueStore` read/query API surface beyond `insert` (only insertion shown in current WIT resource).

## Sources

Kept:
- Developing Extensions — https://zed.dev/docs/extensions/developing-extensions (dev install, wasm32-wasip2, extension.toml basics)
- Extension Capabilities — https://zed.dev/docs/extensions/capabilities (`process:exec` / `download_file` / `npm:install`, user restriction setting)
- Tasks — https://zed.dev/docs/tasks (task sources incl. "by language extension", tags/runnables)
- MCP Server Extensions — https://zed.dev/docs/extensions/mcp-extensions (manifest + `context_server_command`, deprecation note)
- Publishing Guide — https://zed.dev/docs/extensions/publishing/publishing-guide (submodule PR flow, CI rules)
- `Extension` trait & crate index — https://docs.rs/zed_extension_api/latest/zed_extension_api/trait.Extension.html , …/index.html , …/process/ (19 methods, free fns, `process::Command` builder + blocking `output()`)
- crates.io API for zed_extension_api (0.7.0 = newest)
- Zed source (github.com/zed-industries/zed @main): `crates/extension/src/extension_manifest.rs` (full manifest schema), `crates/extension/src/capabilities.rs` (serde tags), `crates/extension_api/wit/since_v0.6.0/{extension,process,http-client}.wit` + `since_v0.8.0/process.wit` (no listen; run-command semantics), `crates/extension_host/src/wasm_host.rs` (`build_wasi_ctx` preopen, `writeable_path_from_extension`), `crates/extension_host/src/wasm_host/wit/since_v0_8_0.rs` (host `run_command` impl, MIN/MAX 0.8.0), `crates/extension_host/src/extension_host.rs` (`load_plugin_language` + `TaskTemplates::FILE_NAME="tasks.json"`, work dir), `crates/task/src/task_template.rs:144`, `crates/paths/src/paths.rs` (extensions dir)
- zed-extensions/odin (per-OS/arch GitHub-release download pattern, version-cache file), zed-extensions/csharp (`languages/*/tasks.json`, `[language_servers]` + `[grammars]` manifest)
- GitHub Discussion #56280 (dynamic tasks NOT exposed via WIT; static tasks.json is the only extension task path)
- GitHub release API: latest Zed = v1.21.0 (version-scheme sanity check)

Deprioritized: zed.dev/blog/zed-decoded-extensions (older, pre-capabilities architecture), deepwiki mirrors (secondary), security advisories GHSA-59p4/GHSA-v385 (used only to confirm sandbox hardening exists).

## Next steps

1. Prototype the two daemon strategies (context-server-managed vs detached `nohup`) and measure lifecycle behavior on workspace close / Zed quit on macOS.
2. Decide task UX: ship `languages/<competitive-programming-lang>/tasks.json` templates vs. instructing users to add `.zed/tasks.json`.
3. Track zed#59351 (MCP extension deprecation) and any future `provide-tasks`/command-palette WIT exports before committing the architecture.
