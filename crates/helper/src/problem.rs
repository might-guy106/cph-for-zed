use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Competitive Companion problem payload (subset we use; extra fields ignored).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Problem {
    pub name: String,
    pub group: String,
    pub url: String,
    pub interactive: bool,
    pub memory_limit: u32,
    pub time_limit: u32,
    pub tests: Vec<Test>,
    pub test_type: String,
    pub batch: Batch,
    pub languages: Languages,
}

impl Default for Problem {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::new(),
            url: String::new(),
            interactive: false,
            memory_limit: 0,
            time_limit: 0,
            tests: Vec::new(),
            test_type: String::new(),
            batch: Batch::default(),
            languages: Languages::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Test {
    pub input: String,
    pub output: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct Batch {
    #[allow(dead_code)]
    pub id: String,
    pub size: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct Languages {
    pub java: Option<Java>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Java {
    pub task_class: String,
}

/// Handle one incoming problem: create the folder, write sol.cpp from the
/// template (never overwriting existing code) and refresh the sample tests.
/// Returns the created directory.
pub fn receive(root: &Path, body: &[u8]) -> Result<PathBuf, String> {
    let problem: Problem =
        serde_json::from_slice(body).map_err(|e| format!("invalid problem json: {e}"))?;
    if problem.name.is_empty() {
        return Err("problem has no name".to_string());
    }

    let dir = target_dir(root, &problem);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;

    let sol = dir.join("sol.cpp");
    if !sol.exists() {
        let template = if problem.test_type == "multiNumber" {
            include_str!("../../../templates/multi.cpp")
        } else {
            include_str!("../../../templates/single.cpp")
        };
        fs::write(&sol, template).map_err(|e| format!("write {}: {e}", sol.display()))?;
        open_in_zed(&sol);
    }

    write_tests(&dir, &problem.tests)?;

    if !problem.url.is_empty() {
        let _ = fs::write(dir.join("problem.url"), format!("{}\n", problem.url));
    }

    let relative = dir.strip_prefix(root).unwrap_or(&dir);
    eprintln!(
        "[cph] + {} ({} tests{})",
        relative.display(),
        problem.tests.len(),
        if problem.interactive {
            ", interactive"
        } else {
            ""
        }
    );
    Ok(dir)
}

fn target_dir(root: &Path, problem: &Problem) -> PathBuf {
    let kind = if problem.batch.size > 1 {
        "contests"
    } else {
        "practice"
    };
    let problem_slug = problem_slug(problem);
    let group_slug = slugify(&problem.group, 80);
    if group_slug.is_empty() {
        root.join(kind).join(problem_slug)
    } else {
        root.join(kind).join(group_slug).join(problem_slug)
    }
}

/// CC ships a filename-safe problem name for Java tooling; use it when sane.
fn problem_slug(problem: &Problem) -> String {
    let task_class = problem
        .languages
        .java
        .as_ref()
        .map(|j| j.task_class.as_str())
        .unwrap_or("");
    if !task_class.is_empty()
        && task_class
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        task_class.to_string()
    } else {
        slugify(&problem.name, 60)
    }
}

fn slugify(text: &str, max_len: usize) -> String {
    let mut slug = String::with_capacity(text.len().min(max_len));
    let mut last_dash = true; // also trims leading dashes
    for c in text.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            slug.push(c);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_end_matches('-')
        .chars()
        .take(max_len)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// Write NN.in / NN.out pairs (01-based, zero-padded) and remove stale
/// numbered pairs left over from a previous parse with more tests.
fn write_tests(dir: &Path, tests: &[Test]) -> Result<(), String> {
    for (i, test) in tests.iter().enumerate() {
        let n = i + 1;
        fs::write(dir.join(format!("{n:02}.in")), &test.input)
            .map_err(|e| format!("write test {n}: {e}"))?;
        fs::write(dir.join(format!("{n:02}.out")), &test.output)
            .map_err(|e| format!("write test {n}: {e}"))?;
    }
    for n in tests.len() + 1..100 {
        let input = dir.join(format!("{n:02}.in"));
        let output = dir.join(format!("{n:02}.out"));
        if !input.exists() && !output.exists() {
            break;
        }
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }
    Ok(())
}

/// Ask a running Zed to open the new solution file (best effort).
fn open_in_zed(path: &Path) {
    for zed in ["/usr/local/bin/zed", "/opt/homebrew/bin/zed"] {
        if Path::new(zed).exists() {
            let _ = std::process::Command::new(zed)
                .arg(path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            return;
        }
    }
}
