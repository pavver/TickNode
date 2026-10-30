# Ukrainian translations of the Home Assistant plugin.

category-home-assistant = Home Assistant

# ha/state_changed
node-ha-state-changed-title = HA: зміна стану
node-ha-state-changed-description = Спрацьовує, коли сутність Home Assistant змінюється
node-ha-state-changed-help =
    Джерело подій: надсилає сигнал на Changed щоразу, коли сутність із властивості entity_id змінює стан або атрибути в Home Assistant. New — стан після зміни, Old — стан до неї. Якщо сутності раніше не було (або її видалено), у відповідного стану порожнє поле state.

    Властивість mode визначає, що робити, коли за один такт приходить кілька змін: "every" (типово) — один сигнал на кожну зміну, по порядку, по одному за такт; "latest" — лише найновіша зміна такту.

    Потрібне підключення до Home Assistant у налаштуваннях плагіна. Поки зв'язку немає, зміни не видно, а після відновлення зміна, що сталася за цей час, приходить одним сигналом. Неіснуюча сутність перепідписується кожні 30 секунд.
node-ha-state-changed-prop-entity_id-title = Сутність
node-ha-state-changed-prop-entity_id-description = Сутність Home Assistant, за якою стежити. Оберіть зі списку або введіть ідентифікатор сутності
node-ha-state-changed-prop-mode-title = Режим
node-ha-state-changed-prop-mode-description = Що робити, коли за один такт приходить кілька змін
node-ha-state-changed-prop-mode-option-every = Кожна зміна
node-ha-state-changed-prop-mode-option-latest = Лише остання
node-ha-state-changed-pin-Changed = Зміна
node-ha-state-changed-pin-New = Новий
node-ha-state-changed-pin-Old = Старий

# ha/get_state
node-ha-get-state-title = HA: отримати стан
node-ha-get-state-description = Зчитує поточний стан сутності Home Assistant
node-ha-get-state-help =
    За сигналом на In питає Home Assistant про поточний стан сутності Entity ID. Відповідь приходить трохи пізніше (за кілька тактів): у разі успіху на State буде сутність, а сигнал піде на Done; у разі невдачі (немає зв'язку, немає такої сутності, тайм-аут) на Error буде причина, а сигнал піде на Failed. Після невдачі State — порожній стан (його поле state порожнє).

    Запит одразу завершується невдачею, якщо зв'язку з Home Assistant немає.
node-ha-get-state-pin-In = Вхід
node-ha-get-state-pin-entity-id = Сутність
node-ha-get-state-pin-Done = Готово
node-ha-get-state-pin-Failed = Невдача
node-ha-get-state-pin-State = Стан
node-ha-get-state-pin-Error = Помилка

# ha/call_service
node-ha-call-service-title = HA: викликати сервіс
node-ha-call-service-description = Викликає сервіс Home Assistant
node-ha-call-service-help =
    За сигналом на In викликає сервіс Home Assistant Domain.Service (наприклад, домен light і сервіс turn_on). Entity ID — необов'язкова цільова сутність; Data — необов'язковий об'єкт із даними сервісу (наприклад, з полем brightness).

    Коли Home Assistant підтвердить виклик, сигнал піде на Done; інакше (немає зв'язку, відхилено, тайм-аут) на Error буде причина, а сигнал піде на Failed. Відповідь приходить трохи пізніше (за кілька тактів); запит одразу завершується невдачею, якщо зв'язку немає.
node-ha-call-service-pin-In = Вхід
node-ha-call-service-pin-Domain = Домен
node-ha-call-service-pin-Service = Сервіс
node-ha-call-service-pin-entity-id = Сутність
node-ha-call-service-pin-Data = Дані
node-ha-call-service-pin-Done = Готово
node-ha-call-service-pin-Failed = Невдача
node-ha-call-service-pin-Error = Помилка

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


# ha/trigger_state
node-ha-trigger-state-title = Стан
node-ha-trigger-state-description = Спрацьовує, коли сутність відповідає переходу стану
node-ha-trigger-state-help =
    Оберіть сутність і, за потреби, атрибут та значення «Зі стану» й «У стан». Порожні поля не обмежують перехід, тому зміна лише атрибутів також може спричинити спрацювання. «Протягом» задає тривалість утримання стану: 90, 1h 30m або 00:05:00. Виходи «До» та «Після» містять ha.state; відсутній стан має порожнє поле state.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-state-pin-Triggered = Спрацювання
node-ha-trigger-state-pin-Info = Інформація
node-ha-trigger-state-pin-Entity = Сутність
node-ha-trigger-state-pin-From = До
node-ha-trigger-state-pin-To = Після
node-ha-trigger-state-prop-entity_id-title = Сутність
node-ha-trigger-state-prop-entity_id-description = Оберіть сутність або введіть її ідентифікатор
node-ha-trigger-state-prop-attribute-title = Атрибут
node-ha-trigger-state-prop-attribute-description = Необов’язковий атрибут замість стану
node-ha-trigger-state-prop-from-title = Зі стану
node-ha-trigger-state-prop-from-description = Необов’язкове попереднє значення стану або атрибута
node-ha-trigger-state-prop-to-title = У стан
node-ha-trigger-state-prop-to-description = Необов’язкове нове значення стану або атрибута
node-ha-trigger-state-prop-for-title = Протягом
node-ha-trigger-state-prop-for-description = Необов’язкова тривалість: 90, 1h 30m або 00:05:00

# ha/trigger_numeric_state
node-ha-trigger-numeric-state-title = Числовий стан
node-ha-trigger-numeric-state-description = Спрацьовує, коли значення сутності перетинає числову межу
node-ha-trigger-numeric-state-help =
    Оберіть сутність та нижню і/або верхню межу без включення граничних значень. Межа — число або ідентифікатор числової сутності. Home Assistant реагує на входження в діапазон, а не на кожну зміну всередині нього. Атрибут обирає числовий атрибут, шаблон значення перетворює його в Home Assistant. «Протягом» задає тривалість утримання в діапазоні. Вихід «Значення» містить число зі стану після зміни або обраного атрибута; локально шаблон не обчислюється. Якщо значення не є числом, вихід не оновлюється.

    Спрацювання надходять по порядку, не більше одного за такт. «Інформація» містить усі змінні тригера. Налаштуйте підключення до Home Assistant у параметрах плагіна. Неповні налаштування не створюють підписки. Зміна налаштувань замінює підписку та очищує чергу. Відхилена підписка повторюється через 30 секунд. Пропущені під час розриву зв’язку події не відтворюються; таймери «Протягом» починаються заново після відновлення підписки.
node-ha-trigger-numeric-state-pin-Triggered = Спрацювання
node-ha-trigger-numeric-state-pin-Info = Інформація
node-ha-trigger-numeric-state-pin-Entity = Сутність
node-ha-trigger-numeric-state-pin-From = До
node-ha-trigger-numeric-state-pin-To = Після
node-ha-trigger-numeric-state-pin-Value = Значення
node-ha-trigger-numeric-state-prop-entity_id-title = Сутність
node-ha-trigger-numeric-state-prop-entity_id-description = Оберіть сутність або введіть її ідентифікатор
node-ha-trigger-numeric-state-prop-attribute-title = Атрибут
node-ha-trigger-numeric-state-prop-attribute-description = Необов’язковий атрибут замість стану
node-ha-trigger-numeric-state-prop-above-title = Вище
node-ha-trigger-numeric-state-prop-above-description = Нижня межа без включення: число або ідентифікатор числової сутності
node-ha-trigger-numeric-state-prop-below-title = Нижче
node-ha-trigger-numeric-state-prop-below-description = Верхня межа без включення: число або ідентифікатор числової сутності
node-ha-trigger-numeric-state-prop-value_template-title = Шаблон значення
node-ha-trigger-numeric-state-prop-value_template-description = Необов’язковий шаблон Home Assistant для перетворення значення
node-ha-trigger-numeric-state-prop-for-title = Протягом
node-ha-trigger-numeric-state-prop-for-description = Необов’язкова тривалість: 90, 1h 30m або 00:05:00

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
    Оберіть особу або відстежувач пристрою, зону та вхід або вихід. Home Assistant визначає перетин межі зони. Виходи «Сутність», «До» та «Після» описують відстежувану сутність; відсутній стан має порожнє поле state.

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
