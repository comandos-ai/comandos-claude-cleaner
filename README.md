# COMANDOS Claude Cleaner

Лёгкая утилита для резидентов Клуба COMANDOS: безопасная очистка следов **Claude Desktop** и **Claude Code CLI** на macOS и Windows в один клик.

## Что делает программа

1. **Автоматический бэкап перед чисткой:** сохраняет в `~/Documents/COMANDOS-Claude-Backups/backup-YYYYMMDD-HHMMSS/` (macOS) или `%USERPROFILE%\Documents\COMANDOS-Claude-Backups\...` (Windows) настройки MCP (`claude_desktop_config.json`), папку проектов `~/.claude/projects`, `MEMORY.md`, `CLAUDE.md`, `plans`, `settings.json` и `~/.claude.json`. Кнопка **«Откатить»** возвращает файлы обратно.
2. **Остановка фоновых процессов:** корректно закрывает `Claude`, `Claude Helper`, `chrome-native-host`, `claude` (`claude.exe` на Windows), чтобы файлы не были заблокированы.
3. **Полная чистка следов (актуально на 2026 год):**
   - **Claude Desktop (macOS и Windows, включая MSIX):** `Cookies*`, `IndexedDB`, `Local Storage`, `Session Storage`, `Cache/Cache_Data`, `Service Worker`, `Partitions/*`, `Network/*`, `ant-device-registry.json`, `ccd-ids.json`, `dispatch-footprint-census.json`, `Local State`, `Preferences`, а на macOS также `HTTPStorages`, `Caches`, `Preferences/*.plist`, `Saved Application State`, `WebKit`.
   - **Claude Code CLI:** удаляет OAuth-токены из связки ключей macOS Keychain (`Claude Code-credentials*`) или Windows Credential Manager (`cmdkey`), очищает поля `userID`, `oauthAccount`, `cachedStatsigGates` в `~/.claude.json` (не трогая список проектов), а также удаляет `~/.claude/.credentials.json`, `statsig*`, `debug/`, `telemetry/`.
4. **Проверка сети и часового пояса:** показывает текущий внешний IP и страну (подсказывает, если выключен VPN в Happ), а также проверяет системный часовой пояс и даёт понятную рекомендацию без изменения глобального реестра Windows.

## Сборка и запуск для разработчиков

Требования: Node.js 20+, `pnpm`, Rust (`cargo` / `rustc`).

```bash
pnpm install
pnpm build
cd src-tauri && cargo test
cd .. && pnpm tauri build
```

Готовые установщики собираются в `src-tauri/target/release/bundle/`:
- **macOS:** `.app` и `.dmg`
- **Windows:** `.exe` (NSIS) и `.msi`
