//! End-to-end judge tests: run the real cph-helper binary against
//! temporary problem folders. Requires g++-16 on PATH.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn problem_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cph-judge-test-{}-{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn judge(dir: &Path, file: &str) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cph-helper"))
        .args(["judge", &dir.join(file).display().to_string()])
        .output()
        .unwrap();
    let code = out.status.code().unwrap_or(-1);
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (code, log)
}

fn write(dir: &Path, file: &str, contents: &str) {
    fs::write(dir.join(file), contents).unwrap();
}

const SUM: &str = r#"#include <bits/stdc++.h>
using namespace std;
int main(){ long long a,b; cin>>a>>b; cout<<a+b<<"\n"; }
"#;

#[test]
fn all_pass_exit_0() {
    let dir = problem_dir("pass");
    write(&dir, "sol.cpp", SUM);
    write(&dir, "01.in", "1 2\n");
    write(&dir, "01.out", "3\n");
    write(&dir, "02.in", "10 20\n");
    write(&dir, "02.out", "30\n");
    let (code, log) = judge(&dir, "sol.cpp");
    assert_eq!(code, 0, "{log}");
    assert!(log.contains("2/2 passed"), "{log}");
}

#[test]
fn wrong_answer_exit_1_with_diff() {
    let dir = problem_dir("wa");
    write(&dir, "sol.cpp", "int main(){ return 0; }\n");
    write(&dir, "01.in", "1 2\n");
    write(&dir, "01.out", "3\n");
    let (code, log) = judge(&dir, "sol.cpp");
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("WA"), "{log}");
    assert!(log.contains("expected \"3\""), "{log}");
}

#[test]
fn tle_killed_and_reported() {
    let dir = problem_dir("tle");
    // short timeout via cph.toml (also exercises config discovery)
    write(&dir, "cph.toml", "timeout_ms = 500\n");
    write(&dir, "sol.cpp", "int main(){ while(1); }\n");
    write(&dir, "01.in", "x\n");
    write(&dir, "01.out", "y\n");
    let started = std::time::Instant::now();
    let (code, log) = judge(&dir, "sol.cpp");
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("TLE"), "{log}");
    assert!(started.elapsed().as_secs() < 10, "took too long: {log}");
}

#[test]
fn compile_error_exit_1() {
    let dir = problem_dir("ce");
    write(&dir, "sol.cpp", "int main(){ syntax error }\n");
    write(&dir, "01.in", "x\n");
    write(&dir, "01.out", "y\n");
    let (code, log) = judge(&dir, "sol.cpp");
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("compile error"), "{log}");
}

#[test]
fn nonzero_exit_is_re() {
    let dir = problem_dir("re");
    write(&dir, "sol.cpp", "int main(){ return 42; }\n");
    write(&dir, "01.in", "x\n");
    write(&dir, "01.out", "y\n");
    let (code, log) = judge(&dir, "sol.cpp");
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("RE (exit 42)"), "{log}");
}
