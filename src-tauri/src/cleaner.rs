use crate::paths::{dir_size_bytes, is_protected_desktop_entry, PathContext};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    pub desktop_trace_count: usize,
    pub desktop_trace_bytes: u64,
    pub code_trace_count: usize,
    pub has_mcp_config: bool,
    pub has_projects_or_memory: bool,
    pub os_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanReport {
    pub desktop_removed: Vec<String>,
    pub code_removed: Vec<String>,
    pub keychain_removed: usize,
    pub claude_json_scrubbed: bool,
    pub freed_bytes: u64,
    pub warnings: Vec<String>,
}

const DESKTOP_ROOT_TARGETS: &[&str] = &[
    "Cookies",
    "Cookies-journal",
    "IndexedDB",
    "Local Storage",
    "Session Storage",
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "DawnCache",
    "Network Action Predictor",
    "TransportSecurity",
    "DIPS",
    "DIPS-wal",
    "DIPS-shm",
    "SharedStorage",
    "SharedStorage-wal",
    "blob_storage",
    "WebStorage",
    "Service Worker",
    "Crashpad",
    "sentry",
    "ant-device-registry.json",
    "ccd-ids.json",
    "dispatch-footprint-census.json",
    "Local State",
    "Preferences",
];

const DESKTOP_NETWORK_TARGETS: &[&str] = &[
    "Cookies",
    "Cookies-journal",
    "Network Persistent State",
    "TransportSecurity",
];

const DESKTOP_NETWORK_PREFIXES: &[&str] = &[
    "Trust Tokens",
    "Reporting and Nel",
    "SCT Auditing Pending Reports",
];

const CLAUDE_JSON_AUTH_KEYS: &[&str] = &[
    "userID",
    "oauthAccount",
    "cachedStatsigGates",
    "tipsHistory",
    "lastReleaseNotesSeen",
    "anonymousId",
    "firstStartTime",
    "numStartups",
];

pub fn scan_traces(ctx: &PathContext) -> ScanStatus {
    let mut desktop_trace_count = 0usize;
    let mut desktop_trace_bytes = 0u64;

    for target in collect_desktop_targets(ctx) {
        if target.exists() {
            desktop_trace_count += 1;
            desktop_trace_bytes += dir_size_bytes(&target);
        }
    }

    let mut code_trace_count = 0usize;
    let code_dir = ctx.claude_code_dir();
    for p in [
        code_dir.join(".credentials.json"),
        code_dir.join("debug"),
        code_dir.join("telemetry"),
    ] {
        if p.exists() {
            code_trace_count += 1;
        }
    }
    if let Ok(entries) = fs::read_dir(&code_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("statsig") {
                code_trace_count += 1;
            }
        }
    }
    if claude_json_has_auth_keys(&ctx.claude_json_path()) {
        code_trace_count += 1;
    }

    let has_mcp_config = ctx.desktop_config_paths().iter().any(|p| p.is_file());
    let has_projects_or_memory = code_dir.join("projects").is_dir()
        || code_dir.join("MEMORY.md").is_file()
        || code_dir.join("CLAUDE.md").is_file();

    let os_name = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
    .to_string();

    ScanStatus {
        desktop_trace_count,
        desktop_trace_bytes,
        code_trace_count,
        has_mcp_config,
        has_projects_or_memory,
        os_name,
    }
}

pub fn clean_all(ctx: &PathContext, clear_system_keychain: bool) -> CleanReport {
    let mut desktop_removed = Vec::new();
    let mut code_removed = Vec::new();
    let mut warnings = Vec::new();
    let mut freed_bytes = 0u64;

    // 1. Clean Claude Desktop traces
    for target in collect_desktop_targets(ctx) {
        if let Some(name) = target.file_name().and_then(|n| n.to_str()) {
            if is_protected_desktop_entry(name) {
                continue;
            }
        }
        if target.exists() {
            let sz = dir_size_bytes(&target);
            match remove_any(&target) {
                Ok(()) => {
                    freed_bytes += sz;
                    desktop_removed.push(target.to_string_lossy().to_string());
                }
                Err(e) => warnings.push(e),
            }
        }
    }

    // 2. Clean Claude Code CLI files in ~/.claude
    let code_dir = ctx.claude_code_dir();
    for rel in [".credentials.json", "debug", "telemetry"] {
        let p = code_dir.join(rel);
        if p.exists() {
            let sz = dir_size_bytes(&p);
            match remove_any(&p) {
                Ok(()) => {
                    freed_bytes += sz;
                    code_removed.push(p.to_string_lossy().to_string());
                }
                Err(e) => warnings.push(e),
            }
        }
    }
    if let Ok(entries) = fs::read_dir(&code_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("statsig") {
                let p = entry.path();
                let sz = dir_size_bytes(&p);
                match remove_any(&p) {
                    Ok(()) => {
                        freed_bytes += sz;
                        code_removed.push(p.to_string_lossy().to_string());
                    }
                    Err(e) => warnings.push(e),
                }
            }
        }
    }

    // 3. Scrub auth/device fields from ~/.claude.json without deleting projects/MCP
    let claude_json_scrubbed = match scrub_claude_json(&ctx.claude_json_path()) {
        Ok(changed) => changed,
        Err(e) => {
            warnings.push(e);
            false
        }
    };

    // 4. Clean OS credential store (Keychain / Windows Credential Manager)
    let keychain_removed = if clear_system_keychain {
        clean_os_credentials(&mut warnings)
    } else {
        0
    };

    CleanReport {
        desktop_removed,
        code_removed,
        keychain_removed,
        claude_json_scrubbed,
        freed_bytes,
        warnings,
    }
}

fn collect_desktop_targets(ctx: &PathContext) -> Vec<PathBuf> {
    let mut targets = Vec::new();

    for base in &ctx.app_support_claude_dirs {
        if !base.exists() {
            continue;
        }
        for name in DESKTOP_ROOT_TARGETS {
            targets.push(base.join(name));
        }

        // Cache/Cache_Data (newer Windows/Chromium layout)
        let cache_data = base.join("Cache").join("Cache_Data");
        if cache_data.exists() {
            targets.push(cache_data);
        }

        // Network/*
        let net_dir = base.join("Network");
        for name in DESKTOP_NETWORK_TARGETS {
            targets.push(net_dir.join(name));
        }
        if let Ok(entries) = fs::read_dir(&net_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if DESKTOP_NETWORK_PREFIXES.iter().any(|p| fname.starts_with(p)) {
                    targets.push(entry.path());
                }
            }
        }

        // Partitions/*
        let partitions_dir = base.join("Partitions");
        if let Ok(parts) = fs::read_dir(&partitions_dir) {
            for part in parts.flatten() {
                if part.path().is_dir() {
                    targets.push(part.path());
                }
            }
        }
    }

    // macOS system caches, HTTPStorages, Preferences, SavedState, WebKit
    if let Some(lib) = &ctx.mac_library {
        for sub in ["HTTPStorages", "Caches"] {
            let dir = lib.join(sub);
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.starts_with("com.anthropic.claudefordesktop") {
                        targets.push(entry.path());
                    }
                }
            }
        }
        targets.push(
            lib.join("Preferences")
                .join("com.anthropic.claudefordesktop.plist"),
        );
        targets.push(
            lib.join("Saved Application State")
                .join("com.anthropic.claudefordesktop.savedState"),
        );
        targets.push(
            lib.join("WebKit")
                .join("com.anthropic.claudefordesktop"),
        );
    }

    targets
}

fn claude_json_has_auth_keys(path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    CLAUDE_JSON_AUTH_KEYS.iter().any(|k| map.contains_key(*k))
}

fn scrub_claude_json(path: &Path) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Не удалось прочитать {}: {}", path.display(), e))?;
    let mut val: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Некорректный JSON в {}: {}", path.display(), e))?;

    let mut changed = false;
    if let Value::Object(ref mut map) = val {
        for key in CLAUDE_JSON_AUTH_KEYS {
            if map.remove(*key).is_some() {
                changed = true;
            }
        }
    }

    if changed {
        let updated = serde_json::to_string_pretty(&val)
            .map_err(|e| format!("Ошибка сериализации {}: {}", path.display(), e))?;
        fs::write(path, updated)
            .map_err(|e| format!("Не удалось записать {}: {}", path.display(), e))?;
    }
    Ok(changed)
}

fn clean_os_credentials(warnings: &mut Vec<String>) -> usize {
    let mut removed = 0usize;

    #[cfg(target_os = "macos")]
    {
        // Delete exact default service name first, then any suffixed entries via dump-keychain
        for _ in 0..10 {
            let ok = Command::new("security")
                .args(["delete-generic-password", "-s", "Claude Code-credentials"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                removed += 1;
            } else {
                break;
            }
        }

        if let Ok(dump) = Command::new("security").args(["dump-keychain"]).output() {
            let text = String::from_utf8_lossy(&dump.stdout);
            let mut services = Vec::new();
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("\"svce\"<blob>=\"") {
                    if let Some(svc) = rest.strip_suffix('"') {
                        if svc.starts_with("Claude Code-credentials") && svc != "Claude Code-credentials" && !services.contains(&svc.to_string()) {
                            services.push(svc.to_string());
                        }
                    }
                }
            }
            for svc in services {
                let ok = Command::new("security")
                    .args(["delete-generic-password", "-s", &svc])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if ok {
                    removed += 1;
                }
            }
        } else {
            warnings.push("Не удалось прочитать macOS Keychain через security dump-keychain".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("cmdkey").args(["/list"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(idx) = trimmed.find("Claude Code-credentials") {
                    let target = trimmed[idx..].trim();
                    let ok = Command::new("cmdkey")
                        .args(["/delete", target])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if ok {
                        removed += 1;
                    }
                }
            }
        } else {
            warnings.push("Не удалось проверить Windows Credential Manager (cmdkey)".to_string());
        }
    }

    let _ = warnings;
    removed
}

fn remove_any(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|e| format!("Не удалось удалить папку {}: {}", path.display(), e))
    } else if path.exists() {
        fs::remove_file(path)
            .map_err(|e| format!("Не удалось удалить файл {}: {}", path.display(), e))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn clean_removes_traces_and_preserves_mcp_and_memory() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let lib = home.join("Library");
        let desktop = lib.join("Application Support/Claude");
        fs::create_dir_all(desktop.join("Network")).unwrap();
        fs::create_dir_all(desktop.join("Service Worker/CacheStorage")).unwrap();
        fs::create_dir_all(desktop.join("Partitions/main")).unwrap();
        fs::create_dir_all(lib.join("HTTPStorages/com.anthropic.claudefordesktop")).unwrap();
        fs::create_dir_all(lib.join("Caches/com.anthropic.claudefordesktop.ShipIt")).unwrap();

        fs::write(desktop.join("claude_desktop_config.json"), r#"{"mcpServers":{"pg":{}}}"#).unwrap();
        fs::write(desktop.join("ant-device-registry.json"), r#"{"deviceId":"abc"}"#).unwrap();
        fs::write(desktop.join("ccd-ids.json"), "{}").unwrap();
        fs::write(desktop.join("Network/Cookies"), "cookie").unwrap();
        fs::write(desktop.join("Network/Trust Tokens"), "token").unwrap();

        let code_dir = home.join(".claude");
        fs::create_dir_all(code_dir.join("projects/p1")).unwrap();
        fs::create_dir_all(code_dir.join("debug")).unwrap();
        fs::create_dir_all(code_dir.join("telemetry")).unwrap();
        fs::write(code_dir.join("MEMORY.md"), "important").unwrap();
        fs::write(code_dir.join(".credentials.json"), "secret").unwrap();
        fs::write(code_dir.join("statsig.cached"), "statsig").unwrap();

        let claude_json = home.join(".claude.json");
        fs::write(
            &claude_json,
            r#"{"userID":"u-1","oauthAccount":{"email":"a@b.c"},"projects":{"/tmp":{"lastCost":1}}}"#,
        )
        .unwrap();

        let ctx = PathContext {
            home: home.clone(),
            app_support_claude_dirs: vec![desktop.clone()],
            mac_library: Some(lib.clone()),
            backup_root: home.join("Documents/COMANDOS-Claude-Backups"),
        };

        let before = scan_traces(&ctx);
        assert!(before.desktop_trace_count >= 6);
        assert!(before.code_trace_count >= 4);
        assert!(before.has_mcp_config);
        assert!(before.has_projects_or_memory);

        let report = clean_all(&ctx, false);
        assert!(report.warnings.is_empty());
        assert!(report.claude_json_scrubbed);

        // Traces gone
        assert!(!desktop.join("ant-device-registry.json").exists());
        assert!(!desktop.join("ccd-ids.json").exists());
        assert!(!desktop.join("Service Worker").exists());
        assert!(!desktop.join("Partitions/main").exists());
        assert!(!desktop.join("Network/Cookies").exists());
        assert!(!desktop.join("Network/Trust Tokens").exists());
        assert!(!lib.join("HTTPStorages/com.anthropic.claudefordesktop").exists());
        assert!(!code_dir.join(".credentials.json").exists());
        assert!(!code_dir.join("debug").exists());
        assert!(!code_dir.join("telemetry").exists());
        assert!(!code_dir.join("statsig.cached").exists());

        // Protected preserved
        assert!(desktop.join("claude_desktop_config.json").is_file());
        assert!(code_dir.join("MEMORY.md").is_file());
        assert!(code_dir.join("projects/p1").is_dir());

        let scrubbed: Value = serde_json::from_str(&fs::read_to_string(&claude_json).unwrap()).unwrap();
        assert!(scrubbed.get("userID").is_none());
        assert!(scrubbed.get("oauthAccount").is_none());
        assert!(scrubbed.get("projects").is_some());
    }
}
