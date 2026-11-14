mod backup;
mod cleaner;
mod env_check;
mod paths;
mod process;

use backup::BackupSummary;
use cleaner::{CleanReport, ScanStatus};
use env_check::EnvCheckReport;
use paths::PathContext;
use process::ProcessKillReport;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FullCleanResult {
    pub backup: BackupSummary,
    pub processes: ProcessKillReport,
    pub clean: CleanReport,
    pub env: EnvCheckReport,
}

#[tauri::command]
fn scan_status() -> Result<ScanStatus, String> {
    let ctx = PathContext::detect()?;
    Ok(cleaner::scan_traces(&ctx))
}

#[tauri::command]
fn check_environment() -> EnvCheckReport {
    env_check::check_environment()
}

#[tauri::command]
fn list_backups() -> Result<Vec<BackupSummary>, String> {
    let ctx = PathContext::detect()?;
    backup::list_backups(&ctx)
}

#[tauri::command]
fn restore_backup(backup_id: String) -> Result<usize, String> {
    let ctx = PathContext::detect()?;
    backup::restore_backup(&ctx, &backup_id)
}

#[tauri::command]
fn run_clean() -> Result<FullCleanResult, String> {
    let ctx = PathContext::detect()?;
    let backup = backup::create_backup(&ctx)?;
    let processes = process::kill_claude_processes();
    let clean = cleaner::clean_all(&ctx, true);
    let env = env_check::check_environment();

    Ok(FullCleanResult {
        backup,
        processes,
        clean,
        env,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_status,
            check_environment,
            list_backups,
            restore_backup,
            run_clean
        ])
        .run(tauri::generate_context!())
        .expect("error while running COMANDOS Claude Cleaner");
}
