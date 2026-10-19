# Tauri-команды и события

| Команда | Вход | Выход |
|---|---|---|
| `get_config` | — | `Config` |
| `set_config` | `Config` | `Result<(), String>` (хоткей не распознан → ошибка) |
| `translate_text` | `{ text, dir?: "ru-en" \| "en-ru" }` | `{ text, variants, dir, engine }` / ошибка |
| `copy_text` | `{ text }` | — |
| `model_status` | — | `{ state: "none" \| "downloading" \| "ready", progress? }` |
| `model_download` / `model_delete` | — | — |
| `ocr_languages` | — | `string[]` |

| Событие (backend → окно) | Данные |
|---|---|
| `translator:fill` | `{ text }` — OCR или «нечего переводить» открыл окно с текстом |
| `model:progress` | `{ done, total }` |
| `config:changed` | `Config` |
