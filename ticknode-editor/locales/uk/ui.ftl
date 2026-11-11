# Connection status and editor header
status-online =
    { $count ->
        [one] підключено · { $count } нода
        [few] підключено · { $count } ноди
       *[other] підключено · { $count } нод
    }
status-online-home =
    { $count ->
        [one] підключено · { $count } аркуш
        [few] підключено · { $count } аркуші
       *[other] підключено · { $count } аркушів
    }
status-connecting = підключення…
status-offline = немає з'єднання з сервером

toolbar-undo = Скасувати (Ctrl/Cmd+Z)
toolbar-redo = Повторити (Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y)
toolbar-palette = Палітра нод
toolbar-plugins = Плагіни
toolbar-fit = Показати весь граф (F)
toolbar-add-frame = Додати рамку
toolbar-console = Консоль
toolbar-theme = Світла / темна тема
toolbar-language = Мова: Українська / English

# Помилки для користувача (ключі `UiMessage` від сервера; аргументи — рядки)
error-cycle-detected = Циклічне посилання: нода { $node } вже залежить від { $dependency }
error-type-mismatch = Несумісні типи даних: не можна з'єднати { $src } з { $dst }
error-invalid-pin-connection = Некоректне з'єднання пінів: один або обидва піни не існують або мають невірний напрямок чи тип
error-node-not-found = Ноду { $node } не знайдено
error-unknown-node-type = Невідомий тип ноди: { $type }
error-invalid-property = Неприпустима властивість: { $property }
error-node-plugin-unavailable = плагін { $plugin } недоступний
error-node-type-unknown = тип ноди { $type } невідомий: потрібного плагіна немає або його вимкнено
error-paste-too-many = Забагато нод для вставки: { $count } (максимум { $max })
error-paste-incomplete = Вставлено не все: { $details }
error-bad-message = Некоректне повідомлення: { $error }
error-setting-invalid = Налаштування «{ $setting }»: недопустиме значення
error-setting-required = Не задано обов'язкове налаштування «{ $setting }»
error-setting-unknown = Невідоме налаштування «{ $setting }»
error-settings-not-saved = Не вдалось зберегти налаштування: { $details }
error-plugin-not-found = Плагін { $plugin } не знайдено
error-plugin-not-loaded = Плагін { $plugin } не завантажено
error-plugin-crash = Аварія плагіна: { $reason }
error-plugin-disable-blocked = Плагін «{ $plugin }» не можна вимкнути: його типи даних використовує { $dependents } — спершу вимкніть їх
error-plugin-config-corrupt = Конфігурація плагіна пошкоджена: { $details }. Збереження налаштувань чи вмикання перезапише її
error-plugin-config-not-saved = Стан плагіна змінено, але конфігурацію не збережено: { $details }
error-condition-missing-pin = Не вказано номер піна («1: a > 0»)
error-condition-not-pin-number = «{ $text }» — не номер піна
error-condition-input-format = «{ $text }»: вхід записується як «pin:name:type»
error-condition-invalid-input-name = «{ $name }» — некоректне ім'я входу
error-condition-unknown-type = «{ $type }» — невідомий тип
error-condition-expected-join = { $end ->
    [true] Очікувалось «and» / «or», а не кінець тексту
   *[other] Очікувалось «and» / «or», а не «{ $found }»
}
error-condition-expected-operand = { $end ->
    [true] Очікувався операнд, а не кінець тексту
   *[other] Очікувався операнд, а не «{ $found }»
}
error-condition-expected-operator = { $end ->
    [true] Очікувався оператор, а не кінець тексту
   *[other] Очікувався оператор, а не «{ $found }»
}
error-condition-unknown-operator = «{ $operator }» — невідомий оператор
error-condition-unclosed-quote = Незакриті лапки
error-condition-not-number = «{ $text }» — не число
error-condition-doubled-operator = «{ $char }» — мабуть, «{ $char }{ $char }»
error-condition-unexpected-char = Неочікуваний символ «{ $char }»

# Автоматичні ноди типів даних плагінів
custom-make-title = Створити { $type }
custom-make-description = Збирає значення «{ $type }» з полів
custom-break-title = Розібрати { $type }
custom-break-description = Розбирає «{ $type }» на поля
custom-make-help =
    Збирає значення типу «{ $type }» ({ $name }) з окремих полів.

    Кожне поле — це вхід: під'єднайте джерело або задайте значення прямо на ноді. Обов'язкові поля є обов'язковими входами: без них значення не створюється (у журналі з'явиться помилка). Необов'язкове поле без значення пропускається.

    Чиста нода: обчислюється щоразу, коли її результат потрібен іншій ноді.
custom-break-help =
    Розбирає значення типу «{ $type }» ({ $name }) на окремі поля: кожне поле — вихідний пін.

    Необов'язкове поле, якого немає у значенні, не має значення (пін порожній).

    Чиста нода: обчислюється щоразу, коли її результат потрібен іншій ноді.

# Node categories
category-events = Події
category-control-flow = Керування
category-time = Час
category-actions = Дії
category-data = Дані
category-math = Математика
category-logic = Логіка
category-string = Рядки
category-lists = Списки
category-objects = Об'єкти
category-variables = Змінні
category-network = Мережа
category-debug = Налагодження
category-test = Тест

# Data types and input hints
type-exec = Виконання
type-number = Число
type-string = Текст
type-bool = Так/ні
type-time = Час
type-datetime = Дата й час
type-duration = Тривалість
type-color = Колір
type-object = Об’єкт
type-list = Список
type-any = Будь-що
type-auto = Авто
type-list-of = Список: { $inner }

hint-time = ГГ:ХХ
hint-datetime = РРРР-ММ-ДД ГГ:ХХ
hint-duration = 10t / 1h 30m / 45s
hint-color = #rrggbb
hint-default = —

parse-error-number = «{ $value }» — не число
parse-error-bool = «{ $value }» — не true/false
parse-error-time = «{ $value }» — не час (ГГ:ХХ або ГГ:ХХ:СС)
parse-error-datetime = «{ $value }» — не дата-час (РРРР-ММ-ДД ГГ:ХХ)
parse-error-duration = «{ $value }» — не тривалість (10t, 1h 30m, 500ms)
duration-input-hint = Введіть число у вибраній одиниці або текст, наприклад 1h 30m чи 10t
duration-unit-title = Одиниця тривалості
duration-unit-ticks = такт
duration-unit-ms = мс
duration-unit-s = с
duration-unit-min = хв
duration-unit-h = год
duration-ticks = { $count ->
    [one] { $count } такт
    [few] { $count } такти
    [many] { $count } тактів
   *[other] { $count } такту
    }
node-timer-active = { $count ->
    [one] { $count } активна затримка
    [few] { $count } активні затримки
   *[other] { $count } активних затримок
    }
node-timer-count = ×{ $count }
parse-error-color = «{ $value }» — не колір (#rrggbb)
parse-error-not-text = значення типу { $type } не набирається текстом

# Shared controls and inline node fields
ui-save = SAVE
inline-value-type-title = Тип значення
inline-pin-value-title = Значення, поки до входу нічого не під'єднано. Порожнє — нода бере своє за замовчуванням

# Canvas and nodes
canvas-empty-title = Порожній граф
canvas-empty-hint = Додайте ноду з палітри й з'єднайте піни перетягуванням
node-plugin-unavailable = плагін недоступний
node-missing-required-title = Не задано обов'язковий вхід — нода не виконується
node-run-title = Запустити
pin-required-suffix = { " " }· обов'язковий

# Match condition editor
match-section-inputs = входи
match-add-input = + вхід
match-add-input-title = Додати вхід (новий пін даних), щоб порівнювати кілька значень
match-section-cases = випадки — згори вниз
match-add-case = + випадок
match-add-case-title = Додати випадок (новий вихід)
match-else = інакше
match-error-bad-input-name = «{ $name }» — некоректне ім'я входу (літери, цифри, _; не оператор)
match-error-duplicate-input = вхід «{ $name }» уже є
match-input-name-title = Ім'я входу — на нього посилаються умови
match-input-type-title = Тип входу; auto — будь-який, оператори підбираються за під'єднаним значенням
match-connect-hint = під'єднайте
match-remove-input-title = Видалити вхід (і умови з ним)
match-move-up-title = Вище
match-move-down-title = Нижче
match-add-condition = + умова
match-add-condition-title = Додати умову до цього випадку
match-remove-case-title = Видалити випадок (і його вихід)
match-then = тоді → { $index }
match-always = завжди
match-if = якщо
match-join-title = Перемкнути «і» / «або» («і» зв'язує сильніше)
match-join-and = і
match-join-or = або
match-left-input-title = Вхід: { $type }
match-op-title = Оператор (залежить від типу входу)
match-right-source-title = Праворуч — значення (✎) або інший вхід
match-right-value-title = Значення для порівняння
match-remove-condition-title = Прибрати умову

# Match condition operators in display
match-op-eq = =
match-op-ne = ≠
match-op-lt = <
match-op-le = ≤
match-op-gt = >
match-op-ge = ≥
match-op-lt-time = < раніше
match-op-le-time = ≤ не пізніше
match-op-gt-time = > пізніше
match-op-ge-time = ≥ не раніше
match-op-lt-duration = < коротша
match-op-gt-duration = > довша
match-op-contains = містить
match-op-not-contains = не містить
match-op-starts-with = починається з
match-op-ends-with = закінчується на
match-op-is-empty = порожній
match-op-not-empty = не порожній
match-op-is-true = = true
match-op-is-false = = false
match-op-has-field = має поле
match-op-no-field = без поля
match-op-len-eq = довжина =
match-op-len-gt = довжина >
match-op-len-lt = довжина <
match-op-has-value = є значення
match-op-no-value = немає значення

# Node property labels
prop-value = Значення
prop-var-name = Змінна
prop-expression = Формула
prop-prefix = Префікс
prop-label = Мітка
prop-element-type = Тип елементів
prop-value-type = Тип
prop-field-type = Тип поля
prop-fields-list = Поля (name:type, …)
prop-interval-ticks = Інтервал, тактів
prop-cases = Випадки

# Relative execution time
time-just-now = щойно
time-seconds-ago =
    { $count ->
        [one] { $count } секунду тому
        [few] { $count } секунди тому
        [many] { $count } секунд тому
       *[other] { $count } секунд тому
    }
time-minutes-ago =
    { $count ->
        [one] { $count } хвилину тому
        [few] { $count } хвилини тому
        [many] { $count } хвилин тому
       *[other] { $count } хвилин тому
    }
time-hours-ago =
    { $count ->
        [one] { $count } годину тому
        [few] { $count } години тому
        [many] { $count } годин тому
       *[other] { $count } годин тому
    }
time-days-ago =
    { $count ->
        [one] { $count } день тому
        [few] { $count } дні тому
        [many] { $count } днів тому
       *[other] { $count } днів тому
    }

# Node palette
palette-search-placeholder = Пошук нод
palette-empty = Нічого не знайдено
palette-info-aria = Як працює нода

# Graph overview and inspector
overview-title-graph = Граф
overview-stats = { $nodes } нод · { $connections } з'єднань
overview-title-controls = Керування
overview-hint-1 = Палітра: клік — нода в центр, перетягування — нода на місце
overview-hint-2 = Тягніть від піна до піна, щоб з'єднати; підключений вхід даних — щоб перенести дріт
overview-hint-3 = Рамка ЛКМ: зліва направо — ноди повністю всередині, справа наліво — всі, яких торкнулась
overview-hint-4 = Shift/Ctrl — додати до виділення; виділені ноди тягнуться разом
overview-hint-5 = Ctrl+C / Ctrl+X / Ctrl+V — буфер, Ctrl+D — дублювати, Ctrl+A — усе; Ctrl/Cmd+Z — скасувати, Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y — повторити
overview-hint-6 = Вставка їде за курсором: ЛКМ — поставити, ПКМ або Esc — скасувати, середня кнопка — зсув
overview-hint-7 = Delete — видалити виділене, Esc — зняти виділення
overview-hint-8 = Середня кнопка — зсув, колесо — масштаб, F — весь граф; на сенсорі — палець і два пальці
overview-hint-9 = Shift + ПКМ — копіювати налаштування ноди; Shift + ЛКМ — вставити в ноду того самого типу; Esc — очистити скопійовані налаштування
settings-copied = Налаштування скопійовано з ноди «{ $title }»
error-settings-type-mismatch = Неможливо вставити налаштування з { $from } у { $to }

inspector-node-not-found = Ноду не знайдено
inspector-section-actions = Дії
inspector-duplicate = Дублювати (Ctrl+D)
inspector-delete-node = Видалити ноду

exec-section-title = Виконання
exec-section-inputs = Входи
exec-section-outputs = Виходи
exec-when = Виконано { $time }
exec-never = Ще не виконувалась
exec-tick = такт { $tick }
exec-node-never-hint = Нода ще не виконувалась
exec-fired = ▶ спрацював
exec-not-fired-hint = Не спрацював в останньому виконанні
exec-no-value = немає значення
exec-no-value-hint = Джерело ще не мало значення
exec-connected-after-hint = Під'єднано після останнього виконання
exec-not-connected = не під'єднано
exec-default-used-hint = Нода використала значення за замовчуванням
exec-output-not-emitted-hint = Значення ще не видавалось

group-selected =
    { $count ->
        [one] Виділено { $count } ноду
        [few] Виділено { $count } ноди
        [many] Виділено { $count } нод
       *[other] Виділено { $count } нод
    }
group-hint = Тягніть будь-яку з них, щоб перемістити всі. Ctrl+V — вставити копію: ЛКМ ставить, ПКМ скасовує.
group-copy = Копіювати (Ctrl+C)
group-delete =
    { $count ->
        [one] Видалити { $count } ноду
        [few] Видалити { $count } ноди
        [many] Видалити { $count } нод
       *[other] Видалити { $count } нод
    }

conn-type-exec = Exec-з'єднання
conn-type-data = З'єднання даних
conn-from = з
conn-to = до
conn-disconnect = Від'єднати

# Console
console-title = Консоль
console-lines = { $count } рядків
console-clear = Очистити
console-hide = Сховати консоль

# Plugins panel
plugins-title = Плагіни
plugins-hide = Сховати панель плагінів
plugins-empty = Плагінів не знайдено. Покладіть скомпільовані компоненти (*.wasm) у data/plugins/ і перезапустіть сервер.
plugins-cap-http-any = HTTP: будь-які домени
plugins-cap-http-domains = HTTP: { $domains }
plugins-cap-storage = Сховище: власний каталог плагіна
plugins-cap-ws-any = WebSocket: будь-які хости
plugins-cap-ws-hosts = WebSocket: { $hosts }
plugins-state-enabled = працює
plugins-state-disabled = вимкнено
plugins-state-error = помилка
plugins-settings-error-chip = помилка налаштувань
plugins-settings-error-title = Плагін відхилив налаштування
plugins-capabilities = Права
plugins-settings = Налаштування
plugins-save = Зберегти
plugins-secret-will-clear = буде очищено
plugins-secret-set = задано
plugins-secret-unset = не задано
plugins-secret-clear = Очистити
plugins-secret-clear-title = Видалити збережене значення
plugins-field-error = «{ $field }»: { $error }

# Пошуковий вибір властивості (наприклад, сутності Home Assistant)
picker-placeholder = оберіть…
picker-search-placeholder = Пошук за назвою або ідентифікатором
picker-empty = Нічого не знайдено
picker-no-options = Списку поки немає (плагін його ще не надав). Можна ввести значення вручну.
picker-more = Ще { $count } - уточніть пошук
picker-use-typed = Використати "{ $value }"

# Implicit conversion on a data connection.
wire-conversion = { $src } → { $dst }

pin-split = Розділити пін структури
pin-collapse = Згорнути пін структури
pin-technical-type = Технічний тип: { $name }
type-unavailable = Недоступний тип

error-batch-command = Ця команда не дозволена всередині групи

inspector-cost = вартість { $cost }
inspector-cost-help = Відносний обсяг роботи за виклик; базова логіка коштує 1. Бюджет тіку розподіляє роботу з черги між тіками.

wire-menu-empty = Немає сумісних нод
wire-menu-popular = Часто вживані
quick-add-recent = Нещодавні
quick-add-all-nodes = Усі ноди
overview-hint-quick-add = Подвійний клік, Space або Tab на канві — швидко додати ноду

wire-point-add = Додати точку
wire-point-remove = Видалити точку
wire-points-clear = Видалити всі точки
error-wire-route = Некоректний маршрут дроту: з’єднання має існувати, а маршрут — містити не більше 32 точок зі скінченними координатами.

error-plugin-api-mismatch = Плагін зібрано для API плагінів { $plugin_api } (SDK { $plugin_sdk }), а сервер використовує { $server_api } (TickNode { $server_version }). Перезберіть плагін з поточним SDK.
error-plugin-api-unknown = Плагін зібрано з SDK, який ще не підтримував перевірку API плагінів. Сервер використовує API плагінів { $server_api } (TickNode { $server_version }). Перезберіть плагін з поточним SDK.

# Unconfirmed pin contracts and detached wires
node-frozen-title = Піни змінились
node-frozen-confirm = Все правильно
node-frozen-help = Вузол не працюватиме, доки ви не підтвердите зміни. Від’єднані дроти залишаться, доки ви не видалите або не під’єднаєте їх знову.
pin-change-removed = Видалено { $direction } пін «{ $name }» ({ $kind })
pin-change-changed = Пін «{ $name }»: { $old } → { $new }
pin-change-required = Новий обов’язковий вхід «{ $name }» ({ $kind })
pin-direction-input = вхідний
pin-direction-output = вихідний
wire-detached-title = Дріт від'єднано: пін змінився
wire-detached-remove = Видалити дріт

node-resize-title = Тягніть, щоб змінити розмір; подвійний клік — автоматичний розмір
overview-hint-resize = Тягніть нижній правий кут, щоб змінити розмір вузла; подвійний клік скидає розмір. Esc скасовує. У багаторядковому полі Ctrl/Cmd+Enter зберігає.
overview-hint-frame = C — додати рамку навколо виділених нод; перетягніть заголовок рамки, щоб посунути її разом із нодами всередині; кут — змінити розмір; подвійний клік по заголовку — перейменувати
overview-hint-nudge = Стрілки — посунути виділені ноди або рамку на клітинку сітки, з Alt — на дрібний крок, з Shift — на п'ять клітинок
error-node-size = Розміри вузла мають бути скінченними числами

# Runtime node errors and trace values on the canvas
error-node-required-missing = Обов'язковий вхід «{ $pin }» не задано
error-node-required-conversion = Не вдалося перетворити значення для входу «{ $pin }»
error-node-frozen = Вузол заморожено: його піни змінились — підтвердіть зміни
error-plugin-node-crashed = Плагін аварійно завершився: { $reason }
toolbar-show-values = Показувати значення
pin-no-value = Значення ще немає
pin-fired-ago = Спрацював { $time }

# Вкладки лівої панелі та ресурси
left-tab-nodes = Ноди
left-tab-resources = Ресурси
left-tab-variables = Змінні

variables-coming-soon = Змінні графа з'являться згодом: оголошені типізовані значення, які можна буде переглядати й змінювати тут.
variables-scope-sheet = Цей аркуш
variables-scope-global = Глобальні
variables-scope-persistent = Постійні — зберігаються після перезапуску
variables-shadowed = На цьому аркуші перекрита змінною аркуша
variables-delete-confirm = Видалити змінну «{ $name }»?
variables-new = Нова змінна
variables-scope-sheet-short = Аркуш
variables-scope-global-short = Глобальна
variables-scope-persistent-short = Постійна
variables-empty = Змінних ще немає. Їх додають ноди «Змінна: створити» або кнопка вище.
variables-no-match = Немає змінних, що відповідають пошуку
variables-action-get = Отримати значення
variables-action-set = Встановити значення
variables-action-changed = Коли змінюється
variables-action-delete = Видалити
variables-search-placeholder = Пошук змінних...
variables-create = Створити
variables-cancel = Скасувати
variables-name = Назва
variables-type = Тип
variables-scope = Область видимості
variables-initial-value = Початкове значення
variables-expand = Розгорнути
variables-collapse = Згорнути
variables-add-to-canvas = Додати на полотно
variables-toggle-multiline = Багаторядковий режим

resources-more =
    { $count ->
        [one] і ще { $count } — уточніть пошук
        [few] і ще { $count } — уточніть пошук
       *[other] і ще { $count } — уточніть пошук
    }
resources-empty = Увімкнені плагіни не пропонують ресурсів
resources-loading = Поки що порожньо

error-frame-invalid = Позиція і розмір рамки мають бути скінченними числами
error-frame-limit = Забагато рамок
error-sheet-limit = Забагато аркушів
error-cross-sheet = Не можна з'єднувати ноди з різних аркушів
error-no-sheet = Спершу відкрийте аркуш
error-sheet-not-found = Аркуш не знайдено
error-variable-missing = Змінну «{ $name }» не знайдено
error-variable-type = Значення не відповідає типу змінної
error-variable-limit = Досягнуто ліміту змінних
error-variable-name = Некоректне ім'я змінної «{ $name }»
error-variable-exists = Змінна вже існує у цій області видимості
error-variable-scope = Глобальні та збережені змінні не можуть належати аркушу
frame-default-title = Коментар
frame-resize-title = Тягніть, щоб змінити розмір рамки
frame-color-gray = Сірий
frame-color-red = Червоний
frame-color-orange = Помаранчевий
frame-color-yellow = Жовтий
frame-color-green = Зелений
frame-color-teal = Бірюзовий
frame-color-blue = Синій
frame-color-purple = Фіолетовий

# Аркуші та головна сторінка
sheets-back = Усі аркуші
sheet-disabled = Вимкнено
home-sheets = Аркуші сценаріїв
home-variables = Глобальні змінні
home-log = Журнал
home-settings = Налаштування
sheet-new = Новий аркуш
sheet-new-title = Новий аркуш
sheet-no-caption = Без опису
sheet-nodes =
    { $count ->
        [one] { $count } нода
        [few] { $count } ноди
        [many] { $count } нод
       *[other] { $count } нод
    }
sheet-errors =
    { $count ->
        [one] { $count } помилка
        [few] { $count } помилки
        [many] { $count } помилок
       *[other] { $count } помилок
    }
sheet-last-run = Останній запуск { $time }
sheet-never-run = Ще не запускався
sheet-enabled = Увімкнено
sheet-edit = Редагувати
sheet-duplicate = Дублювати
sheet-move-up = Вгору
sheet-move-down = Вниз
sheet-delete = Видалити
sheet-save = Зберегти
sheet-cancel = Скасувати
sheet-delete-confirm = Видалити аркуш «{ $title }»? Його буде перенесено в теку кошика на сервері.
sheets-empty = Аркушів ще немає — створіть перший

# Розділ налаштувань
settings-interface = Інтерфейс
settings-language = Мова
settings-language-description = Мова інтерфейсу редактора
settings-theme = Тема
settings-theme-dark = Темна
settings-theme-light = Світла
settings-theme-description = Колірна схема редактора
settings-show-values = Показувати значення на зв'язках
settings-show-values-description = Відображати поточні значення даних безпосередньо вздовж ліній зв'язку
settings-engine = Рушій
settings-engine-note = Діє для сервера і всіх відкритих редакторів
settings-tick = Тривалість такту (мс)
settings-tick-description = Як часто рушій виконує крок. Кожен перехід exec займає один такт. Коротші такти реагують швидше й використовують більше процесора.
settings-budget = Бюджет роботи на такт
settings-budget-description = Обмежує відносний обсяг роботи на такт і відкладає решту на наступні такти; 0 означає без обмежень.
settings-save = Зберегти
settings-reset = Скинути
settings-invalid-number = Некоректне число
error-save-settings = Не вдалося зберегти налаштування

settings-formatting-language = Мова форматування
settings-formatting-language-description = Мова тексту, який створює рушій.
language-en = Англійська
language-uk = Українська
quantity-unit-title = Одиниця вимірювання
parse-error-quantity = «{ $value }» не є значенням типу { $type }.
error-plugin-quantity-type = Плагін { $plugin } не може оголошувати зарезервований тип фізичної величини { $type }.

settings-display = Одиниці та форматування
settings-display-description = Правила використовують модуль канонічного значення. Пороги вводяться в обраній одиниці. Правила діють для всіх аркушів і серверного тексту.
settings-display-default = Відновити типове правило
settings-display-from = Від |значення|
settings-display-remove = Видалити крок
settings-display-add = + крок
settings-display-digits = Цифри
settings-display-invalid = Задайте 1–8 кроків від нуля зі скінченними зростаючими порогами; кількість цифр — 1–15 для значущих цифр або 0–15 для знаків після коми.
error-display-quantity = Невідома величина у правилі: { $quantity }.
error-display-unit = Невідома одиниця у правилі: { $quantity }.
error-display-digits = Правило для { $quantity } потребує 1–15 значущих цифр або 0–15 знаків після коми.
error-display-steps = Правило для { $quantity } потребує 1–8 кроків.
error-display-threshold = Пороги для { $quantity } мають починатися з нуля, бути скінченними, невід’ємними та строго зростати.
settings-display-rounding = Округлення
settings-display-significant = Значущі цифри
settings-display-decimals = Знаки після коми
settings-display-numbers = Числа

error-hysteresis-thresholds = Некоректні пороги гістерезису: у режимі зростання Поріг увімкнення має бути ≥ Порога вимкнення; у режимі спадання — ≤ Порога вимкнення

error-property-duration = Введіть коректну тривалість.
error-property-time = Введіть час у форматі ГГ:ХХ або ГГ:ХХ:СС.
error-property-multi-select = Оберіть значення з доступних варіантів.
error-property-text = Введіть текст.
property-duration-earlier = Раніше (від’ємне зміщення)
property-duration-later = Пізніше (додатне зміщення)
property-time-placeholder = HH:MM

property-from-input = Сутність із входу

property-duration-placeholder = 30
property-duration-hint = Число у вибраній одиниці, 1h 30m або ГГ:ХХ:СС
