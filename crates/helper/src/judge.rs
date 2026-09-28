//! Compile a solution and run it against NN.in / NN.out samples,
//! printing verdicts to the terminal. Exit code 0 iff every test passes.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub struct Options {
    pub compile_only: bool,
}

pub fn run(src: &Path, opts: &Options) -> i32 {
    if !src.is_file() {
        eprintln!("[cph] no such file: {}", src.display());
        return 2;
    }
    let cfg = config::load(src);
    let bin = src.with_file_name(src.file_stem().unwrap());

    let started = Instant::now();
    let compiled = Command::new(cfg.cxx())
        .args(cfg.cxxflags())
        .arg(src)
        .arg("-o")
        .arg(&bin)
        .output();
    let compile_out = match compiled {
        Ok(out) => out,
        Err(e) => {
            eprintln!("{RED}✗ failed to run compiler '{}': {e}{RESET}", cfg.cxx());
            return 1;
        }
    };
    if !compile_out.status.success() {
        eprintln!("{RED}✗ compile error{RESET}");
        eprint!("{}", String::from_utf8_lossy(&compile_out.stderr));
        return 1;
    }
    eprintln!(
        "{DIM}» compiled {} in {}ms{RESET}",
        src.file_name().unwrap().to_string_lossy(),
        started.elapsed().as_millis()
    );

    if opts.compile_only {
        return 0;
    }

    let tests = find_tests(src);
    if tests.is_empty() {
        eprintln!("[cph] no NN.in samples next to {}", src.display());
        return 0;
    }

    let mut passed = 0;
    for input in &tests {
        let n = input.file_stem().unwrap().to_string_lossy().to_string();
        let expected = input.with_extension("out");
        let outcome = run_one(&bin, input, Duration::from_millis(cfg.timeout_ms()));
        let ok = report(&n, &expected, &outcome);
        if ok {
            passed += 1;
        }
    }

    let total = tests.len();
    let (color, mark) = if passed == total {
        (GREEN, "✓")
    } else {
        (RED, "✗")
    };
    eprintln!("{color}{mark} {passed}/{total} passed{RESET}");
    if passed == total {
        0
    } else {
        1
    }
}

/// `NN.in` files (two digits) in the source file's directory, sorted.
fn find_tests(src: &Path) -> Vec<PathBuf> {
    let dir = src.parent().unwrap_or(Path::new("."));
    let mut tests: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().is_some_and(|e| e == "in")
                        && p.file_stem()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit()))
                })
                .collect()
        })
        .unwrap_or_default();
    tests.sort();
    tests
}

struct RunOutcome {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: Option<i32>,
    signal: Option<i32>,
    elapsed: Duration,
    timed_out: bool,
}

fn run_one(bin: &Path, input: &Path, timeout: Duration) -> RunOutcome {
    use std::os::unix::process::ExitStatusExt;

    let start = Instant::now();
    let spawn = || {
        Command::new(bin)
            .stdin(Stdio::from(fs::File::open(input)?))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    };
    let mut child = match spawn() {
        Ok(child) => child,
        Err(e) => {
            return RunOutcome {
                stdout: Vec::new(),
                stderr: format!("spawn failed: {e}").into_bytes(),
                code: None,
                signal: None,
                elapsed: start.elapsed(),
                timed_out: false,
            };
        }
    };

    // Reader threads avoid pipe-buffer deadlocks on large outputs.
    let mut out_pipe = child.stdout.take().unwrap();
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let mut err_pipe = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let mut timed_out = true;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                timed_out = false;
                break Some(status);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    break child.wait().ok();
                }
                std::thread::sleep(Duration::from_millis(3));
            }
            Err(_) => break None,
        }
    };

    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    RunOutcome {
        stdout,
        stderr,
        code: status.and_then(|s| s.code()),
        signal: status.and_then(|s| s.signal()),
        elapsed: start.elapsed(),
        timed_out,
    }
}

/// Print the verdict line for one test. Returns true on pass.
fn report(n: &str, expected: &Path, outcome: &RunOutcome) -> bool {
    let ms = outcome.elapsed.as_millis();
    if outcome.timed_out {
        eprintln!("{YELLOW}  ✗ {n}: TLE{RESET}{DIM} ({ms}ms){RESET}");
        return false;
    }
    if let Some(sig) = outcome.signal {
        eprintln!(
            "{RED}  ✗ {n}: RE (signal {}){RESET}{DIM} ({ms}ms){RESET}",
            signal_name(sig)
        );
        print_stderr(&outcome.stderr);
        return false;
    }
    if outcome.code != Some(0) {
        eprintln!(
            "{RED}  ✗ {n}: RE (exit {}){RESET}{DIM} ({ms}ms){RESET}",
            outcome.code.map_or("?".into(), |c| c.to_string())
        );
        print_stderr(&outcome.stderr);
        return false;
    }
    if !expected.exists() {
        eprintln!("{GREEN}  ✓ {n}{RESET}{DIM} ({ms}ms, no expected output){RESET}");
        return true;
    }

    let got = String::from_utf8_lossy(&outcome.stdout);
    let want = fs::read_to_string(expected).unwrap_or_default();
    match compare(&want, &got) {
        None => {
            eprintln!("{GREEN}  ✓ {n}{RESET}{DIM} ({ms}ms){RESET}");
            true
        }
        Some(diff) => {
            eprintln!("{RED}  ✗ {n}: WA{RESET}{DIM} ({ms}ms){RESET}");
            eprintln!("{DIM}      {diff}{RESET}");
            false
        }
    }
}

fn print_stderr(stderr: &[u8]) {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    if !text.is_empty() {
        for line in text.lines().take(10) {
            eprintln!("{DIM}      stderr | {line}{RESET}");
        }
    }
}

fn signal_name(sig: i32) -> String {
    match sig {
        4 => "SIGILL".into(),
        6 => "SIGABRT".into(),
        8 => "SIGFPE".into(),
        9 => "SIGKILL".into(),
        11 => "SIGSEGV".into(),
        13 => "SIGPIPE".into(),
        15 => "SIGTERM".into(),
        other => format!("{other}"),
    }
}

/// CPH-style correctness: trim the whole text, split lines, counts must
/// match, each line compared with both ends trimmed.
/// Returns None on match, or a short human-readable first difference.
fn compare(expected: &str, got: &str) -> Option<String> {
    let norm = |s: &str| -> Vec<String> {
        s.replace("\r\n", "\n")
            .trim()
            .split('\n')
            .map(|l| l.trim().to_string())
            .collect()
    };
    let want = norm(expected);
    let have = norm(got);
    for (i, (w, h)) in want.iter().zip(&have).enumerate() {
        if w != h {
            return Some(format!("line {}: expected \"{w}\" got \"{h}\"", i + 1));
        }
    }
    if want.len() != have.len() {
        return Some(format!("expected {} lines, got {}", want.len(), have.len()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::compare;

    #[test]
    fn exact_match() {
        assert_eq!(compare("3\n", "3\n"), None);
    }

    #[test]
    fn trailing_whitespace_ignored() {
        assert_eq!(compare("3 \n", "3\n"), None);
        assert_eq!(compare("1 2  \n4 5\n", "1 2\n4 5  \n"), None);
    }

    #[test]
    fn crlf_ignored() {
        assert_eq!(compare("a\r\nb\r\n", "a\nb\n"), None);
    }

    #[test]
    fn missing_final_newline_ignored() {
        assert_eq!(compare("a\nb\n", "a\nb"), None);
    }

    #[test]
    fn wrong_value_reported_with_line_number() {
        let diff = compare("1\n2\n3\n", "1\n2\n4\n").unwrap();
        assert!(diff.contains("line 3"), "{diff}");
        assert!(diff.contains("expected \"3\" got \"4\""), "{diff}");
    }

    #[test]
    fn extra_line_reported() {
        let diff = compare("1\n", "1\n2\n").unwrap();
        assert!(diff.contains("expected 1 lines, got 2"), "{diff}");
    }

    #[test]
    fn empty_both_match() {
        assert_eq!(compare("", ""), None);
        assert_eq!(compare("\n", ""), None);
    }
}
