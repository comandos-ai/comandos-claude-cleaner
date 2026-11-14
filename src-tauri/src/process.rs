use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessKillReport {
    pub stopped: Vec<String>,
    pub notes: Vec<String>,
}

pub fn kill_claude_processes() -> ProcessKillReport {
    let mut stopped = Vec::new();
    let mut notes = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let exact_names = ["Claude", "claude"];
        let pattern_names = ["Claude Helper", "chrome-native-host"];

        for name in exact_names {
            if let Ok(status) = Command::new("pkill").args(["-x", name]).status() {
                if status.success() {
                    stopped.push(name.to_string());
                }
            }
        }
        for pat in pattern_names {
            if let Ok(status) = Command::new("pkill").args(["-f", pat]).status() {
                if status.success() {
                    stopped.push(pat.to_string());
                }
            }
        }

        if !stopped.is_empty() {
            thread::sleep(Duration::from_millis(600));
            for name in exact_names {
                let _ = Command::new("pkill").args(["-9", "-x", name]).status();
            }
            for pat in pattern_names {
                let _ = Command::new("pkill").args(["-9", "-f", pat]).status();
            }
        } else {
            notes.push("Активные процессы Claude не найдены".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        let images = [
            "Claude.exe",
            "Claude Helper.exe",
            "claude.exe",
            "chrome-native-host.exe",
        ];
        for img in images {
            if let Ok(out) = Command::new("taskkill")
                .args(["/F", "/IM", img, "/T"])
                .output()
            {
                if out.status.success() {
                    stopped.push(img.to_string());
                }
            }
        }
        if !stopped.is_empty() {
            thread::sleep(Duration::from_millis(600));
        } else {
            notes.push("Активные процессы Claude не найдены".to_string());
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        notes.push("Остановка процессов поддерживается на macOS и Windows".to_string());
    }

    ProcessKillReport { stopped, notes }
}
