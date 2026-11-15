# Ukrainian translations of the weather plugin.

node-http-weather-title = Отримати погоду
node-http-weather-description = Отримує температуру для міста
node-http-weather-help =
    За сигналом на Trigger робить HTTP-запит за адресою з властивості url: { "{" }city{ "}" } замінюється на City (без City — Kyiv). Temp — фізична величина температури в канонічних градусах Цельсія. Сервіс має відповісти текстом «температура,опис», у градусах Цельсія, наприклад `22.5,Sunny`.

    • Успіх — Temp, Description і сигнал на Success.
    • Помилка мережі, статус 4xx/5xx або незрозуміла відповідь — сигнал на Failure.

    Новий сигнал скасовує попередній незавершений запит. Плагін може звертатися лише до доменів, дозволених у його маніфесті.
node-http-weather-pin-Trigger = Trigger
node-http-weather-pin-City = Місто
node-http-weather-pin-Success = Success
node-http-weather-pin-Failure = Failure
node-http-weather-pin-Temp = Температура
node-http-weather-pin-Description = Опис
node-http-weather-prop-url-title = URL
node-http-weather-prop-url-description = Адреса сервісу погоди
