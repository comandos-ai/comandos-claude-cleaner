use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathContext {
    pub home: PathBuf,
    pub app_support_claude_dirs: Vec<PathBuf>,
    pub mac_library: Option<PathBuf>,
    pub backup_root: PathBuf,
}

impl PathContext {
    pub fn detect() -> Result<Self, String> {
        let home = home_dir().ok_or_else(|| "Не удалось определить домашнюю папку пользователя".to_string())?;
        let mut app_support_claude_dirs = Vec::new();

        #[cfg(target_os = "macos")]
        let mac_library = {
            let lib = home.join("Library");
            let claude_dir = lib.join("Application Support").join("Claude");
            app_support_claude_dirs.push(claude_dir);
            Some(lib)
        };

        #[cfg(not(target_os = "macos"))]
        let mac_library = None;

        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = env::var("APPDATA") {
                app_support_claude_dirs.push(PathBuf::from(appdata).join("Claude"));
            }
            if let Ok(local) = env::var("LOCALAPPDATA") {
                let local_path = PathBuf::from(&local);
                // MSIX Claude Desktop installations live under Packages\Claude_*\LocalCache\Roaming\Claude
                if let Ok(entries) = std::fs::read_dir(local_path.join("Packages")) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("Claude_") {
                            let msix_claude = entry.path().join("LocalCache").join("Roaming").join("Claude");
                            if !app_support_claude_dirs.contains(&msix_claude) {
                                app_support_claude_dirs.push(msix_claude);
                            }
                        }
                    }
                }
                let anthropic_claude = local_path.join("AnthropicClaude");
                if anthropic_claude.exists() {
                    app_support_claude_dirs.push(anthropic_claude);
                }
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            app_support_claude_dirs.push(home.join(".config").join("Claude"));
        }

        let backup_root = home.join("Documents").join("COMANDOS-Claude-Backups");

        Ok(Self {
            home,
            app_support_claude_dirs,
            mac_library,
            backup_root,
        })
    }

    pub fn claude_code_dir(&self) -> PathBuf {
        self.home.join(".claude")
    }

    pub fn claude_json_path(&self) -> PathBuf {
        self.home.join(".claude.json")
    }

    pub fn desktop_config_paths(&self) -> Vec<PathBuf> {
        self.app_support_claude_dirs
            .iter()
            .map(|d| d.join("claude_desktop_config.json"))
            .collect()
    }
}

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(PathBuf::from))
    }
    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

pub fn is_protected_desktop_entry(name: &str) -> bool {
    name == "claude_desktop_config.json" || name.starts_with(".claude") || name.ends_with(".claude.json")
}

pub fn dir_size_bytes(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
