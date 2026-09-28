use std::fs;

use zed_extension_api::{
    self as zed, settings::LspSettings, Architecture, GithubReleaseOptions, LanguageServerId, Os,
};

/// GitHub repo that publishes cph-helper release binaries.
const GITHUB_REPO: &str = "might-guy106/cph-for-zed";

/// Re-check GitHub for a newer helper at most once a day.
const RELEASE_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

const WORK_BIN: &str = "cph-helper-bin/cph-helper";
const WORK_VERSION: &str = "cph-helper-bin/version";
const WORK_LAST_CHECK: &str = "cph-helper-bin/last-check";

struct Cph {
    cached_binary: Option<String>,
}

impl Cph {
    /// Where the helper binary comes from, in order of preference.
    fn binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<String> {
        if let Some(path) = self.cached_binary.clone() {
            return Ok(path);
        }

        // 1. explicit user override: settings.lsp.cph.binary.path
        if let Some(path) = LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .ok()
            .and_then(|settings| settings.binary)
            .and_then(|binary| binary.path)
        {
            return Ok(path);
        }

        // 2. cph-helper on PATH
        if let Some(path) = worktree.which("cph-helper") {
            return Ok(path);
        }

        // 3. dev checkout of the cph repo opened as the worktree:
        //    <worktree>/cph/target/debug/cph-helper. The WASM sandbox cannot
        //    stat paths outside its work dir, but it can read worktree files,
        //    so extension.toml doubles as the "this is the dev repo" marker.
        let root = worktree.root_path();
        if worktree.read_text_file("cph/extension.toml").is_ok() {
            return Ok(format!("{root}/cph/target/debug/cph-helper"));
        }

        // 4. download a released binary (cached in the extension work dir)
        let path = self.download_binary(language_server_id)?;
        self.cached_binary = Some(path.clone());
        Ok(path)
    }

    fn download_binary(&self, language_server_id: &LanguageServerId) -> zed::Result<String> {
        let (platform, arch) = zed::current_platform();
        let asset_stem = asset_stem(platform, arch)
            .ok_or_else(|| format!("unsupported platform: {platform:?}/{arch:?}"))?;

        let cached_exists = fs::metadata(WORK_BIN).map(|m| m.is_file()).unwrap_or(false);
        if cached_exists && cache_is_fresh() {
            return Ok(WORK_BIN.to_string());
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            GITHUB_REPO,
            GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let cached_version = fs::read_to_string(WORK_VERSION).unwrap_or_default();
        if cached_exists && cached_version.trim() == release.version {
            touch_last_check();
            return Ok(WORK_BIN.to_string());
        }

        let asset_name = format!("{asset_stem}.tar.gz");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("release {} has no asset {asset_name}", release.version))?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );
        let _ = fs::remove_dir_all("cph-helper-bin");
        zed::download_file(
            &asset.download_url,
            "cph-helper-bin",
            zed::DownloadedFileType::GzipTar,
        )
        .map_err(|e| format!("download failed: {e}"))?;
        zed::make_file_executable(WORK_BIN)?;
        fs::write(WORK_VERSION, &release.version).map_err(|e| e.to_string())?;
        touch_last_check();
        Ok(WORK_BIN.to_string())
    }
}

fn asset_stem(os: Os, arch: Architecture) -> Option<String> {
    let arch = match arch {
        Architecture::Aarch64 => "aarch64",
        Architecture::X8664 => "x86_64",
        Architecture::X86 => return None,
    };
    let os = match os {
        Os::Mac => "apple-darwin",
        Os::Linux => "unknown-linux-gnu",
        Os::Windows => return None, // not built yet
    };
    Some(format!("cph-helper-{arch}-{os}"))
}

fn cache_is_fresh() -> bool {
    fs::read_to_string(WORK_LAST_CHECK)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .is_some_and(|ts| now_secs().saturating_sub(ts) < RELEASE_CHECK_INTERVAL_SECS)
}

fn touch_last_check() {
    let _ = fs::write(WORK_LAST_CHECK, now_secs().to_string());
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl zed::Extension for Cph {
    fn new() -> Self {
        Self {
            cached_binary: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = self.binary_path(language_server_id, worktree)?;
        let root = worktree.root_path();
        Ok(zed::Command {
            command,
            args: vec!["serve".into()],
            env: vec![("CPH_ROOT".into(), root)],
        })
    }
}

zed::register_extension!(Cph);
