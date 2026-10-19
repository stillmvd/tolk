# Протокол sidecar `tolk-mt`

Запуск: `tolk-mt.exe <каталог моделей>`. Одна JSON-строка на запрос/ответ, UTF-8, `\n`.

Запрос: `{"id":1,"dir":"ru-en","text":"открыть замок","n":3}`
Ответ: `{"id":1,"text":"open the lock","variants":["open the lock","unlock","open the castle"]}`
Ошибка: `{"id":1,"error":"model not found: ru-en"}`

Готовность: после загрузки моделей sidecar пишет `{"ready":true}`. Закрытие stdin → выход; приложение при выходе
убивает процесс без ожидания.
