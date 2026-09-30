use std::path::Path;

use serde::Deserialize;

/// Project settings from cph.toml (every key optional; defaults below).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub cxx: Option<String>,
    pub cxxflags: Option<String>,
    pub timeout_ms: Option<u64>,
    pub notify: Option<bool>,
}

pub const DEFAULT_CXX: &str = "g++-16";
pub const DEFAULT_CXXFLAGS: &str = "-std=c++17 -O2 -Wall -Winvalid-pch";
pub const DEFAULT_TIMEOUT_MS: u64 = 3000;

impl Config {
    pub fn cxx(&self) -> &str {
        self.cxx.as_deref().unwrap_or(DEFAULT_CXX)
    }

    pub fn cxxflags(&self) -> Vec<String> {
        self.cxxflags
            .as_deref()
            .unwrap_or(DEFAULT_CXXFLAGS)
            .split_whitespace()
            .map(str::to_string)
            .collect()
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)
    }

    pub fn notify(&self) -> bool {
        self.notify.unwrap_or(true)
    }
}

/// Find the nearest cph.toml walking up from `start` (a file or dir).
pub fn load(start: &Path) -> Config {
    let mut dir = if start.is_dir() {
        Some(start)
    } else {
        start.parent()
    };
    while let Some(d) = dir {
        let candidate = d.join("cph.toml");
        if candidate.is_file() {
            return read(&candidate);
        }
        dir = d.parent();
    }
    Config::default()
}

fn read(path: &Path) -> Config {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Config::default();
    };
    match toml::from_str(&contents) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[cph] ignoring {}: {e}", path.display());
            Config::default()
        }
    }
}
