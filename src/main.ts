import "./style.css";
import { invoke } from "@tauri-apps/api/core";

interface ScanStatus {
  desktop_trace_count: number;
  desktop_trace_bytes: number;
  code_trace_count: number;
  has_mcp_config: boolean;
  has_projects_or_memory: boolean;
  os_name: string;
}

interface BackupSummary {
  id: string;
  path: string;
  created_at: string;
  item_count: number;
  size_bytes: number;
}

interface ProcessKillReport {
  stopped: string[];
  notes: string[];
}

interface CleanReport {
  desktop_removed: string[];
  code_removed: string[];
  keychain_removed: number;
  claude_json_scrubbed: boolean;
  freed_bytes: number;
  warnings: string[];
}

interface EnvCheckReport {
  ip: string;
  country_code: string;
  country_name: string;
  ip_timezone: string;
  local_timezone: string;
  vpn_ok: boolean;
  vpn_message: string;
  tz_ok: boolean;
  tz_hint: string;
}

interface FullCleanResult {
  backup: BackupSummary;
  processes: ProcessKillReport;
  clean: CleanReport;
  env: EnvCheckReport;
}

const osBadge = document.getElementById("os-badge") as HTMLSpanElement;
const refreshBtn = document.getElementById("refresh-btn") as HTMLButtonElement;
const tracePill = document.getElementById("trace-pill") as HTMLSpanElement;
const desktopStatus = document.getElementById("desktop-status") as HTMLSpanElement;
const codeStatus = document.getElementById("code-status") as HTMLSpanElement;
const protectStatus = document.getElementById("protect-status") as HTMLSpanElement;
const cleanBtn = document.getElementById("clean-btn") as HTMLButtonElement;
const cleanBtnLabel = document.getElementById("clean-btn-label") as HTMLSpanElement;

const vpnPill = document.getElementById("vpn-pill") as HTMLSpanElement;
const vpnText = document.getElementById("vpn-text") as HTMLParagraphElement;
const tzText = document.getElementById("tz-text") as HTMLParagraphElement;

const resultPanel = document.getElementById("result-panel") as HTMLElement;
const resultFreed = document.getElementById("result-freed") as HTMLSpanElement;
const resultSummary = document.getElementById("result-summary") as HTMLUListElement;
const backupPathEl = document.getElementById("backup-path") as HTMLElement;
const restoreBtn = document.getElementById("restore-btn") as HTMLButtonElement;
const resultLog = document.getElementById("result-log") as HTMLPreElement;

const existingBackupPanel = document.getElementById("existing-backup-panel") as HTMLElement;
const lastBackupDate = document.getElementById("last-backup-date") as HTMLSpanElement;
const lastBackupPath = document.getElementById("last-backup-path") as HTMLElement;
const restoreLastBtn = document.getElementById("restore-last-btn") as HTMLButtonElement;

const toast = document.getElementById("toast") as HTMLDivElement;

let lastBackupId: string | null = null;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} КБ`;
  const mb = kb / 1024;
  return `${mb.toFixed(1)} МБ`;
}

function showToast(message: string, kind: "ok" | "error") {
  toast.textContent = message;
  toast.className = `toast ${kind}`;
}

function clearToast() {
  toast.className = "toast hidden";
  toast.textContent = "";
}

function renderEnv(env: EnvCheckReport) {
  vpnPill.textContent = env.vpn_ok ? "VPN активен" : "Проверьте VPN";
  vpnPill.className = `pill ${env.vpn_ok ? "pill-ok" : "pill-warn"}`;
  vpnText.textContent = env.vpn_message;
  tzText.textContent = env.tz_hint;
}

async function loadStatus() {
  clearToast();
  refreshBtn.disabled = true;
  refreshBtn.dataset.loading = "true";
  refreshBtn.setAttribute("aria-busy", "true");
  try {
    const [scan, backups, env] = await Promise.all([
      invoke<ScanStatus>("scan_status"),
      invoke<BackupSummary[]>("list_backups"),
      invoke<EnvCheckReport>("check_environment"),
    ]);

    osBadge.textContent = scan.os_name;

    const totalTraces = scan.desktop_trace_count + scan.code_trace_count;
    if (totalTraces > 0) {
      tracePill.textContent = `Найдено следов: ${totalTraces}`;
      tracePill.className = "pill pill-warn";
    } else {
      tracePill.textContent = "Следов не найдено";
      tracePill.className = "pill pill-ok";
    }

    desktopStatus.textContent =
      scan.desktop_trace_count > 0
        ? `Найдено: ${scan.desktop_trace_count} (${formatBytes(scan.desktop_trace_bytes)})`
        : "Чисто";
    codeStatus.textContent =
      scan.code_trace_count > 0
        ? `Найдено: ${scan.code_trace_count} (+ связка ключей)`
        : "Локальных файлов нет (+ проверка ключей)";

    const protectedItems: string[] = [];
    if (scan.has_mcp_config) protectedItems.push("MCP");
    if (scan.has_projects_or_memory) protectedItems.push("проекты и MEMORY.md");
    protectStatus.textContent =
      protectedItems.length > 0
        ? `Под защитой: ${protectedItems.join(", ")}`
        : "Новых конфигов пока нет";

    renderEnv(env);

    if (backups.length > 0 && resultPanel.classList.contains("hidden")) {
      const latest = backups[0];
      lastBackupId = latest.id;
      lastBackupDate.textContent = latest.created_at;
      lastBackupPath.textContent = latest.path;
      lastBackupPath.title = latest.path;
      existingBackupPanel.classList.remove("hidden");
    } else if (backups.length === 0) {
      existingBackupPanel.classList.add("hidden");
    }
  } catch (err) {
    showToast(`Ошибка диагностики: ${String(err)}`, "error");
  } finally {
    refreshBtn.disabled = false;
    delete refreshBtn.dataset.loading;
    refreshBtn.removeAttribute("aria-busy");
  }
}

async function handleClean() {
  clearToast();
  cleanBtn.disabled = true;
  cleanBtn.setAttribute("aria-busy", "true");
  refreshBtn.disabled = true;
  cleanBtnLabel.textContent = "Создаю бэкап и очищаю…";

  try {
    const res = await invoke<FullCleanResult>("run_clean");
    lastBackupId = res.backup.id;
    existingBackupPanel.classList.add("hidden");

    resultFreed.textContent = `Освобождено ${formatBytes(res.clean.freed_bytes)}`;
    backupPathEl.textContent = res.backup.path;
    backupPathEl.title = res.backup.path;

    const summaryLines: string[] = [
      `Сохранено в бэкап: ${res.backup.item_count} эл. (${formatBytes(res.backup.size_bytes)})`,
      res.processes.stopped.length > 0
        ? `Остановлены процессы: ${res.processes.stopped.join(", ")}`
        : "Активные процессы Claude не были запущены",
      `Удалено следов Claude Desktop: ${res.clean.desktop_removed.length}`,
      `Удалено следов Claude Code: ${res.clean.code_removed.length} файлов/папок, ключей ОС: ${res.clean.keychain_removed}${res.clean.claude_json_scrubbed ? ", ~/.claude.json очищен от userID/OAuth" : ""}`,
    ];

    if (res.clean.warnings.length > 0) {
      summaryLines.push(`Предупреждения: ${res.clean.warnings.join("; ")}`);
    }

    resultSummary.innerHTML = "";
    for (const line of summaryLines) {
      const li = document.createElement("li");
      li.textContent = line;
      resultSummary.appendChild(li);
    }

    const logParts = [
      ...res.clean.desktop_removed.map((p) => `[Desktop] ${p}`),
      ...res.clean.code_removed.map((p) => `[Code] ${p}`),
    ];
    resultLog.textContent =
      logParts.length > 0 ? logParts.join("\n") : "Файловых следов для удаления не обнаружено.";

    resultPanel.classList.remove("hidden");
    renderEnv(res.env);
    await loadStatus();
  } catch (err) {
    showToast(`Не удалось выполнить очистку: ${String(err)}`, "error");
  } finally {
    cleanBtn.disabled = false;
    cleanBtn.removeAttribute("aria-busy");
    refreshBtn.disabled = false;
    cleanBtnLabel.textContent = "Очистить следы";
  }
}

async function handleRestore(btn: HTMLButtonElement) {
  if (!lastBackupId) return;
  clearToast();
  btn.disabled = true;
  btn.setAttribute("aria-busy", "true");
  const orig = btn.textContent;
  btn.textContent = "Восстанавливаю…";
  try {
    const count = await invoke<number>("restore_backup", { backupId: lastBackupId });
    showToast(`Восстановлено элементов из бэкапа: ${count}`, "ok");
    await loadStatus();
  } catch (err) {
    showToast(`Ошибка восстановления: ${String(err)}`, "error");
  } finally {
    btn.disabled = false;
    btn.removeAttribute("aria-busy");
    btn.textContent = orig;
  }
}

cleanBtn.addEventListener("click", () => void handleClean());
refreshBtn.addEventListener("click", () => void loadStatus());
restoreBtn.addEventListener("click", () => void handleRestore(restoreBtn));
restoreLastBtn.addEventListener("click", () => void handleRestore(restoreLastBtn));

void loadStatus();
