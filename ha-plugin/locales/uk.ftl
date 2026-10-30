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
