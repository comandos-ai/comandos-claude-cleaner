# Правила проекта `comandos-claude-cleaner` (`AGENTS.md`)

- **Назначение:** лёгкая десктоп-утилита (Tauri v2 + Rust + Vite/TypeScript) для резидентов Клуба COMANDOS: бэкап, остановка процессов, полная чистка следов Claude Desktop + Claude Code CLI и проверка VPN/TZ без локального прокси поверх Happ.
- **Репозиторий и релизы:** [`https://github.com/comandos-ai/comandos-claude-cleaner`](https://github.com/comandos-ai/comandos-claude-cleaner) (ветка `main`, актуальный публичный релиз [`v0.1.0`](https://github.com/comandos-ai/comandos-claude-cleaner/releases/tag/v0.1.0)).
- **Прямые ссылки на установщики `v0.1.0` (для Академии, без токенов):**
  - macOS Universal (`.dmg`): `https://github.com/comandos-ai/comandos-claude-cleaner/releases/download/v0.1.0/COMANDOS.Claude.Cleaner_0.1.0_universal.dmg`
  - Windows x64 NSIS (`-setup.exe`): `https://github.com/comandos-ai/comandos-claude-cleaner/releases/download/v0.1.0/COMANDOS.Claude.Cleaner_0.1.0_x64-setup.exe`
  - Windows x64 MSI (`.msi`): `https://github.com/comandos-ai/comandos-claude-cleaner/releases/download/v0.1.0/COMANDOS.Claude.Cleaner_0.1.0_x64_en-US.msi`
- **Стек и пакетный менеджер:** Rust (`src-tauri/`) + TypeScript/Vite (`src/`), пакетный менеджер строго `pnpm`.
- **Точка входа:** `src-tauri/src/lib.rs` (команды `scan_status`, `run_clean`, `list_backups`, `restore_backup`, `check_environment`), фронтенд `src/main.ts` и `src/style.css`.
- **Безопасность данных пользователя:** никогда не удалять `claude_desktop_config.json`, `~/.claude/projects`, `~/.claude/MEMORY.md`, `~/.claude/CLAUDE.md`, `~/.claude/plans`, `~/.claude/settings.json`; всегда делать бэкап до удаления.
- **Качество UI и сборки:** интерфейс проверен по конвейеру `design-taste-frontend` + `ui-ux-pro-max` + `impeccable` (`impeccable detect` — 0 замечаний). Сборка установщиков (`.dmg` universal для macOS и `.exe`/`.msi` для Windows) настроена в `.github/workflows/build.yml` с выгрузкой артефактов через `actions/upload-artifact@v4` и релизов через `tauri-apps/tauri-action@v0`.
- **Документация:** каноническая карточка в Obsidian — `COMANDOS/Claude Antiban и чистка хвостов (разбор Застава v2).md`.
- **Последнее подтверждённое изменение:** `2026-11-14` — опубликован репозиторий `comandos-ai/comandos-claude-cleaner`, собран и проверен релиз `v0.1.0` (`run 43573902629`, `.dmg`, `-setup.exe`, `.msi`).
