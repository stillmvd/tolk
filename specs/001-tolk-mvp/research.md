# Research: Tolk 001

Полный разбор — `.planning/RESEARCH.md` (вне git). Здесь — решения.

## Онлайн-перевод
- **Decision**: Google `translate.googleapis.com/translate_a/single?client=gtx&dt=t&dt=at&dt=bd`, запасной —
  `clients5.google.com/translate_a/t?client=dict-chrome-ex`; кэш LRU в ОЗУ (200 записей), ≤ 2 запроса/с, 429 →
  следующий в цепочке.
- **Rationale**: единственные рабочие бесплатные эндпоинты без ключа на 2026-10-19; `dt=bd` даёт варианты слова.
- **Alternatives**: Яндекс (403 без сессии), Bing/Edge (токен мёртв), LibreTranslate (ключ), Lingva (мусор).

## Офлайн
- **Decision**: sidecar на ct2rs 0.10 (`ruy` + `sentencepiece`), модели Opus-MT в CTranslate2 (`gaudi/*`), int8 при
  загрузке, JSON по stdin/stdout.
- **Rationale**: тот же движок, что у ViTranslate (CTranslate2), без Python; изоляция от зависания при выходе.
- **Alternatives**: candle (нет ru-en, f32), ort (rc, свой бим-поиск), встроенный Python (сотни МБ).

## Захват текста
- **Decision**: буфер хука → UIA `TextPattern.GetSelection` → Ctrl+C со снимком всех форматов буфера.
- **Rationale**: так же у ViTranslate; буфер хука мгновенный и не трогает буфер обмена.
- **Alternatives**: только Ctrl+C (портит буфер, медленно), только UIA (пусто в части программ).

## Замена и отмена
- **Decision**: SendInput Unicode (короткое), Ctrl+V (длинное/многострочное); отмена — Backspace×N + исходник.
- **Rationale**: системный Ctrl+Z программ откатывает по-разному (часто половину) — свой откат предсказуем.

## Хоткеи
- **Decision**: в LL-хуке; Alt+Q съедается, затем `VK 0xE8` до отпускания Alt; в исключённых — пропуск; нечего
  переводить — повтор Alt+Q синтетикой с меткой.
- **Alternatives**: `RegisterHotKey` — не даёт пропускать по контексту, а хук нужен всё равно.

## Подсказка
- **Decision**: нативное GDI+ окно (основа — `Ghbdtn/src-tauri/src/overlay.rs`), кликабельное, без активации.
- **Rationale**: мгновенный показ и ≤ 40 МБ в покое; WebView создаётся ~300 мс и держит память.

## OCR
- **Decision**: `windows` `Media_Ocr`, BitBlt области, два движка (en, ru) — лучший по доле букв; результат в
  окно-переводчик.
- **Rationale**: бесплатно, локально, без моделей; окно-переводчик уже умеет перевод/копирование/правку.
