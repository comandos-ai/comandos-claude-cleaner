use crate::paths::{dir_size_bytes, PathContext};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupItem {
    pub label: String,
    pub original_path: String,
    pub relative_path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub id: String,
    pub created_at: String,
    pub items: Vec<BackupItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSummary {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub item_count: usize,
    pub size_bytes: u64,
}

pub fn create_backup(ctx: &PathContext) -> Result<BackupSummary, String> {
    let now = Local::now();
    let id = format!("backup-{}", now.format("%Y%m%d-%H%M%S"));
    let backup_dir = ctx.backup_root.join(&id);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Не удалось создать папку бэкапа {}: {}", backup_dir.display(), e))?;

    let mut items = Vec::new();

    // 1. Desktop MCP configs
    for (idx, cfg_path) in ctx.desktop_config_paths().iter().enumerate() {
        if cfg_path.is_file() {
            let rel = format!("desktop/claude_desktop_config_{}.json", idx);
            let dst = backup_dir.join(&rel);
            copy_file_with_parents(cfg_path, &dst)?;
            items.push(BackupItem {
                label: "Настройки MCP (Claude Desktop)".to_string(),
                original_path: cfg_path.to_string_lossy().to_string(),
                relative_path: rel,
                is_dir: false,
            });
        }
    }

    // 2. ~/.claude.json
    let claude_json = ctx.claude_json_path();
    if claude_json.is_file() {
        let rel = "claude_code/dot_claude.json".to_string();
        let dst = backup_dir.join(&rel);
        copy_file_with_parents(&claude_json, &dst)?;
        items.push(BackupItem {
            label: "Файл ~/.claude.json".to_string(),
            original_path: claude_json.to_string_lossy().to_string(),
            relative_path: rel,
            is_dir: false,
        });
    }

    // 3. Important ~/.claude files & directories
    let code_dir = ctx.claude_code_dir();
    let important_files = [
        ("MEMORY.md", "Память Claude Code (MEMORY.md)"),
        ("CLAUDE.md", "Глобальная инструкция (CLAUDE.md)"),
        ("settings.json", "Настройки Claude Code (settings.json)"),
    ];
    for (fname, label) in important_files {
        let src = code_dir.join(fname);
        if src.is_file() {
            let rel = format!("claude_code/{}", fname);
            copy_file_with_parents(&src, &backup_dir.join(&rel))?;
            items.push(BackupItem {
                label: label.to_string(),
                original_path: src.to_string_lossy().to_string(),
                relative_path: rel,
                is_dir: false,
            });
        }
    }

    let important_dirs = [
        ("projects", "Проекты и память (~/.claude/projects)"),
        ("plans", "Планы (~/.claude/plans)"),
        ("commands", "Команды (~/.claude/commands)"),
        ("skills", "Навыки (~/.claude/skills)"),
    ];
    for (dname, label) in important_dirs {
        let src = code_dir.join(dname);
        if src.is_dir() {
            let rel = format!("claude_code/{}", dname);
            copy_dir_recursive(&src, &backup_dir.join(&rel))?;
            items.push(BackupItem {
                label: label.to_string(),
                original_path: src.to_string_lossy().to_string(),
                relative_path: rel,
                is_dir: true,
            });
        }
    }

    let manifest = BackupManifest {
        id: id.clone(),
        created_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        items: items.clone(),
    };
    let manifest_path = backup_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Ошибка сериализации manifest.json: {}", e))?;
    fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("Не удалось записать manifest.json: {}", e))?;

    Ok(BackupSummary {
        id,
        path: backup_dir.to_string_lossy().to_string(),
        created_at: manifest.created_at,
        item_count: items.len(),
        size_bytes: dir_size_bytes(&backup_dir),
    })
}

pub fn list_backups(ctx: &PathContext) -> Result<Vec<BackupSummary>, String> {
    if !ctx.backup_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&ctx.backup_root)
        .map_err(|e| format!("Не удалось прочитать папку бэкапов: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if !manifest_path.is_file() {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(&manifest_path) {
            if let Ok(m) = serde_json::from_str::<BackupManifest>(&raw) {
                out.push(BackupSummary {
                    id: m.id,
                    path: path.to_string_lossy().to_string(),
                    created_at: m.created_at,
                    item_count: m.items.len(),
                    size_bytes: dir_size_bytes(&path),
                });
            }
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

pub fn restore_backup(ctx: &PathContext, backup_id: &str) -> Result<usize, String> {
    if backup_id.contains("..") || backup_id.contains('/') || backup_id.contains('\\') {
        return Err("Некорректный идентификатор бэкапа".to_string());
    }
    let backup_dir = ctx.backup_root.join(backup_id);
    let manifest_path = backup_dir.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Не найден manifest.json бэкапа: {}", e))?;
    let manifest: BackupManifest = serde_json::from_str(&raw)
        .map_err(|e| format!("Повреждён manifest.json: {}", e))?;

    let mut restored = 0;
    for item in manifest.items {
        let src = backup_dir.join(&item.relative_path);
        let dst = PathBuf::from(&item.original_path);
        if item.is_dir {
            if src.is_dir() {
                copy_dir_recursive(&src, &dst)?;
                restored += 1;
            }
        } else if src.is_file() {
            copy_file_with_parents(&src, &dst)?;
            restored += 1;
        }
    }
    Ok(restored)
}

fn copy_file_with_parents(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Не удалось создать папку {}: {}", parent.display(), e))?;
    }
    fs::copy(src, dst).map_err(|e| {
        format!(
            "Не удалось скопировать {} -> {}: {}",
            src.display(),
            dst.display(),
            e
        )
    })?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Не удалось создать папку {}: {}", dst.display(), e))?;
    for entry in walkdir::WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| format!("Ошибка пути: {}", e))?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| format!("Не удалось создать {}: {}", target.display(), e))?;
        } else if entry.file_type().is_file() {
            copy_file_with_parents(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn backup_and_restore_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let desktop_dir = home.join("Library/Application Support/Claude");
        fs::create_dir_all(&desktop_dir).unwrap();
        let mcp_cfg = desktop_dir.join("claude_desktop_config.json");
        fs::write(&mcp_cfg, r#"{"mcpServers":{"test":{}}}"#).unwrap();

        let code_dir = home.join(".claude/projects/demo");
        fs::create_dir_all(&code_dir).unwrap();
        let mem = home.join(".claude/MEMORY.md");
        fs::write(&mem, "# Memory\nkeep me").unwrap();
        let proj_file = code_dir.join("notes.md");
        fs::write(&proj_file, "project data").unwrap();

        let claude_json = home.join(".claude.json");
        fs::write(&claude_json, r#"{"userID":"old-123"}"#).unwrap();

        let ctx = PathContext {
            home: home.clone(),
            app_support_claude_dirs: vec![desktop_dir],
            mac_library: None,
            backup_root: home.join("Documents/COMANDOS-Claude-Backups"),
        };

        let summary = create_backup(&ctx).unwrap();
        assert_eq!(summary.item_count, 4);

        // Simulate modification/removal
        fs::remove_file(&mcp_cfg).unwrap();
        fs::write(&claude_json, "{}").unwrap();
        fs::remove_file(&mem).unwrap();

        let restored = restore_backup(&ctx, &summary.id).unwrap();
        assert_eq!(restored, 4);
        assert!(mcp_cfg.is_file());
        assert_eq!(fs::read_to_string(&mem).unwrap(), "# Memory\nkeep me");
        assert_eq!(fs::read_to_string(&claude_json).unwrap(), r#"{"userID":"old-123"}"#);
    }
}
