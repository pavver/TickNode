# Ukrainian translations of the Home Assistant plugin.

category-home-assistant = Home Assistant

# ha.state
type-ha-state-title = Стан HA
type-ha-state-description = Стан сутності Home Assistant
type-ha-state-field-entity_id = Сутність
type-ha-state-field-state = Стан
type-ha-state-field-attributes = Атрибути
type-ha-state-field-last_changed = Остання зміна
type-ha-state-field-last_updated = Останнє оновлення

# Settings
setting-home-assistant-url-title = Адреса
setting-home-assistant-url-description = WebSocket-адреса Home Assistant: ws://<хост>:8123/api/websocket (wss:// з TLS). Додаток підставляє адресу Supervisor автоматично
setting-home-assistant-token-title = Токен доступу
setting-home-assistant-token-description = Довгоживучий токен доступу (Профіль > Безпека в Home Assistant) або токен supervisor у додатку

# Errors
home-assistant-error-no-token = Токен доступу не задано
home-assistant-error-bad-url = Адреса має починатися з ws:// або wss://, а не "{ $url }"


# ha/trigger_time
node-ha-trigger-time-title = Час
node-ha-trigger-time-description = Спрацьовує у визначений час або за часовою міткою сутності
node-ha-trigger-time-help =
    Поле «О» приймає HH:MM[:SS], сутність input_datetime або сенсор із часовою міткою. Home Assistant використовує власний часовий пояс. Фіксований час спричиняє спрацювання щодня.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-time-pin-Triggered = Спрацювання
node-ha-trigger-time-pin-Info = Інформація
node-ha-trigger-time-pin-Description = Опис
node-ha-trigger-time-prop-at-title = О котрій
node-ha-trigger-time-prop-at-description = HH:MM[:SS], сутність input_datetime або сенсор із часовою міткою

# ha/trigger_time_pattern
node-ha-trigger-time-pattern-title = Шаблон часу
node-ha-trigger-time-pattern-description = Спрацьовує за повторюваним часовим шаблоном
node-ha-trigger-time-pattern-help =
    Задайте години (0–23), хвилини та секунди (0–59). /5 означає кратні п’яти значення, * — будь-яке значення. Непотрібні поля залиште порожніми; потрібно заповнити принаймні одне. Розклад і типові значення пропущених полів визначає Home Assistant.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-time-pattern-pin-Triggered = Спрацювання
node-ha-trigger-time-pattern-pin-Info = Інформація
node-ha-trigger-time-pattern-pin-Description = Опис
node-ha-trigger-time-pattern-prop-hours-title = Години
node-ha-trigger-time-pattern-prop-hours-description = 0–23, * або крок, наприклад /2
node-ha-trigger-time-pattern-prop-minutes-title = Хвилини
node-ha-trigger-time-pattern-prop-minutes-description = 0–59, * або крок, наприклад /5
node-ha-trigger-time-pattern-prop-seconds-title = Секунди
node-ha-trigger-time-pattern-prop-seconds-description = 0–59, * або крок, наприклад /10

# ha/trigger_sun
node-ha-trigger-sun-title = Сонце
node-ha-trigger-sun-description = Спрацьовує на сході або заході сонця з необов’язковим зміщенням
node-ha-trigger-sun-help =
    Оберіть схід або захід сонця. Зміщення приймає тривалість, наприклад 30m або 00:30:00; від’ємне зміщення, наприклад -00:30:00, означає спрацювання до події. Home Assistant використовує налаштоване місцезнаходження.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-sun-pin-Triggered = Спрацювання
node-ha-trigger-sun-pin-Info = Інформація
node-ha-trigger-sun-pin-Description = Опис
node-ha-trigger-sun-prop-event-title = Подія
node-ha-trigger-sun-prop-event-description = Подія сонця, на яку очікувати
node-ha-trigger-sun-prop-event-option-sunrise = Схід сонця
node-ha-trigger-sun-prop-event-option-sunset = Захід сонця
node-ha-trigger-sun-prop-offset-title = Зміщення
node-ha-trigger-sun-prop-offset-description = Необов’язкова тривалість зі знаком: 30m, -00:30:00 або 0

# ha/trigger_zone
node-ha-trigger-zone-title = Зона
node-ha-trigger-zone-description = Спрацьовує, коли особа або пристрій входить у зону чи виходить із неї
node-ha-trigger-zone-help =
    Оберіть особу або відстежувач пристрою, зону та вхід або вихід. Home Assistant визначає перетин межі зони. Виходи «Сутність», «До» та «Після» описують відстежувану сутність; відсутній стан очищає відповідний вихід.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-zone-pin-Triggered = Спрацювання
node-ha-trigger-zone-pin-Info = Інформація
node-ha-trigger-zone-pin-Entity = Сутність
node-ha-trigger-zone-pin-From = До
node-ha-trigger-zone-pin-To = Після
node-ha-trigger-zone-prop-entity_id-title = Сутність
node-ha-trigger-zone-prop-entity_id-description = Оберіть сутність або введіть її ідентифікатор
node-ha-trigger-zone-prop-zone-title = Зона
node-ha-trigger-zone-prop-zone-description = Зона для входу або виходу
node-ha-trigger-zone-prop-event-title = Подія
node-ha-trigger-zone-prop-event-description = Перетин межі зони, на який очікувати
node-ha-trigger-zone-prop-event-option-enter = Вхід
node-ha-trigger-zone-prop-event-option-leave = Вихід

# ha/trigger_event
node-ha-trigger-event-title = Подія
node-ha-trigger-event-description = Спрацьовує за відповідної події шини Home Assistant
node-ha-trigger-event-help =
    Тип події — назва події на шині. Необов’язкові дані події — JSON-об’єкт, поля якого фільтрують події. Неправильний JSON записується в журнал після редагування та блокує підписку. Виходи «Дані події» та «Тип події» описують отриману подію.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-event-pin-Triggered = Спрацювання
node-ha-trigger-event-pin-Info = Інформація
node-ha-trigger-event-pin-event-data = Дані події
node-ha-trigger-event-pin-event-type = Тип події
node-ha-trigger-event-prop-event_type-title = Тип події
node-ha-trigger-event-prop-event_type-description = Назва події на шині Home Assistant
node-ha-trigger-event-prop-event_data-title = Дані події
node-ha-trigger-event-prop-event_data-description = Необов’язковий JSON-об’єкт для фільтрації полів події

# ha/trigger_homeassistant
node-ha-trigger-homeassistant-title = Запуск/зупинка Home Assistant
node-ha-trigger-homeassistant-description = Спрацьовує під час запуску або завершення роботи Home Assistant
node-ha-trigger-homeassistant-help =
    Оберіть запуск або завершення роботи. На момент події WebSocket має бути підключений; події запуску до встановлення з’єднання не відтворюються. Сигнал завершення роботи може надійти незадовго до розриву з’єднання.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-homeassistant-pin-Triggered = Спрацювання
node-ha-trigger-homeassistant-pin-Info = Інформація
node-ha-trigger-homeassistant-pin-Description = Опис
node-ha-trigger-homeassistant-prop-event-title = Подія
node-ha-trigger-homeassistant-prop-event-description = Подія життєвого циклу Home Assistant
node-ha-trigger-homeassistant-prop-event-option-start = Запуск
node-ha-trigger-homeassistant-prop-event-option-shutdown = Завершення роботи

# ha/trigger_mqtt
node-ha-trigger-mqtt-title = MQTT
node-ha-trigger-mqtt-description = Спрацьовує, коли повідомлення MQTT відповідає темі
node-ha-trigger-mqtt-help =
    Тема підтримує символи підстановки MQTT. Необов’язкове навантаження фільтрує точний текст повідомлення. Оберіть QoS 0, 1 або 2. Потрібна інтеграція MQTT у Home Assistant. Виходи «Тема» та «Навантаження» містять отримане повідомлення.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-mqtt-pin-Triggered = Спрацювання
node-ha-trigger-mqtt-pin-Info = Інформація
node-ha-trigger-mqtt-pin-Topic = Тема
node-ha-trigger-mqtt-pin-Payload = Навантаження
node-ha-trigger-mqtt-prop-topic-title = Тема
node-ha-trigger-mqtt-prop-topic-description = Тема MQTT із необов’язковими символами підстановки
node-ha-trigger-mqtt-prop-payload-title = Навантаження
node-ha-trigger-mqtt-prop-payload-description = Необов’язковий фільтр точного тексту навантаження
node-ha-trigger-mqtt-prop-qos-title = QoS
node-ha-trigger-mqtt-prop-qos-description = Якість обслуговування MQTT
node-ha-trigger-mqtt-prop-qos-option-0 = 0
node-ha-trigger-mqtt-prop-qos-option-1 = 1
node-ha-trigger-mqtt-prop-qos-option-2 = 2

# ha/trigger_webhook
node-ha-trigger-webhook-title = Вебхук
node-ha-trigger-webhook-description = Спрацьовує, коли вебхук Home Assistant отримує запит
node-ha-trigger-webhook-help =
    Задайте унікальний ідентифікатор вебхука. Надсилайте POST або PUT на /api/webhook/<id> у Home Assistant. «Лише локально» типово ввімкнено. «Дані» містять тіло JSON або дані форми, «Запит» — параметри URL. Якщо тіло JSON не є об’єктом, його обгорнуто в поле value. Ідентифікатор вебхука слід зберігати як секрет.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-webhook-pin-Triggered = Спрацювання
node-ha-trigger-webhook-pin-Info = Інформація
node-ha-trigger-webhook-pin-Data = Дані
node-ha-trigger-webhook-pin-Query = Запит
node-ha-trigger-webhook-prop-webhook_id-title = Ідентифікатор вебхука
node-ha-trigger-webhook-prop-webhook_id-description = Унікальний ідентифікатор для /api/webhook/<id>
node-ha-trigger-webhook-prop-local_only-title = Лише локально
node-ha-trigger-webhook-prop-local_only-description = Приймати запити лише з локальної мережі
node-ha-trigger-webhook-prop-local_only-option-yes = Так
node-ha-trigger-webhook-prop-local_only-option-no = Ні

# ha/trigger_calendar
node-ha-trigger-calendar-title = Календар
node-ha-trigger-calendar-description = Спрацьовує на початку або наприкінці події календаря
node-ha-trigger-calendar-help =
    Оберіть сутність календаря та початок або кінець події. Необов’язкове зміщення приймає тривалість зі знаком; -15m означає за п’ятнадцять хвилин до події. Вихід «Подія» містить змінну calendar_event із назвою та часом події.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-calendar-pin-Triggered = Спрацювання
node-ha-trigger-calendar-pin-Info = Інформація
node-ha-trigger-calendar-pin-Event = Подія
node-ha-trigger-calendar-prop-entity_id-title = Сутність
node-ha-trigger-calendar-prop-entity_id-description = Оберіть сутність або введіть її ідентифікатор
node-ha-trigger-calendar-prop-event-title = Подія
node-ha-trigger-calendar-prop-event-description = Початок або кінець події календаря
node-ha-trigger-calendar-prop-event-option-start = Початок
node-ha-trigger-calendar-prop-event-option-end = Кінець
node-ha-trigger-calendar-prop-offset-title = Зміщення
node-ha-trigger-calendar-prop-offset-description = Необов’язкова тривалість зі знаком: 30m, -00:30:00 або 0

# ha/trigger_tag
node-ha-trigger-tag-title = Мітка
node-ha-trigger-tag-description = Спрацьовує під час сканування мітки Home Assistant
node-ha-trigger-tag-help =
    Задайте ідентифікатор мітки з Home Assistant. Виходи «Ідентифікатор мітки» та «Ідентифікатор пристрою» визначають мітку та пристрій сканування. Фільтр за пристроєм не застосовується.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-tag-pin-Triggered = Спрацювання
node-ha-trigger-tag-pin-Info = Інформація
node-ha-trigger-tag-pin-tag-id = Ідентифікатор мітки
node-ha-trigger-tag-pin-device-id = Ідентифікатор пристрою
node-ha-trigger-tag-prop-tag_id-title = Ідентифікатор мітки
node-ha-trigger-tag-prop-tag_id-description = Ідентифікатор мітки Home Assistant

# ha/trigger_conversation
node-ha-trigger-conversation-title = Речення
node-ha-trigger-conversation-description = Спрацьовує, коли Assist розпізнає речення
node-ha-trigger-conversation-help =
    Введіть один або кілька шаблонів речень, розділених крапками з комою або новими рядками. Синтаксис Home Assistant підтримує необов’язкові слова та слоти. «Речення» містить розпізнаний текст, «Слоти» — значення слотів. Потрібна розмовна інтеграція.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-conversation-pin-Triggered = Спрацювання
node-ha-trigger-conversation-pin-Info = Інформація
node-ha-trigger-conversation-pin-Sentence = Речення
node-ha-trigger-conversation-pin-Slots = Слоти
node-ha-trigger-conversation-prop-command-title = Речення
node-ha-trigger-conversation-prop-command-description = Шаблони речень, розділені крапками з комою або новими рядками

# ha/trigger_template
node-ha-trigger-template-title = Шаблон
node-ha-trigger-template-description = Спрацьовує, коли шаблон Home Assistant стає істинним
node-ha-trigger-template-help =
    Шаблон значення — шаблон Jinja в Home Assistant. Спрацьовує під час переходу результату з хибного на істинний. Необов’язкове «Протягом» вимагає утримання істинного результату заданий час. Обчислення шаблону та відстеження сутностей виконує Home Assistant.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-template-pin-Triggered = Спрацювання
node-ha-trigger-template-pin-Info = Інформація
node-ha-trigger-template-pin-Description = Опис
node-ha-trigger-template-prop-value_template-title = Шаблон значення
node-ha-trigger-template-prop-value_template-description = Шаблон Jinja в Home Assistant, що стає істинним
node-ha-trigger-template-prop-for-title = Протягом
node-ha-trigger-template-prop-for-description = Необов’язкова тривалість: 90, 1h 30m або 00:05:00

# ha/action
node-ha-action-title = Дія
node-ha-action-description = Виконує дію Home Assistant із полями її форми
node-ha-action-help =
    Оберіть сутність і дію зі списків: список дій фільтрується за обраною сутністю та сортується за частотою використання. Вхід Entity замінює вибрану сутність, якщо його задано. Вибір дії за наявності каталогу зберігає її поля як вхідні піни (розширені — наприкінці); вони відновлюються після збереження та перепідключення. Обов'язкові поля потрібно заповнити; незадані необов'язкові поля не надсилаються.

    Поля з переліком варіантів приймають текст без випадного списку. Числові поля з відомими одиницями мають входи фізичних величин; канонічне значення перед надсиланням перетворюється в збережену одиницю HA. Колір надсилається як RGB, тривалість — як години, хвилини та секунди, час — як HH:MM:SS. Done підтверджує успіх; Failed і Error повідомляють про відхилений запит, тайм-аут або відсутність зв'язку. Response оновлюється лише тоді, коли HA повертає дані відповіді. Повторно оберіть дію, щоб оновити схему полів після її зміни в HA.
node-ha-action-prop-target-title = Сутність
node-ha-action-prop-target-description = Цільова сутність; визначає доступні дії. Вхід Entity замінює цей вибір
node-ha-action-prop-action-title = Дія
node-ha-action-prop-action-description = Дія Home Assistant для обраної сутності
node-ha-action-pin-In = Вхід
node-ha-action-pin-Entity = Сутність
node-ha-action-pin-Done = Готово
node-ha-action-pin-Failed = Невдача
node-ha-action-pin-Error = Помилка
node-ha-action-pin-Response = Відповідь

# Automation conditions
node-ha-condition-time-title = Умова: Час
node-ha-condition-time-description = Перевіряє місцевий час і день тижня Home Assistant
node-ha-condition-time-help = «Після» включає межу, «До» — ні. Однакові «Після» та «До» означають цілу добу. Інтервал може перетинати північ; день тижня завжди стосується поточного місцевого дня. Дні: mon,tue,wed,thu,fri,sat,sun. Порожні поля не обмежують перевірку. Часова зона надходить із get_config, живий зсув UTC обчислює HA. WASI не використовує часову зону хоста. Під час зміни зони або літнього часу зсув може затриматися до хвилини плюс доставка події. До отримання даних результат — «Ні». Вхід перевіряє умову; логічний результат надходить на «Результат» і спрацьовує рівно один вихід: «Так» або «Ні». Локальні умови не роблять запитів під час виконання. Невідомий, недоступний або ще не завантажений стан чи розрив зв’язку дає «Ні» з одноразовим попередженням до зміни ситуації.
node-ha-condition-time-pin-In = Вхід
node-ha-condition-time-pin-Yes = Так
node-ha-condition-time-pin-No = Ні
node-ha-condition-time-pin-Result = Результат
node-ha-condition-time-prop-after-title = Після
node-ha-condition-time-prop-after-description = Необов’язковий місцевий час HH:MM[:SS], включно з межею
node-ha-condition-time-prop-before-title = До
node-ha-condition-time-prop-before-description = Необов’язковий місцевий час HH:MM[:SS], без включення межі
node-ha-condition-time-prop-weekday-title = День тижня
node-ha-condition-time-prop-weekday-description = Необов’язкові дні через кому: mon,tue,wed,thu,fri,sat,sun

node-ha-condition-sun-title = Умова: Сонце
node-ha-condition-sun-description = Перевіряє межі сходу й заходу сонця зі зсувами
node-ha-condition-sun-help = Використовує next_rising і next_setting із кешу sun.sun. Місцевий час наступної події переноситься на сьогоднішню дату HA, потім додається зсув. «До» та «Після» включають межу й поєднуються через І: після заходу та до сходу не утворюють нічний інтервал. Після події це наближення: завтрашній сонячний час може відрізнятися на кілька хвилин. Полярні події далі ніж за дві доби дають «Ні». Зсуви: -30m або 00:30:00. Зсув UTC оновлюється так само, як в умові «Час», із можливою затримкою до хвилини. Вхід перевіряє умову; логічний результат надходить на «Результат» і спрацьовує рівно один вихід: «Так» або «Ні». Локальні умови не роблять запитів під час виконання. Невідомий, недоступний або ще не завантажений стан чи розрив зв’язку дає «Ні» з одноразовим попередженням до зміни ситуації.
node-ha-condition-sun-pin-In = Вхід
node-ha-condition-sun-pin-Yes = Так
node-ha-condition-sun-pin-No = Ні
node-ha-condition-sun-pin-Result = Результат
node-ha-condition-sun-prop-after-title = Після
node-ha-condition-sun-prop-after-description = Необов’язкова межа за сонячною подією
node-ha-condition-sun-prop-after-option-none = Немає
node-ha-condition-sun-prop-after-option-sunrise = Схід сонця
node-ha-condition-sun-prop-after-option-sunset = Захід сонця
node-ha-condition-sun-prop-after_offset-title = Зсув після
node-ha-condition-sun-prop-after_offset-description = Необов’язкова тривалість зі знаком, додана до події «Після»
node-ha-condition-sun-prop-before-title = До
node-ha-condition-sun-prop-before-description = Необов’язкова межа за сонячною подією
node-ha-condition-sun-prop-before-option-none = Немає
node-ha-condition-sun-prop-before-option-sunrise = Схід сонця
node-ha-condition-sun-prop-before-option-sunset = Захід сонця
node-ha-condition-sun-prop-before_offset-title = Зсув до
node-ha-condition-sun-prop-before_offset-description = Необов’язкова тривалість зі знаком, додана до події «До»

node-ha-condition-zone-title = Умова: Зона
node-ha-condition-zone-description = Перевіряє перебування людини або трекера в зоні
node-ha-condition-zone-help = Порівнює кешований стан person/device_tracker з friendly_name зони; zone.home відповідає home. Можна вибрати людину або ввести ID device_tracker. Використовується назва зони, яку повідомляє сутність HA, без окремого розрахунку GPS та радіуса; для зон, що перекриваються, вирішальним є повідомлений стан. Вхід перевіряє умову; логічний результат надходить на «Результат» і спрацьовує рівно один вихід: «Так» або «Ні». Локальні умови не роблять запитів під час виконання. Невідомий, недоступний або ще не завантажений стан чи розрив зв’язку дає «Ні» з одноразовим попередженням до зміни ситуації.
node-ha-condition-zone-pin-In = Вхід
node-ha-condition-zone-pin-Yes = Так
node-ha-condition-zone-pin-No = Ні
node-ha-condition-zone-pin-Result = Результат
node-ha-condition-zone-prop-entity_id-title = Сутність
node-ha-condition-zone-prop-entity_id-description = ID сутності людини або device_tracker
node-ha-condition-zone-prop-zone-title = Зона
node-ha-condition-zone-prop-zone-description = Зона для перевірки; zone.home відповідає home

node-ha-condition-template-title = Умова: Шаблон
node-ha-condition-template-description = Асинхронно перевіряє умову Jinja в Home Assistant
node-ha-condition-template-help = На «Вхід» запускає підписку render_template у HA, бере перший результат і відписується. «Результат» і «Так»/«Ні» надходять за кілька тактів. Лише логічне true або текст true без урахування регістру означають істину, як в умовах HA. Помилка, розрив зв’язку або тайм-аут 30 секунд дають «Ні» та попередження англійською. Кожен вхід створює запит, до 64 одночасно на плагін. Зміна властивостей або видалення вузла скасовує його запити. Вхід перевіряє умову; логічний результат надходить на «Результат» і спрацьовує рівно один вихід: «Так» або «Ні». Локальні умови не роблять запитів під час виконання. Невідомий, недоступний або ще не завантажений стан чи розрив зв’язку дає «Ні» з одноразовим попередженням до зміни ситуації.
node-ha-condition-template-pin-In = Вхід
node-ha-condition-template-pin-Yes = Так
node-ha-condition-template-pin-No = Ні
node-ha-condition-template-pin-Result = Результат
node-ha-condition-template-prop-value_template-title = Шаблон значення
node-ha-condition-template-prop-value_template-description = Шаблон Jinja Home Assistant з логічним результатом

# Resource collections and actions
home-assistant-resource-entities-title = Сутності
home-assistant-resource-entities-action-ha-entity = Коли змінюється
home-assistant-resource-entities-action-ha-action = Дія

home-assistant-resource-zones-title = Зони
home-assistant-resource-zones-action-ha-trigger-zone = Тригер зони
home-assistant-resource-zones-action-ha-condition-zone = Умова зони

home-assistant-resource-persons-title = Люди
home-assistant-resource-persons-action-ha-condition-zone = Умова зони

node-ha-entity-title = Сутність
node-ha-entity-description = Спрацьовує при зміні сутності Home Assistant і надає її типізоване значення
node-ha-entity-help =
    Оберіть сутність. «Зміна» за замовчуванням спрацьовує лише при зміні тексту стану; «Будь-яке оновлення» враховує також зміни атрибутів. Кожна подія оновлює «Значення» та «Попереднє» (значення до зміни), у порядку надходження, не більше однієї події за такт.
    Тип значення визначається автоматично при виборі відомої сутності. Його можна змінити на фізичну величину, число, увімк/вимк, текст, дату й час (RFC 3339) або тривалість. Одиниці сенсора тривалості перетворюються на проміжок часу. Невідомий, недоступний або неперетворюваний стан очищає відповідний вихід. Фізичні величини мають канонічні значення (наприклад, кВт перетворюються у Вт); кожен стан використовує власну одиницю. Відсутня або несумісна одиниця очищає відповідний вихід і спричиняє одноразове попередження до відновлення. Одиниця береться з unit_of_measurement; «Атрибути» й «Стан сутності» надають повний поточний стан.
    Початковий знімок і знімок після відновлення зв’язку оновлюють поточні дані без імпульсу «Зміна» та без зміни «Попереднє». Видалення сутності очищає поточні дані. Без зв’язку останні значення зберігаються, пропущені події не відтворюються. Невдалу підписку повторюють через 30 секунд.
node-ha-entity-pin-Changed = Зміна
node-ha-entity-pin-Value = Значення
node-ha-entity-pin-Previous = Попереднє
node-ha-entity-pin-Unit = Одиниця
node-ha-entity-pin-Attributes = Атрибути
node-ha-entity-pin-entity-state = Стан сутності
node-ha-entity-prop-entity_id-title = Сутність
node-ha-entity-prop-entity_id-description = Сутність Home Assistant, за якою стежити
node-ha-entity-prop-value_type-title = Тип значення
node-ha-entity-prop-value_type-description = Визначається автоматично при виборі відомої сутності; можна змінити вручну
node-ha-entity-prop-value_type-option-number = Число
node-ha-entity-prop-value_type-option-bool = Увімк/Вимк
node-ha-entity-prop-value_type-option-text = Текст
node-ha-entity-prop-value_type-option-datetime = Дата й час
node-ha-entity-prop-value_type-option-duration = Тривалість
node-ha-entity-prop-fire_on-title = Спрацьовувати на
node-ha-entity-prop-fire_on-description = Коли надсилати імпульс «Зміна»; дані оновлюються при кожній події
node-ha-entity-prop-fire_on-option-state = Зміна стану
node-ha-entity-prop-fire_on-option-any = Будь-яке оновлення

node-ha-entity-prop-value_type-option-quantity-power = Потужність
node-ha-entity-prop-value_type-option-quantity-energy = Енергія
node-ha-entity-prop-value_type-option-quantity-voltage = Напруга
node-ha-entity-prop-value_type-option-quantity-current = Струм
node-ha-entity-prop-value_type-option-quantity-apparent-power = Повна потужність
node-ha-entity-prop-value_type-option-quantity-frequency = Частота
node-ha-entity-prop-value_type-option-quantity-temperature = Температура
node-ha-entity-prop-value_type-option-quantity-percentage = Відсоток
node-ha-entity-prop-value_type-option-quantity-pressure = Тиск
node-ha-entity-prop-value_type-option-quantity-speed = Швидкість
node-ha-entity-prop-value_type-option-quantity-length = Довжина
node-ha-entity-prop-value_type-option-quantity-volume = Об’єм
node-ha-entity-prop-value_type-option-quantity-mass = Маса
node-ha-entity-prop-value_type-option-quantity-illuminance = Освітленість
node-ha-entity-prop-value_type-option-quantity-irradiance = Опромінення
node-ha-entity-prop-value_type-option-quantity-concentration = Концентрація
node-ha-entity-prop-value_type-option-quantity-parts = Частки
node-ha-entity-prop-value_type-option-quantity-reactive-power = Реактивна потужність
node-ha-entity-prop-value_type-option-quantity-reactive-energy = Реактивна енергія
node-ha-entity-prop-value_type-option-quantity-area = Площа
node-ha-entity-prop-value_type-option-quantity-data-rate = Швидкість передавання даних
node-ha-entity-prop-value_type-option-quantity-data-size = Обсяг даних
node-ha-entity-prop-value_type-option-quantity-precipitation-intensity = Інтенсивність опадів
node-ha-entity-prop-value_type-option-quantity-volume-flow-rate = Об’ємна витрата
node-ha-entity-prop-value_type-option-quantity-conductivity = Електропровідність
node-ha-entity-prop-value_type-option-quantity-signal-strength = Рівень сигналу
node-ha-entity-prop-value_type-option-quantity-sound-pressure = Звуковий тиск
node-ha-entity-prop-value_type-option-quantity-angle = Кут
node-ha-entity-prop-value_type-option-quantity-blood-glucose = Глюкоза крові

# Additional entity value type options.
node-ha-entity-prop-value_type-option-quantity-beaufort = Сила вітру (Бофорт)
node-ha-entity-prop-value_type-option-quantity-energy-consumption = Енергія на відстань
node-ha-entity-prop-value_type-option-quantity-aqi = Індекс якості повітря
node-ha-entity-prop-value_type-option-quantity-ph = pH
node-ha-entity-prop-value_type-option-quantity-currency-uah = Українська гривня
node-ha-entity-prop-value_type-option-quantity-currency-usd = Долар США
node-ha-entity-prop-value_type-option-quantity-currency-eur = Євро
node-ha-entity-prop-value_type-option-quantity-currency-gbp = Фунт стерлінгів
node-ha-entity-prop-value_type-option-quantity-currency-pln = Польський злотий
node-ha-entity-prop-value_type-option-quantity-currency-chf = Швейцарський франк
node-ha-entity-prop-value_type-option-quantity-currency-czk = Чеська крона
node-ha-entity-prop-value_type-option-quantity-currency-huf = Угорський форинт
node-ha-entity-prop-value_type-option-quantity-currency-ron = Румунський лей
node-ha-entity-prop-value_type-option-quantity-currency-bgn = Болгарський лев
node-ha-entity-prop-value_type-option-quantity-currency-sek = Шведська крона
node-ha-entity-prop-value_type-option-quantity-currency-nok = Норвезька крона
node-ha-entity-prop-value_type-option-quantity-currency-dkk = Данська крона
node-ha-entity-prop-value_type-option-quantity-currency-isk = Ісландська крона
node-ha-entity-prop-value_type-option-quantity-currency-try = Турецька ліра
node-ha-entity-prop-value_type-option-quantity-currency-gel = Грузинський ларі
node-ha-entity-prop-value_type-option-quantity-currency-mdl = Молдовський лей
node-ha-entity-prop-value_type-option-quantity-currency-kzt = Казахстанський тенге
node-ha-entity-prop-value_type-option-quantity-currency-ils = Ізраїльський новий шекель
node-ha-entity-prop-value_type-option-quantity-currency-jpy = Японська єна
node-ha-entity-prop-value_type-option-quantity-currency-cny = Китайський юань
node-ha-entity-prop-value_type-option-quantity-currency-krw = Південнокорейська вона
node-ha-entity-prop-value_type-option-quantity-currency-inr = Індійська рупія
node-ha-entity-prop-value_type-option-quantity-currency-cad = Канадський долар
node-ha-entity-prop-value_type-option-quantity-currency-aud = Австралійський долар
node-ha-entity-prop-value_type-option-quantity-currency-nzd = Новозеландський долар
node-ha-entity-prop-value_type-option-quantity-currency-sgd = Сінгапурський долар
node-ha-entity-prop-value_type-option-quantity-currency-hkd = Гонконгський долар
node-ha-entity-prop-value_type-option-quantity-currency-brl = Бразильський реал
node-ha-entity-prop-value_type-option-quantity-currency-mxn = Мексиканський песо
node-ha-entity-prop-value_type-option-quantity-currency-zar = Південноафриканський ранд
node-ha-entity-prop-value_type-option-quantity-currency-aed = Дирхам ОАЕ
node-ha-entity-prop-value_type-option-quantity-color-temperature = Колірна температура
