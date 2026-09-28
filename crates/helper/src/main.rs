mod config;
mod http;
mod judge;
mod lsp;
mod problem;
mod tasks;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Worktree root shared between the LSP reader (learns it from the
/// `initialize` request) and the HTTP server (needs it for every POST).
pub type RootHolder = Arc<RwLock<Option<PathBuf>>>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") => serve(),
        Some("judge") => judge(&args[2..]),
        _ => {
            eprintln!("usage: cph-helper <serve|judge> ...");
            std::process::exit(2);
        }
    }
}

fn judge(args: &[String]) {
    use std::path::Path;

    let mut compile_only = false;
    let mut file = None;
    for arg in args {
        match arg.as_str() {
            "--compile-only" => compile_only = true,
            other => file = Some(other.to_string()),
        }
    }
    let Some(file) = file else {
        eprintln!("usage: cph-helper judge [--compile-only] <solution.cpp>");
        std::process::exit(2);
    };
    std::process::exit(judge::run(
        Path::new(&file),
        &judge::Options { compile_only },
    ));
}

fn serve() {
    let root: RootHolder = Arc::new(RwLock::new(
        std::env::var_os("CPH_ROOT").map(PathBuf::from),
    ));
    let port: u16 = std::env::var("CPH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(27121);

    let http_root = Arc::clone(&root);
    std::thread::spawn(move || {
        if let Err(e) = http::serve(http_root, port) {
            eprintln!("[cph] http server: {e}");
            std::process::exit(1);
        }
    });

    // LSP runs on the main thread (stdin/stdout belong to Zed).
    lsp::run(root);

    // stdin closed (editor gone): keep serving HTTP until Zed kills us.
    std::thread::park();
}
