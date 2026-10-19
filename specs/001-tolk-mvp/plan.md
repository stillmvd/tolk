# Implementation Plan: Tolk — перевод ru↔en на месте

**Branch**: `main` | **Date**: 2026-10-19 | **Spec**: [spec.md](spec.md)

## Summary

Трей-утилита Windows 11. Low-level хук клавиатуры ведёт буфер слов и ловит Alt+Q / Alt+Shift+Q / Ctrl+Z (в окне
отмены). Рабочий поток берёт фрагмент (буфер слов → UIA-выделение → Ctrl+C),
переводит цепочкой «Google gtx → Google clients5 → офлайн-модель», заменяет текст Backspace×N + SendInput Unicode
(длинное — вставкой с восстановлением буфера), переключает раскладку, держит запись отмены и показывает нативную
подсказку с вариантами. Окно-переводчик и настройки — WebView по требованию; OCR — нативный оверлей выбора области +
Windows.Media.Ocr, результат — в окно-переводчик. Офлайн — sidecar `tolk-mt` (ct2rs + Opus-MT), JSON по stdin/stdout.

## Technical Context

**Language/Version**: Rust 1.96 (edition 2021), TypeScript 5.8, React 19
**Primary Dependencies**: tauri 2 (tray-icon), tauri-plugin-single-instance, tauri-plugin-autostart, windows 0.62
(KeyboardAndMouse, WindowsAndMessaging, DataExchange, Accessibility/UIA, GdiPlus, Media_Ocr, Graphics_Imaging), ureq 3
(HTTP, rustls), serde/serde_json, crossbeam-channel; sidecar — ct2rs 0.10 (`ruy`, `sentencepiece`); UI — kockers-ds
(`link:`), tauri-ship
**Storage**: `%APPDATA%\Tolk\config.json` (настройки), `%LOCALAPPDATA%\Tolk\models\` (офлайн-модель), история — только
при включении (`%APPDATA%\Tolk\history.jsonl`)
**Testing**: `cargo test` (чистая логика: направление, буфер слов, расширение, регистр/пунктуация, разбор ответов
Google, протокол sidecar), живой пример `cargo run --example live` (печатает в Блокнот — по подтверждению)
**Target Platform**: Windows 11 x64
**Project Type**: desktop (Tauri) + workspace-крейты
**Performance Goals**: хук ≤ 1 мс/нажатие; слово ≤ 300 мс при живой сети; офлайн-фраза ≤ 1,5 с
**Constraints**: в покое ≤ 40 МБ, без WebView; без прав администратора; текст не на диск
**Scale/Scope**: 1 пользователь, ru/en, ~10 программ проверки

## Constitution Check

| Принцип | Как соблюдается |
|---|---|
| I. Приватность | Буфер слов и запись отмены — в ОЗУ; сеть только из `translate()` по явному действию и при `mode != offline`; логи без текста; история — флаг, по умолчанию выкл. |
| II. Мгновенный хук | Колбэк: решение по атомарным флагам (`armed_undo`, `hint_open`, `excluded`, `password`), событие в канал. Всё прочее — рабочий поток. Переустановка хука как в Ghbdtn. |
| III. Не ломать ввод | Перед заменой — проверка «окно то же, нажатий не было» (счётчик событий хука); иначе подсказка. Буфер обмена — снимок всех форматов. Пароль/повышенные права/исключения — без замены. Alt+Q в исключённых — пропуск; «нечего переводить» — повтор комбинации синтетикой. |
| IV. Отменяемость | Запись отмены живёт до первого события хука/клика/смены фокуса; Ctrl+Z в это окно поглощается и откатывает. |
| V. Отклик | Таймауты: gtx 1,2 с → clients5 0,8 с → офлайн; индикатор в подсказке после 150 мс; ошибки — подсказкой. |
| VI. Соседство с Ghbdtn | Метка `dwExtraInfo` своя (`TOLK`), чужие инжекты (в т. ч. Ghbdtn) сбрасывают буфер; клавиши Ghbdtn не трогаем. |

Нарушений нет.

## Ключевые решения (подробно — [research.md](research.md))

1. **Хоткеи — в LL-хуке, не `RegisterHotKey`**: хук всё равно нужен (буфер слов, Ctrl+Z, Alt+цифры), а решение
   «съесть/пропустить» по флагам даёт пропуск в исключённых программах.
2. **Alt+Q съедается** → перед отпусканием Alt шлётся пустая клавиша `VK 0xE8`, чтобы не открылось меню окна.
3. **Захват**: набранное (буфер валиден) → UIA `GetSelection` непустое → Ctrl+C со снимком буфера. Выделение важнее
   буфера: если UIA сообщает непустое выделение — переводим его.
4. **Замена**: ≤ 400 символов без переносов — Backspace×N (для набранного) / набор поверх выделения через SendInput
   Unicode; иначе — вставка Ctrl+V из своей записи буфера + восстановление снимка.
5. **Отмена**: Backspace × длина вставленного + набор исходника; раскладка — исходная.
6. **Подсказка — нативная** (GDI+ слой из `Ghbdtn/src-tauri/src/overlay.rs`), кликабельная (без `WS_EX_TRANSPARENT`,
   с `WS_EX_NOACTIVATE`); позиция — UIA-рамка слова/каретки → `GetGUIThreadInfo` → курсор.
7. **Окна** — WebView создаются по требованию и уничтожаются при закрытии (в покое без WebView).
8. **Офлайн** — sidecar-процесс (зависание ct2rs при выходе на Windows → изоляция), модель скачивается по кнопке
   (`gaudi/opus-mt-{ru-en,en-ru}-ctranslate2`, SHA-256), `compute_type=int8`. Требует CMake для сборки — ставится
   перед этой фазой (`winget install Kitware.CMake`).
9. **OCR** — BitBlt области → `SoftwareBitmap` → `OcrEngine` (en и ru, лучший по доле букв алфавита) → текст в
   окно-переводчик, оно переводит само.

## Project Structure

### Documentation (this feature)

```text
specs/001-tolk-mvp/
├── plan.md  research.md  data-model.md  quickstart.md
├── contracts/ (commands.md — Tauri-команды и события; mt-protocol.md — sidecar)
└── tasks.md
```

### Source Code

```text
src-tauri/
├── Cargo.toml            # workspace: core, mt; приложение tolk
├── src/                  # Tauri: lib.rs (запуск, трей, окна), tray.rs, hint.rs (нативная подсказка),
│                         # area.rs (оверлей выбора области), commands.rs
├── core/                 # tolk-core: чистая логика + win/
│   ├── src/text.rs       # направление, токены, расширение фрагмента, регистр/пунктуация
│   ├── src/translate/    # mod.rs (цепочка, кэш), google.rs (запрос/разбор), offline.rs (клиент sidecar)
│   ├── src/config.rs
│   ├── src/win/          # hook.rs, keys.rs (SendInput), clipboard.rs, lang.rs, context.rs, uia.rs, ocr.rs
│   ├── src/engine.rs     # рабочий поток: событие → захват → перевод → замена/подсказка → запись отмены
│   └── examples/live.rs
└── mt/                   # tolk-mt: sidecar ct2rs
src/                      # React: translator/, settings/, main.tsx (маршрут по метке окна)
```

**Structure Decision**: как Ghbdtn — Tauri-приложение + workspace-крейт ядра; sidecar отдельным крейтом, чтобы
CMake-сборка CTranslate2 не входила в обычный `pnpm check` до фазы офлайна.

## Complexity Tracking

| Отступление | Зачем | Почему проще не подходит |
|---|---|---|
| Sidecar-процесс `tolk-mt` | Зависание ct2rs при выходе на Windows, тяжёлая CMake-сборка | В одном процессе выход приложения может зависнуть; сборка каждого `pnpm check` дольше на минуты |
