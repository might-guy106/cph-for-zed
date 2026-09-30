use std::fs;
use std::path::Path;

/// Make sure the worktree has .zed/tasks.json with the cph tasks so they
/// show up in `task: spawn`. Creates the file if missing; if a user file
/// exists without our entries, prints a ready-to-paste snippet instead of
/// touching it.
pub fn ensure(root: &Path) {
    let zed_dir = root.join(".zed");
    let tasks_path = zed_dir.join("tasks.json");
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "cph-helper".to_string());

    if tasks_path.exists() {
        let contents = fs::read_to_string(&tasks_path).unwrap_or_default();
        if !contents.contains("cph: test") {
            eprintln!(
                "[cph] .zed/tasks.json exists but has no cph tasks; add these entries:\n{}",
                tasks_json(&exe)
            );
        }
        return;
    }

    if fs::create_dir_all(&zed_dir).is_err() {
        return;
    }
    let _ = fs::write(&tasks_path, tasks_json(&exe));
}

fn tasks_json(exe: &str) -> String {
    let exe = exe.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"[
  {{
    "label": "cph: test",
    "command": "{exe}",
    "args": ["judge", "$ZED_FILE"],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "always",
    "hide": "never",
    "save": "current"
  }},
  {{
    "label": "cph: compile",
    "command": "{exe}",
    "args": ["judge", "--compile-only", "$ZED_FILE"],
    "use_new_terminal": false,
    "allow_concurrent_runs": true,
    "reveal": "always",
    "hide": "on_success",
    "save": "current"
  }},
  {{
    "label": "cph: serve",
    "command": "{exe}",
    "args": ["serve"],
    "cwd": "$ZED_WORKTREE_ROOT",
    "use_new_terminal": true,
    "reveal": "always",
    "hide": "never"
  }}
]
"#
    )
}
