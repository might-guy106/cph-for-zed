# CPH (Competitive Programming Helper) — Feature Inventory for a Zed Port

Source of truth: local clone of `https://github.com/agrawal-d/cph` @ commit `187590e` (2026‑09‑16, latest `main`).
Companion/companion‑submit facts verified against clones of `jmerle/competitive-companion` and `agrawal-d/cph-submit`.
Every claim below cites a repo file path. Items marked **[verified]** were read directly from source; **[inference]** is my interpretation; **[absent]** means I searched and could not find the feature.

Classification legend: **[UI-dependent]** needs a VS Code webview/panel (Zed has no custom-UI API) · **[logic]** pure process/file logic, portable · **[editor-integrated]** commands/keybindings/tasks, portable in adapted form.

---

## 1. Competitive Companion integration — [logic] (listener) + [editor-integrated] (file open)

**Port & protocol [verified].** CPH starts an HTTP server on **port 27121** (`src/config.ts` → `port: 27121`). The server is created in `src/companion.ts` → `setupCompanionServer()` and listens in `activate()` (`src/extension.ts`).
- Competitive Companion POSTs the problem JSON to `http://localhost:27121/` with header `Content-Type: application/json`. In Companion's `src/hosts/hosts.ts`, `27121` is the hard-coded "Competitive Programming Helper" default port; `src/hosts/CustomHost.ts` sends `fetch('http://localhost:<port>/', { body: data, headers:{'Content-Type':'application/json'} })`.
- CPH's server reads the raw request body, `JSON.parse`s it into a `Problem`, and calls `handleNewProblem(problem)` (`src/companion.ts`). It does **not** route on URL path — any POST body is treated as a problem.

**On-disk metadata (`.cph/.<name>.json_<md5>.prob`) [verified].** The exact filename is **not** `.json_<hash>`; it is:
```
.<srcFileName>_<md5(srcPath)>.prob
```
built in `src/parser.ts` → `getProbSaveLocation()`:
- `hash = crypto.createHash('md5').update(srcPath).digest('hex')` (full 32-char hex; `.substr(0)` is a no-op).
- `baseProbName = '.' + srcFileName + '_' + hash + '.prob'` → e.g. `.A_Two_Sum.cpp_9f2b….prob` (note the leading dot).
- Directory: if setting `cph.general.saveLocation` is a non-empty valid path, the `.prob` goes there directly; otherwise it goes in a `.cph/` subfolder next to the source file (`path.join(srcFolder, '.cph')`). `saveProblem()` creates `.cph/` if missing.
- **Contents** = `JSON.stringify(problem)` where `problem` is the `Problem` type (`src/types.ts`):
  ```json
  {
    "name": "A. Two Sum",
    "url": "https://codeforces.com/.../problem/0/A",
    "interactive": false,
    "memoryLimit": 256,
    "timeLimit": 1000,
    "group": "Codeforces - Codeforces Round #...",
    "tests": [ { "input": "...", "output": "...", "id": 1695999999999 } ],
    "srcPath": "/abs/path/A_Two_Sum.cpp",
    "local": false,                  // optional; true only for locally-created problems
    "customCheckerPath": "/abs/checker.py"  // optional
  }
  ```
  `tests[].id` is added by CPH (Companion does not send it): `randomId(index) = Math.floor(Date.now() + index)` (`src/companion.ts`, `src/utils.ts`). `srcPath` is set to the newly created source file. The schema exactly matches Companion's `Task` model (`competitive-companion/src/models/Task.ts`: `name, group, url, interactive, memoryLimit, timeLimit, tests[], testType, input, output, languages, batch`), minus Companion-only fields CPH ignores.

**Contests vs single problems [verified].** CPH has **no contest concept** — each problem is an independent POST. In Companion, a contest parse produces a `Contest` whose `send()` loops `for (const task of this.tasks) await task.send()` (`competitive-companion/src/models/Contest.ts`) → **one sequential HTTP POST per problem**. CPH handles each POST independently, creating one source file + one `.prob` per problem. (Companion tags tasks with a shared `batch.id`/`batch.size`, but CPH ignores them.)

**Folder / file naming rules [verified].** All problems are created in the **first workspace folder** (`vscode.workspace.workspaceFolders[0]`); no per-contest subfolders. Filename = `getProblemFileName(problem, ext)` (`src/companion.ts`):
- Default: split `problem.name` into "words" via setting `cph.general.wordRegex` (default `[\\p{L}]+|[0-9]+`), join with `_` → `Two_Sum.cpp`. If regex yields null, fallback `name.replace(/\W+/g,'_')`.
- If `cph.general.includeProblemIndex` = false (default), strip the leading `X - ` section (`name.split(' - ')`, drop first).
- Java/Kotlin: the base name is PascalCased (`toPascalCase`, `src/utilsPure.ts`) and if it starts with a digit, prefixed with `Problem`.
- Short-name overrides (settings `useShortCodeForcesName` / `useShortLuoguName` / `useShortAtCoderName`, all default false): Codeforces → `<contestId><index>` (e.g. `1440A.cpp`) via `getProblemName()` regexes in `src/submit.ts`; Luogu → `P1000.cpp`; AtCoder → `abc311_a.cpp`.
- Extension `ext` comes from `config.extensions` map (`c→c, cpp/cc/cxx→cpp, python→py, rust→rs, java→java, kotlin→kt, js→js, go→go, hs→hs, csharp→cs, cangjie→cj`).
- Non-ASCII in the **binary** name is escaped via `toAsciiFilename` (`_u<hex>`), but the source filename keeps the word-join.

---

## 2. Template mechanism — [logic]

**Setting key [verified]:** `cph.general.defaultLanguageTemplateFileLocation` (string, default `""` = no template). Holds an **absolute path to a user-supplied template file**; there is **no built-in default template content** (CPH writes an empty file then, if configured, overwrites with the template).
- Applied only when (a) the file is newly created by a Companion import, and (b) `cph.general.defaultLanguage` is set (not `none`). Logic in `src/companion.ts` → `handleNewProblem()`: `writeFileSync(srcPath,'')`, then if `getDefaultLanguageTemplateFileLocation()` returns a path that exists, read it and `writeFileSync(srcPath, templateContents)`.
- **Java special-case:** the literal string `CLASS_NAME` in the template is replaced with the file's basename (`src/companion.ts`).
- **Variable replacement** (setting `cph.general.doTemplateFileVariableReplacement`, default false): replaces `$var$` tokens. Available vars = every key of the `problem` object plus `date` (`YYYY-MM-DD`) and `time` (`HH:MM:SS`). Each value is `JSON.stringify`ed then stripped of surrounding quotes (`src/companion.ts`). Documented examples: `$name$`, `$url$`, `$date$`, `$time$` (`docs/user-guide.md`).
- **Cursor placeholder:** the literal `$CURSOR_PLACEHOLDER` in the new file is deleted and the cursor moved there (`src/companion.ts`). [editor-integrated]
- Template file missing → error toast "Template file does not exist" (`src/companion.ts`).

---

## 3. Compile & run pipeline — [logic] (compile/run/judge) + [UI-dependent] (status surface)

### Per-language compile configuration [verified]
For each of 13 languages there are 3–4 settings (shown for C++; identical shape for `c, csharp, python, ruby, rust, go, haskell, java, kotlin, js, cangjie`):
- `cph.language.cpp.Command` — compiler executable, default `g++`.
- `cph.language.cpp.Args` — extra flags as a **space-separated string** (split on `' '`), default `""`.
- `cph.language.cpp.OutputArg` — output flag, default `-o` (enum `-o` or `/Fe:` for MSVC). Only C and C++ have `OutputArg`.
- `cph.language.cpp.SubmissionCompiler` — Codeforces compiler label for submit, default `GNU G++23 14.2 (64 bit, msys2)` (enum of 9 CF compiler names). Mapped to CF's numeric language id via `config.compilerToId` (`src/config.ts`).

**Exact C++ compile command [verified]** (`src/compiler.ts` → `getFlags()`):
```
g++ <srcPath> -o <binPath> <...userArgs> -D DEBUG -D CPH [-D ONLINE_JUDGE]
```
- `-D DEBUG` and `-D CPH` are **always** appended; `-D ONLINE_JUDGE` appended only when the "online judge env" toggle is on (setting `cph.general.defaultOnlineJudge`, default false; togglable per-session in the UI checkbox).
- Binary path `getBinSaveLocation()`: `<srcDir>/<asciiSrcName>.bin` (`.class`→dir for Java, `.jar` Kotlin, `_bin`/`.bin` C#, else `.bin`); or `cph.general.saveLocation` if set.
- Other languages build analogous arg lists (Rust `rustc src -o bin`; Go `go build -o bin src`; Java `javac src -d dir`; Kotlin `kotlinc src -include-runtime -d jar`; Haskell `ghc src -o bin -no-keep-hi-files -no-keep-o-files`; C# `dotnet build`/`mcs`; Cangjie `cjc src -o bin`).
- **skipCompile** (interpreted) = `py, js, rb` (`src/config.ts`); their "binary" is just the source path.

### Execution & verdicts [verified]
`src/executions.ts` → `runTestCase(language, binPath, input)` spawns the binary/interpreter, writes `input` to stdin, captures stdout/stderr/exit code/signal, and times it.
- **Timeout / TLE:** a manual `setTimeout(() => { result.timeOut = true; process.kill(); }, getTimeOutPref())`. `getTimeOutPref()` = setting **`cph.general.timeOut`** (default **3000** ms; fallback `|| 3000`). There is also a Node spawn backstop `timeout: config.timeout` = **10000** ms (`src/config.ts`) that SIGTERMs the child; the 3000 ms user timer normally fires first. The problem's own `timeLimit` field is **not** used for the kill — the user setting governs all cases.
- **Time measurement:** wall-clock `Date.now()` delta around process start→exit (`const begin = Date.now()` … `result.time = end - begin`). Displayed as `<n>ms`, or "Timed out" if `timeOut`.
- **Memory limit:** stored in `.prob` but **never enforced** — no `ulimit`/`rlimit`/`maxBuffer` anywhere **[verified by grep]**. → **no MLE verdict**.
- **Interactive problems:** `interactive` flag stored but **not specially handled** (no special runner) **[verified]**.

**Verdict surface in UI [verified]** (`src/webview/processRunSingle.ts`, `src/judge.ts`, `src/webview/frontend/CaseView.tsx`). CPH does **not** print AC/WA/CE/RE/TLE acronyms; it shows:
- **Passed** (green) — `result.pass === true`.
- **Failed** (red) — `result.pass === false`. Sub-causes surfaced inline:
  - **WA-type:** wrong answer → `isResultCorrect()` false; an inline token diff is shown (see §4).
  - **RE-type:** non-zero exit code **or** any signal **or** (unless `cph.general.ignoreSTDERROR`) non-empty stderr → `didError` ⇒ fail; the **signal name** (e.g. `SIGSEGV`, `SIGABRT`) replaces the output text, and stderr is shown in a "Standard Error" box.
  - **TLE-type:** killed by timer → `timeOut` ⇒ shows **"Timed out"**; `SIGTERM`/`SIGKILL` also force `pass=false`.
- **CE (compile error):** `compileFile()` returns false → the run aborts **before** any testcase runs; the compiler stderr is written to the **VS Code Output channel named "cph"** (`ocWrite`/`ocShow`, `src/utils.ts`) and the webview gets `compiling-stop` + `not-running`. There is **no per-testcase CE verdict** — the whole run just doesn't happen. `cph.general.hideStderrorWhenCompiledOK` (default true) suppresses warnings on success.
- **Correctness rule** (`src/judge.ts` → `isResultCorrect`): normalize CRLF→LF, trim, split lines; **line counts must match** and each line compared **after trimming that line's trailing/leading whitespace** (`expectedLines[i].trim() !== resultLines[i].trim()`). So trailing whitespace per line is ignored, but an extra/missing line fails. **No floating-point tolerance** (that's what Custom Checker is for).

### Run orchestration [verified]
`src/webview/processRunAll.ts`: compile once → for each test send `running` → `runSingleAndSave()` sequentially → `deleteBinary()` at the end. Run-all reuses one compiled binary; run-single (`processRunSingle.ts`) compiles, runs, then deletes the binary unless `skipCompile`. After all tests the binary is deleted (`src/executions.ts` → `deleteBinary`, `rm`/`del`).

---

## 4. Test-case UI — [UI-dependent] (this is the part that cannot port to Zed as-is)

CPH's entire results UI is a **React webview** registered as a sidebar view `cph.judgeView` inside activity-bar container `cph-judge-view-container` (`package.json` `contributes.views`; provider `src/webview/JudgeView.ts`; frontend `src/webview/frontend/*.tsx`). It communicates via `postMessage` (`WebviewToVSEvent` / `VSToWebViewMessage`, `src/types.ts`).

What the panel shows (`src/webview/frontend/App.tsx` + `CaseView.tsx`) **[verified]**:
- **Header:** problem name (link to URL), live pass counter `X / Y passed` with color (green=all pass, red=0 pass). Spinning loader while compiling.
- **Per test case (`CaseView`):** collapsible card `TC n` with state class `passed`/`failed`/`running`.
  - Title row: chevron minimize/expand, "Running…/Checking…" while active, **Passed/Failed** badge, and exec time (`<n>ms` or "Timed out").
  - **Input** textarea (editable) + copy button.
  - **Expected output** textarea (editable) + copy (hidden when a custom checker is set).
  - **Received output** read-only textarea + copy + **"Set"** button (copies received → expected). Signal name shown here if the process was signaled.
  - **Output Difference** block (see diff below) when failed and not error/checker.
  - **Standard Error** read-only box when stderr non-empty.
  - **Checker Log** expandable (exit code, stdout/stderr, invocation, duration) when a custom checker ran.
  - Per-case buttons: **Run again** (single), **Stop**, **Delete**.
  - Auto-minimize on pass; auto-expand on fail. Stdout truncated to 100 000 chars (`[Truncated]`).
- **Bottom toolbar:** **New Testcase**, **Submit** (Codeforces/Kattis/CSES — only when `problem.url` host matches), **Custom Checker** toggle + path input, footer links (Donate, Feedback, **Import** testcases, Cat companion 🐱, Bugs, About), an **"Online Judge"** checkbox (defines `ONLINE_JUDGE`), a remote broadcast message, and optional live user count.
- **Main actions:** big green **Run All** split-button (chevron reveals a context menu with **Compile Without Running**), red **Delete** (deletes the `.prob` file), yellow **Settings** (opens VS Code settings filtered to the extension).
- **Add / edit / delete:** add = "New Testcase" appends a blank case; edit = type in input/expected textareas (auto-saved to `.prob` after a 500 ms debounce via the `save` message); delete = per-case trash or global Delete for the whole `.prob`. **Import** (`ImportCases.tsx`) pastes a batch of input/output pairs.
- **Run all vs single:** "Run All" (`run-all-and-save`) vs per-case "Run again" (`run-single-and-save`).

**Diff presentation [verified]** (`src/utils/diffOutput.ts` + `DiffView` in `CaseView.tsx`): a **token-level LCS diff** (not a line diff for display). Tokens = words/whitespace/newlines; rendered inline as chips:
- `match` → plain text; `extra` (in received, not expected) → green/inserted background; `missing` (in expected, not received) → red/removed background + strikethrough; `\n` → line break.
- A separate line-level pass computes `DiffLine[]` (`match`/`changed`/`missing`/`extra`) for a summary string ("N lines differ."), but the UI renders the token chips. Hidden by default if `cph.general.hideOutputDifference` (default **true**) — user toggles and it's persisted (`set-hide-output-diff`).

---

## 5. Extra features

### Codeforces / CSES / AlgoZenith submission — [editor-integrated] (command) + external browser add-on
**There is no companion binary; `cph-submit` is a browser extension** (Firefox & Chrome) **[verified]** (`cph-submit/readme.md`: "Browser add-on that enables direct submission on Codeforces, CSES and Maang.in").
- Install from the Firefox/Chrome store; keep a browser window open. No settings in CPH itself other than the per-language `SubmissionCompiler`.
- **Flow [verified]:** CPH command `cph.submitToCodeForces` (`src/submit.ts`) or the webview Submit button → `storeSubmitProblem(problem)` (`src/companion.ts`) reads the source file, resolves the Codeforces language id (`getLanguageId` → `config.compilerToId`), and stashes `savedResponse = { empty:false, url, problemName, sourceCode, languageId }` in memory. The webview shows "waiting for submit".
- The **cph-submit browser add-on polls** `http://localhost:27121/getSubmit` with header **`cph-submit: true`** (`cph-submit/src/config.ts`, `offscreen.ts`, `backgroundScript.ts`). CPH's server responds to **any** request with `JSON.stringify(savedResponse)`; when the request has the `cph-submit: true` header it also clears `savedResponse` and posts `submit-finished` to the webview (`src/companion.ts`).
- The add-on then opens/autofills the Codeforces (or CSES / AlgoZenith/"Maang.in") submit page via injected content scripts (`cph-submit/src/injectedScript.ts`, `csesInjectedScript.ts`, `algoZenithInjectedScript.ts`, `handleSubmit.ts`).
- `problemName` for CF is extracted from the URL by `getProblemName()` regexes (`/contest/<id>/problem/<idx>`, `/gym/…`, `/problemset/problem/…`, etc.) → e.g. `1440C` (`src/submit.ts`).
- **CSES** submit: webview `submitCSES` → same `storeSubmitProblem`; the add-on's CSES script handles it. Only shown when host ends with `cses.fi` (`App.tsx`).

### Kattis submission — [editor-integrated] + external Python cli
- Command `cph.submitToKattis` (`src/submit.ts`, `src/companion.ts` → `submitKattisProblem`). Requires the official **Kattis `submit.py` + `.kattisrc`** placed in `~/.kattis/` (validated with `existsSync`). CPH runs `spawn('python', ['~/.kattis/submit.py', '-f', srcPath])` and writes `Y\n` to its stdin to confirm opening the submissions page (`src/companion.ts`). Only for problems whose URL host is `open.kattis.com`. **[logic]** (spawn) but driven from webview/command.

### Custom Checker (Special Judge) — [logic]
- Per-problem field `customCheckerPath` (stored in `.prob`). Set via the webview "Custom Checker" button → absolute path to a **Python** script.
- Execution **[verified]** (`src/utils/customChecker.ts`): writes the testcase input and the program's stdout to temp files `cph-input-<rand>.txt` / `cph-output-<rand>.txt` in the OS temp dir, runs `python <checkerPath> <inputFile> <outputFile>`, judges purely by **exit code** (`0` = pass). stdout/stderr/exit code/time captured and shown in "Checker Log". Temp files deleted after. Only used when the run itself didn't error; bypasses `isResultCorrect` and hides the expected-output field.
- Documented in `docs/user-guide.md` "Custom Checker (Special Judge)".

### Problem navigation between tests — [absent]
There is **no "next/previous problem" or "next failing test" navigation** **[verified by reading App.tsx/CaseView.tsx]**. The closest behaviors:
- Switching the active editor loads that file's `.prob` into the panel (`src/webview/editorChange.ts` → `editorChanged`, fires on `onDidChangeActiveTextEditor`).
- Passed cases auto-minimize so the view scrolls to failures. No keyboard nav between cases.

### Keyboard shortcuts — [editor-integrated] [verified] (`package.json` `contributes.keybindings`)
| Binding | Command | Action |
|---|---|---|
| `Ctrl+Alt+B` | `cph.runTestCases` | Compile & run all testcases for the active file (creates a local problem if none). |
| `Ctrl+Alt+D` | `cph.judgeView.focus` | Focus/open the judge webview panel. |
| `Ctrl+Alt+S` | `cph.submitToCodeForces` | Submit active problem to Codeforces. |
- Extra commands (no default keybinding): `cph.compileWithoutRunning`, `cph.submitToKattis`, and the legacy alias `extension.runCodeforcesTestcases` (all registered in `src/extension.ts`).
- A **status-bar button** "Run Testcases" (`$(run-all)`) triggers `cph.runTestCases` (`src/extension.ts`).

### Stress testing / brute-force comparison / test generator — [absent]
**CPH has no stress-testing, no random test generator, and no brute-force/slow-solution comparison mode.** Verified by grep for `stress|brute|generator|random test` across `src/`, `docs/`, `README.md` — no matches. (This is a notable gap vs. alternatives like Competitive Companion's sibling tools or cpeditor; a Zed port could add it as a differentiator.)

### AI features — [absent]
**None.** No LLM/Copilot/OpenAI/Gemini integration anywhere (grep found only `jest.clearAllMocks`). The README and `package.json` list no AI capability.

### Misc [verified]
- **Telemetry / live user count:** `src/telmetry.ts` + `@vscode/extension-telemetry` (key empty by default). Optional "live user count" pings `cph.general.remoteServerAddress` (default `http://20.244.105.138:4546`) every 30 s, only when `cph.general.showLiveUserCount` (default false). **Not needed for a port.**
- **Remote broadcast message:** on activate, fetches a notice from a raw GitHub URL and shows it in the panel (`src/extension.ts` → `downloadRemoteMessage`, `config.remoteMessageUrl`).
- **Localization:** en, zh-CN, zh-TW, ko, ja (`package.nls*.json`, `src/i18n.ts`).
- **Cat companion 🐱:** an easter-egg animated cat (`CatCompanion.tsx`, `meow.mp3`) reacting to pass rate.
- **Online-judge env toggle:** defines `ONLINE_JUDGE` macro/`DEBUG`/`CPH` env at compile/run time (`src/compiler.ts`, `src/executions.ts`).

---

## 6. Feature-by-feature portability for a Zed extension

| Feature | Classification | Portable to Zed? |
|---|---|---|
| Companion HTTP listener on :27121 | [logic] | ✅ Yes — plain HTTP server, editor-agnostic. |
| `.cph/.<name>_<md5>.prob` storage & schema | [logic] | ✅ Yes — pure file I/O. |
| Filename generation (wordRegex, short names, PascalCase) | [logic] | ✅ Yes. |
| Template file + `$var$` / `CLASS_NAME` / `$CURSOR_PLACEHOLDER` | [logic] / [editor-integrated] | ✅ Mostly — cursor placement needs Zed's equivalent of "open file at position". |
| Compile pipeline & flags | [logic] | ✅ Yes — `child_process`/equivalent spawn. |
| Run + judge + TLE timeout + token diff | [logic] | ✅ Yes — fully portable (incl. `isResultCorrect`, `diffOutput`). |
| Custom checker (Python, exit-code) | [logic] | ✅ Yes. |
| Kattis submit (`~/.kattis/submit.py`) | [logic] / [editor-integrated] | ✅ Yes — spawn python. |
| Codeforces/CSES submit stash + `:27121/getSubmit` handshake | [logic] | ✅ Yes — the cph-submit add-on talks to port 27121 regardless of editor. |
| Commands & keybindings (run all, submit, compile-only) | [editor-integrated] | ⚠️ Adapt — Zed has commands/keybindings/tasks; map `Ctrl+Alt+B` etc. to Zed actions/tasks. |
| Status-bar "Run Testcases" button | [editor-integrated] | ⚠️ Adapt — Zed status bar API is limited; may need a command instead. |
| **Results panel (test list, diff view, add/edit/delete, run-all/single buttons)** | **[UI-dependent]** | ❌ **Blocked** — Zed has no webview/custom-panel API. Must re-surface as: terminal/task output, a markdown/buffer report, diagnostics, or an external TUI/web app. |
| Settings UI (`cph.*` keys) | [editor-integrated] | ⚠️ Adapt — move to Zed `settings.json` keys. |
| Live user count / remote message / telemetry / cat | [UI-dependent]/[logic] | ❌ Skip — non-essential. |

---

## Contradictions
- **`config.timeout` vs `cph.general.timeOut`:** `src/config.ts` sets `timeout: 10000` while the user setting `cph.general.timeOut` defaults to 3000. Both are used in `executions.ts` (Node spawn backstop vs. the manual kill timer). The **effective TLE is the user setting (3000 ms)**; the 10 000 ms is a rarely-hit backstop. Recorded so the port doesn't accidentally use the wrong constant.
- **Metadata filename:** the task brief guessed `.cph/.<name>.json_<hash>.prob`. The real pattern is `.cph/.<srcFileName>_<md5(srcPath)>.prob` (no `.json` segment) **[verified `src/parser.ts`]**.

## Missing evidence / residual uncertainty
- The full `WebviewToVSEvent` message list was read from `src/types.ts`, but the exact visual layout/CSS is from reading TSX, not from running the extension; pixel-level replication was not attempted (irrelevant for Zed anyway).
- cph-submit's CF form-fill selectors were not audited line-by-line (not needed to replicate the CPH side, which is just the `:27121/getSubmit` response).
- Whether Zed's extension API (Wasm) can bind a TCP listener on 27121 is an **open platform question** — the CPH side is verified, but Zed's capability must be confirmed separately (may require an external helper process).

## Sources (kept)
- `agrawal-d/cph` @ 187590e — `src/companion.ts` (listener, naming, template, submit handshake), `src/config.ts` (port 27121, timeouts, compilerToId), `src/parser.ts` (.prob path/schema), `src/types.ts` (Problem/Run/verdict/message types), `src/compiler.ts` (flags), `src/executions.ts` (run/timeout/kill), `src/judge.ts` (correctness rule), `src/webview/processRunSingle.ts` + `processRunAll.ts` (verdict assignment), `src/webview/JudgeView.ts` (webview plumbing), `src/webview/frontend/App.tsx` + `CaseView.tsx` (UI/diff), `src/utils/diffOutput.ts` (LCS token diff), `src/utils/customChecker.ts` (special judge), `src/submit.ts` (CF/Kattis submit), `src/extension.ts` (commands, status bar), `package.json` (commands, keybindings, all `cph.*` settings + defaults), `docs/user-guide.md` (templates, custom checker, Kattis), `docs/dev-guide.md` (language onboarding).
- `jmerle/competitive-companion` — `src/hosts/hosts.ts` + `CustomHost.ts` (port 27121 POST), `src/models/Task.ts` (wire schema), `src/models/Contest.ts` (one POST per problem).
- `agrawal-d/cph-submit` — `readme.md` (browser add-on, CF/CSES/AlgoZenith), `src/config.ts` + `offscreen.ts` + `backgroundScript.ts` (polls `http://localhost:27121/getSubmit` with `cph-submit: true`).

## Next steps (most useful follow-ups)
1. Confirm whether Zed extensions can open a TCP server (for Companion on 27121) or whether an external helper binary is required.
2. Decide the Zed UX substitute for the webview results panel (task output vs. scratch buffer vs. external app) — this is the single biggest design gap.
3. Prototype the `:27121/getSubmit` handshake early; it must remain byte-compatible so the existing cph-submit browser add-on keeps working.
