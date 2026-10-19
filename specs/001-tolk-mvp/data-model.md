# Data Model: Tolk 001

## Config (`%APPDATA%\Tolk\config.json`)

| Поле | Тип | По умолчанию |
|---|---|---|
| `enabled` | bool | true |
| `hotkey` | string | `"Alt+Q"` |
| `area_hotkey` | string | `"Alt+Shift+Q"` |
| `window_hotkey` | string \| null | null |
| `mode` | `"online"` \| `"offline"` | `"online"` |
| `excluded` | string[] (имена exe) | KeePass, 1Password, Bitwarden, mstsc |
| `autostart` | bool | false |
| `history` | bool | false |
| `repeat_ms` | u32 | 1500 |

## Fragment (ОЗУ)

`text`, `source: Typed | Selection | Window | Ocr`, `hwnd`, `dir: RuEn | EnRu`, `words` (для Typed — сколько слов
взято), `hook_seq` (номер последнего события хука на момент захвата).

## Translation (ОЗУ, кэш)

`text`, `variants: Vec<String>` (≤ 5, только для одного слова), `engine: Gtx | Clients5 | Offline`.

## UndoRecord (ОЗУ)

`hwnd`, `inserted: String`, `original: String`, `layout_before: HKL`, `hook_seq`. Живёт, пока `hook_seq` не изменился
(любое событие хука, клик, смена фокуса — снятие). Для расширения фрагмента хранится и предыдущий перевод.

## WordBuffer (ОЗУ)

Символы текущей фразы с границами слов; сброс: клик, смена окна, Enter, Tab, стрелки/Home/End, Ctrl-комбинации кроме
Ctrl+Backspace (удаляет слово из буфера), простой > 30 с, поле пароля, чужой инжект. Ограничение — 200 символов.

## История (только при `history: true`)

`history.jsonl`: `{t, from, to, src, dst}` — до 500 строк, кольцом.
