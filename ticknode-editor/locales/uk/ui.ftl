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
error-invalid-variable-ref = Некоректне посилання на змінну: { $reference }
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
inline-pin-string-title = Значення, поки до входу нічого не під'єднано. "var.x" у лапках задає буквальний текст. Порожнє — нода бере своє за замовчуванням
inline-variable-tooltip = Змінна «{ $name }» ({ $type }): { $value }
inline-variable-not-found = Змінну «{ $name }» не знайдено
inline-variable-clear = Видалити прив'язку до змінної
inline-variable-edit-title = Посилання на змінну (var.x, sheet.x, global.x) або значення

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
overview-stats = { $nodes } нод · { $connections } з'єднань
overview-title-controls = Керування
overview-hint-1 = Палітра: клік — нода в центр, перетягування — нода на місце
overview-hint-2 = Тягніть від піна до піна, щоб з'єднати; підключений вхід даних — щоб перенести дріт
overview-hint-3 = Рамка ЛКМ: зліва направо — ноди повністю всередині, справа наліво — всі, яких торкнулась
overview-hint-4 = Shift/Ctrl — додати до виділення; виділені ноди тягнуться разом
overview-hint-5 = Ctrl+C / Ctrl+X / Ctrl+V — буфер, Ctrl+D — дублювати, Ctrl+A — усе; Ctrl/Cmd+Z — скасувати, Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y — повторити
overview-hint-6 = Вставка їде за курсором: ЛКМ — поставити, ПКМ або Esc — скасувати, середня кнопка — зсув
overview-hint-7 = Delete — видалити виділене, Esc — зняти виділення; ПКМ на ноді чи дроті — меню дій
overview-hint-8 = Середня кнопка — зсув, колесо — масштаб, F — весь граф; на сенсорі — палець і два пальці
overview-hint-9 = Shift + ПКМ — копіювати налаштування ноди; Shift + ЛКМ — вставити в ноду того самого типу; Esc — очистити скопійовані налаштування
settings-copied = Налаштування скопійовано з ноди «{ $title }»
error-settings-type-mismatch = Неможливо вставити налаштування з { $from } у { $to }

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
node-frozen-help = Нода не працюватиме, доки ви не підтвердите зміни. Від’єднані дроти залишаться, доки ви не видалите або не під’єднаєте їх знову.
pin-change-removed = Видалено { $direction } пін «{ $name }» ({ $kind })
pin-change-changed = Пін «{ $name }»: { $old } → { $new }
pin-change-required = Новий обов’язковий вхід «{ $name }» ({ $kind })
pin-direction-input = вхідний
pin-direction-output = вихідний
wire-detached-remove = Видалити дріт

node-resize-title = Тягніть, щоб змінити розмір; подвійний клік — автоматичний розмір
overview-hint-resize = Тягніть нижній правий кут, щоб змінити розмір ноди; подвійний клік скидає розмір. Esc скасовує. У багаторядковому полі Ctrl/Cmd+Enter зберігає.
overview-hint-frame = C — додати рамку навколо виділених нод; перетягніть заголовок рамки, щоб посунути її разом із нодами всередині; кут — змінити розмір; подвійний клік по заголовку — перейменувати
overview-hint-nudge = Стрілки — посунути виділені ноди або рамку на клітинку сітки, з Alt — на дрібний крок, з Shift — на п'ять клітинок
error-node-size = Розміри ноди мають бути скінченними числами

# Runtime node errors and trace values on the canvas
error-node-required-missing = Обов'язковий вхід «{ $pin }» не задано
error-node-required-conversion = Не вдалося перетворити значення для входу «{ $pin }»
error-node-required-variable = Змінна «{ $variable }» для обов'язкового входу «{ $pin }» недоступна
error-node-frozen = Нода заморожена: її піни змінились — підтвердіть зміни
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
home-usage = Навантаження
home-settings = Налаштування
usage-title = Статистика навантаження
usage-refresh = Оновити
usage-window-minutes = Останні 60 хвилин
usage-window-hours = Останні 48 годин
usage-ticks = Тіки
usage-avg-tick = Сер. час тіка
usage-max-tick = Макс. час тіка
usage-spent-work = Витрачена робота
usage-deferred = Відкладені
usage-plugins = Паливо плагінів
usage-plugin-name = Плагін
usage-fuel = Паливо
usage-calls = Виклики
usage-traps = Помилки (traps)
usage-top-nodes = Топ нід за вартістю
usage-node-name = Нода
usage-sheet = Аркуш
usage-cost = Вартість
usage-executions = Виконання
usage-other-nodes = Інші ноди
usage-sheets-title = Робота за аркушами
usage-empty = Статистику ще не зібрано
usage-current-tick = Поточний тік
usage-main-sheet = Головний
usage-global = Глобальний
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
settings-formatting-language-description = Мова тексту, який створює рушій, зокрема в нодах «У текст» і «Текст за шаблоном».
property-locale-server = Сервер ({ $language })
language-en = Англійська
language-uk = Українська
quantity-unit-title = Одиниця вимірювання
parse-error-quantity = «{ $value }» не є значенням типу { $type }.
error-plugin-quantity-type = Плагін { $plugin } не може оголошувати зарезервований тип фізичної величини { $type }.
error-plugin-media-type = Плагін { $plugin } не може оголошувати зарезервований тип медіа { $type }.
error-media-missing-source = Об'єкт медіа повинен містити url, file_id або blob

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

property-from-input = Із входу

property-duration-placeholder = 30
property-duration-hint = Число у вибраній одиниці, 1h 30m або ГГ:ХХ:СС

parse-error-identifier = «{ $value }» не є цілим 64-бітним числом зі знаком
identifier-clear = Очистити ідентифікатор
error-plugin-identifier-output = Плагін повернув некоректне значення ідентифікатора

template-error = Рядок { $line }, поз { $column }: { $message }
template-render-error = Помилка шаблону: { $message }
template-preview-empty = (порожньо)
template-preview-waiting = Попередній перегляд зʼявиться після отримання значень
template-preview-samples = Спробувати значення
template-preview-sample-badge = зразок
template-preview-samples-title = Зразки значень
template-preview-samples-clear = Очистити
template-preview-no-waiting-inputs = Всі входи мають значення
template-hover-text-of = Форматовано як «У текст» ({ $name })
template-lint-unknown-filter = Невідомий фільтр '{ $name }'
template-lint-unknown-filter-suggest = Невідомий фільтр '{ $name }'. Можливо, ви мали на увазі '{ $suggestion }'?
template-lint-filter-type = Фільтр '{ $name }' не застосовується до { $type }
template-lint-unknown-field = Невідоме поле '{ $field }' у { $type }
template-lint-unknown-field-suggest = Невідоме поле '{ $field }' у { $type }. Можливо, ви мали на увазі '{ $suggestion }'?
template-lint-unknown-test = Невідомий тест '{ $name }'
template-lint-unknown-variable = Невідома змінна '{ $name }'
template-lint-unknown-variable-suggest = Невідома змінна '{ $name }'. Можливо, ви мали на увазі '{ $suggestion }'?

template-help-filter-date = Форматує дату й час або час: date('DD.MM.YYYY HH:mm') чи пресет; у лапках доступні коди формату
template-help-filter-strftime = Форматує дату й час або час за допомогою кодів strftime
template-help-filter-text = Форматує значення з урахуванням одиниці вимірювання та локалі
template-help-filter-plural = Вибирає форму множини: позиційні форми відповідають правилам мови форматування (напр. one, few, many), ключові слова zero/one/two/few/many/other, а # замінюється на відформатоване число
template-help-filter-default = Резервне значення, якщо вхідне значення невизначене або порожнє
template-help-function-now = Поточна дата й час як рядок ISO 8601 / RFC 3339
template-help-filter-ago = Форматує відносний час, наприклад '5 хвилин тому' або 'через 2 години'
template-help-filter-duration = Форматує тривалість за допомогою компактних одиниць ('short'), слів ('long') чи шаблону 'H:mm'
template-help-filter-unit = Форматує число або фізичну величину за правилами відображення проєкту чи вказаною одиницею вимірювання
template-help-unit-symbol = Одиниця вимірювання для { $quantity }

template-help-date-preset-datetime = Стандартні дата й час
template-help-date-preset-date = Тільки дата
template-help-date-preset-time = Тільки час
template-help-date-preset-long = Повна дата з назвою місяця мовою форматування
template-help-date-preset-iso = Дата й час у стандарті ISO 8601

template-help-date-token-year4 = Рік, 4 цифри
template-help-date-token-year2 = Рік, 2 цифри
template-help-date-token-month-name = Назва місяця (з днем у шаблоні — у формі для дат)
template-help-date-token-month-short = Скорочена назва місяця
template-help-date-token-month2 = Місяць, 2 цифри (01–12)
template-help-date-token-month1 = Місяць, 1 або 2 цифри (1–12)
template-help-date-token-day2 = День місяця, 2 цифри (01–31)
template-help-date-token-day1 = День місяця, 1 або 2 цифри (1–31)
template-help-date-token-weekday-name = Повна назва дня тижня
template-help-date-token-weekday-short = Скорочена назва дня тижня
template-help-date-token-hour24-2 = Година 00–23, 2 цифри
template-help-date-token-hour24-1 = Година 0–23, 1 або 2 цифри
template-help-date-token-hour12-2 = Година 01–12, використовується з A для AM/PM
template-help-date-token-hour12-1 = Година 1–12, використовується з A для AM/PM
template-help-date-token-minute2 = Хвилини, 2 цифри (00–59)
template-help-date-token-minute1 = Хвилини, 1 або 2 цифри (0–59)
template-help-date-token-second2 = Секунди, 2 цифри (00–59)
template-help-date-token-second1 = Секунди, 1 або 2 цифри (0–59)
template-help-date-token-millis = Мілісекунди, 3 цифри
template-help-date-token-ampm-upper = AM або PM, великими літерами
template-help-date-token-ampm-lower = am або pm, малими літерами
template-help-date-token-offset-plain = Зсув UTC без двокрапки, наприклад +0200
template-help-date-token-offset-colon = Зсув UTC із двокрапкою, наприклад +02:00
template-help-date-literal = Текст у квадратних дужках виводиться без змін, наприклад [о]

template-help-strftime-upper-y = Рік, 4 цифри
template-help-strftime-y = Рік, 2 цифри (00–99)
template-help-strftime-m = Місяць, 2 цифри (01–12)
template-help-strftime-upper-b = Повна назва місяця
template-help-strftime-b = Скорочена назва місяця
template-help-strftime-d = День місяця, 2 цифри (01–31)
template-help-strftime-e = День місяця з доповненням пробілом ( 1–31)
template-help-strftime-upper-a = Повна назва дня тижня
template-help-strftime-a = Скорочена назва дня тижня
template-help-strftime-j = День року, 3 цифри (001–366)
template-help-strftime-upper-h = Година 00–23, 2 цифри
template-help-strftime-upper-i = Година 01–12, 2 цифри
template-help-strftime-upper-m = Хвилини, 2 цифри (00–59)
template-help-strftime-upper-s = Секунди, 2 цифри (00–59)
template-help-strftime-f = Мікросекунди, 6 цифр
template-help-strftime-p = Позначення AM або PM
template-help-strftime-z = Зсув UTC (+HHMM або -HHMM)
template-help-strftime-upper-z = Назва або абревіатура часового поясу
template-help-strftime-percent = Символ % без змін

template-help-duration-preset-short = Компактні локалізовані одиниці
template-help-duration-preset-long = Слова з формами множини

template-help-duration-token-days = Повні дні
template-help-duration-token-hours2 = Години, 2 цифри (у межах дня за наявності d, інакше загальні години)
template-help-duration-token-hours1 = Години, 1 або 2 цифри (у межах дня за наявності d, інакше загальні години)
template-help-duration-token-minutes2 = Хвилини, 2 цифри (у межах години за наявності d/H/HH, інакше загальні хвилини)
template-help-duration-token-minutes1 = Хвилини, 1 або 2 цифри (у межах години за наявності d/H/HH, інакше загальні хвилини)
template-help-duration-token-seconds2 = Секунди в межах хвилини, 2 цифри
template-help-duration-token-seconds1 = Секунди в межах хвилини, 1 або 2 цифри
template-help-duration-token-millis = Мілісекунди, 3 цифри
template-help-duration-literal = Текст у квадратних дужках виводиться без змін, наприклад [год]

template-help-filter-abs = Повертає абсолютне значення числа.
template-help-filter-attr = Отримує атрибут або елемент об'єкта.
template-help-param-filter-attr-name = Назва атрибута для пошуку
template-help-filter-batch = Розбиває послідовність на групи вказаного розміру.
template-help-param-filter-batch-size = Кількість елементів у кожній групі
template-help-param-filter-batch-fill_with = Необов'язкове значення для заповнення неповної групи
template-help-filter-bool = Перетворює значення на логічне.
template-help-filter-capitalize = Робить перший символ великим, а решту — малими.
template-help-filter-count = Повертає кількість елементів у послідовності чи рядку.
template-help-param-filter-d-default = Резервне значення, якщо вхідне значення невизначене або порожнє
template-help-param-filter-date-pattern = Шаблон форматування дати й часу або пресет
template-help-param-filter-default-value = Резервне значення, якщо вхідне значення невизначене або порожнє
template-help-filter-dictsort = Сортує словник за ключем або значенням.
template-help-param-filter-dictsort-case_sensitive = Чи враховувати регістр під час сортування
template-help-param-filter-dictsort-by = Сортувати за 'key' або 'value'
template-help-param-filter-dictsort-reverse = Сортувати у зворотному порядку
template-help-filter-e = Псевдонім для escape.
template-help-filter-escape = Екранує символи HTML у рядку.
template-help-filter-first = Повертає перший елемент послідовності чи рядка.
template-help-filter-float = Перетворює значення на число з плаваючою комою.
template-help-param-filter-float-default = Резервне значення у разі помилки перетворення
template-help-filter-format = Форматує рядок із плейсхолдерами у стилі printf.
template-help-param-filter-format-args = Позиційні значення для плейсхолдерів у стилі printf
template-help-filter-groupby = Групує послідовність об'єктів за атрибутом.
template-help-param-filter-groupby-attribute = Назва атрибута для групування
template-help-filter-indent = Додає відступи до рядків тексту.
template-help-param-filter-indent-first = Чи робити відступ на першому рядку
template-help-param-filter-indent-blank = Чи робити відступ на порожніх рядках
template-help-filter-int = Перетворює значення на ціле число.
template-help-param-filter-int-default = Резервне значення у разі помилки перетворення
template-help-param-filter-int-base = Основа системи числення (наприклад 10 або 16)
template-help-filter-items = Повертає ітератор пар ключ-значення словника.
template-help-filter-join = Об'єднує елементи послідовності в рядок.
template-help-param-filter-join-separator = Рядок-розділювач між елементами
template-help-filter-last = Повертає останній елемент послідовності чи рядка.
template-help-filter-length = Повертає кількість елементів у послідовності, словнику чи рядку.
template-help-filter-lines = Розбиває рядок на список рядків.
template-help-filter-list = Перетворює ітерований об'єкт на список.
template-help-filter-lower = Переводить усі символи рядка в нижній регістр.
template-help-filter-map = Застосовує фільтр або отримує атрибут для кожного елемента послідовності.
template-help-filter-max = Повертає найбільший елемент послідовності.
template-help-param-filter-max-case_sensitive = Чи враховувати регістр під час порівняння рядків
template-help-param-filter-max-attribute = Необов'язковий атрибут для порівняння елементів
template-help-filter-min = Повертає найменший елемент послідовності.
template-help-param-filter-min-case_sensitive = Чи враховувати регістр під час порівняння рядків
template-help-param-filter-min-attribute = Необов'язковий атрибут для порівняння елементів
template-help-filter-pprint = Форматує значення у зручному для читання вигляді.
template-help-filter-reject = Фільтрує послідовність, вилучаючи елементи, які проходять тест.
template-help-param-filter-reject-test_name = Назва тесту для відкидання відповідних елементів
template-help-filter-rejectattr = Фільтрує об'єкти, вилучаючи елементи, чий атрибут проходить тест.
template-help-param-filter-rejectattr-attribute = Назва атрибута для перевірки
template-help-param-filter-rejectattr-test_name = Необов'язкова назва тесту для відкидання елементів
template-help-filter-replace = Замінює входження одного підрядка іншим.
template-help-param-filter-replace-old = Підрядок для пошуку
template-help-param-filter-replace-new = Підрядок для заміни
template-help-param-filter-replace-count = Максимальна кількість замін
template-help-filter-reverse = Розгортає послідовність чи рядок у зворотному порядку.
template-help-filter-round = Округлює число до вказаної точності.
template-help-param-filter-round-precision = Кількість знаків після коми для округлення
template-help-filter-safe = Позначає рядок як безпечний HTML.
template-help-filter-select = Фільтрує послідовність, залишаючи лише елементи, які проходять тест.
template-help-param-filter-select-test_name = Назва тесту для збереження лише відповідних елементів
template-help-filter-selectattr = Фільтрує об'єкти, залишаючи елементи, чий атрибут проходить тест.
template-help-param-filter-selectattr-attribute = Назва атрибута для перевірки
template-help-param-filter-selectattr-test_name = Необов'язкова назва тесту для вибору елементів
template-help-filter-slice = Розбиває послідовність на вказану кількість частин.
template-help-param-filter-slice-slices = Кількість частин для створення
template-help-param-filter-slice-fill_with = Необов'язкове значення для заповнення неповної частини
template-help-filter-sort = Сортує елементи послідовності.
template-help-param-filter-sort-reverse = Сортувати у спадному порядку
template-help-param-filter-sort-case_sensitive = Чи враховувати регістр під час сортування
template-help-param-filter-sort-attribute = Необов'язковий атрибут для сортування
template-help-filter-split = Розбиває рядок на список за вказаним розділювачем.
template-help-param-filter-strftime-format = Рядок форматування strftime
template-help-filter-string = Перетворює значення на рядок.
template-help-filter-sum = Повертає суму чисел або значень атрибутів послідовності.
template-help-param-filter-sum-attribute = Необов'язковий атрибут для підсумовування
template-help-param-filter-sum-start = Початкове значення для підсумовування
template-help-filter-title = Робить великою першу літеру кожного слова в рядку.
template-help-filter-trim = Видаляє пробільні або вказані символи з початку та кінця рядка.
template-help-param-filter-trim-chars = Необов'язковий набір символів для видалення
template-help-filter-unique = Повертає лише унікальні елементи послідовності.
template-help-param-filter-unique-case_sensitive = Чи враховувати регістр для унікальності
template-help-param-filter-unique-attribute = Необов'язковий атрибут для визначення унікальності
template-help-param-filter-unit-symbol = Цільовий символ одиниці вимірювання
template-help-param-filter-unit-digits = Кількість значущих цифр
template-help-param-filter-unit-decimals = Кількість знаків після коми
template-help-param-filter-unit-units = Таблиця одиниць або перевизначення системи одиниць
template-help-filter-upper = Переводить усі символи рядка у верхній регістр.
template-help-test-boolean = Перевіряє, чи є значення логічним.
template-help-test-defined = Перевіряє, чи змінна визначена.
template-help-test-divisibleby = Перевіряє, чи ділиться число на інше без остачі.
template-help-param-test-divisibleby-num = Число, на яке перевіряється ділення без остачі
template-help-test-endingwith = Перевіряє, чи закінчується рядок вказаним суфіксом.
template-help-param-test-endingwith-suffix = Суфікс рядка для перевірки
template-help-test-eq = Перевіряє, чи значення дорівнює іншому значенню.
template-help-param-test-eq-other = Значення для порівняння на рівність
template-help-test-escaped = Перевіряє, чи рядок позначений як екранований або безпечний.
template-help-test-even = Перевіряє, чи є число парним.
template-help-test-false = Перевіряє, чи є значення суворо false.
template-help-test-filter = Перевіряє, чи існує фільтр із такою назвою.
template-help-test-float = Перевіряє, чи є число числом із плаваючою комою.
template-help-test-ge = Перевіряє, чи значення більше або дорівнює іншому.
template-help-param-test-ge-other = Значення для порівняння
template-help-test-gt = Перевіряє, чи значення строго більше за інше.
template-help-param-test-gt-other = Значення для порівняння
template-help-test-in = Перевіряє, чи присутнє значення в послідовності чи контейнері.
template-help-param-test-in-seq = Послідовність або контейнер для перевірки
template-help-test-integer = Перевіряє, чи є число цілим.
template-help-test-iterable = Перевіряє, чи можна ітерувати значення.
template-help-test-le = Перевіряє, чи значення менше або дорівнює іншому.
template-help-param-test-le-other = Значення для порівняння
template-help-test-lower = Перевіряє, чи всі символи в рядку малі.
template-help-test-lt = Перевіряє, чи значення строго менше за інше.
template-help-param-test-lt-other = Значення для порівняння
template-help-test-mapping = Перевіряє, чи є значення словником або відображенням.
template-help-test-ne = Перевіряє, чи значення не дорівнює іншому.
template-help-param-test-ne-other = Значення для порівняння
template-help-test-none = Перевіряє, чи є значення none або невизначеним.
template-help-test-number = Перевіряє, чи є значення цілим або дробовим числом.
template-help-test-odd = Перевіряє, чи є число непарним.
template-help-test-safe = Перевіряє, чи рядок позначений як безпечний HTML.
template-help-test-sameas = Перевіряє, чи два об'єкти посилаються на той самий екземпляр.
template-help-param-test-sameas-other = Об'єкт для порівняння ідентичності
template-help-test-sequence = Перевіряє, чи є значення послідовністю.
template-help-test-startingwith = Перевіряє, чи починається рядок вказаним префіксом.
template-help-param-test-startingwith-prefix = Префікс рядка для перевірки
template-help-test-string = Перевіряє, чи є значення рядком.
template-help-test-test = Перевіряє, чи існує тест із такою назвою.
template-help-test-true = Перевіряє, чи є значення суворо true.
template-help-test-undefined = Перевіряє, чи змінна є невизначеною.
template-help-test-upper = Перевіряє, чи всі символи в рядку великі.
template-help-function-debug = Виводить налагоджувальну інформацію про змінні шаблону.
template-help-function-dict = Створює словник з іменованих аргументів.
template-help-function-namespace = Створює мутабельний об'єкт простору імен.
template-help-param-function-namespace-kwargs = Початкові атрибути для простору імен
template-help-function-range = Генерує послідовність цілих чисел.
template-help-param-function-range-stop = Кінець числового діапазону (не включно)
template-help-param-function-range-step = Крок між числами діапазону

template-help-filter-d = Псевдонім для фільтра default.

template-help-param-filter-duration-pattern = Шаблон форматування тривалості або пресет ('short', 'long', 'H:mm')
template-help-param-filter-indent-width = Кількість пробілів або рядок для кожного рівня відступу
template-help-param-filter-map-attribute = Назва фільтра або атрибута для застосування до кожного елемента
template-help-param-filter-plural-forms = Позиційні форми або ключові слова zero/one/two/few/many/other
template-help-param-filter-round-method = Спосіб округлення: 'common', 'ceil' або 'floor'
template-help-param-filter-split-separator = Необов'язковий розділювач (за замовчуванням — пробіли)
template-help-param-function-dict-kwargs = Пари ключ-значення для створення словника
template-help-param-function-range-start = Початок числового діапазону (за замовчуванням 0)

# Formula functions and errors
error-formula-too-large = Розмір формули ({ $len } байтів) перевищує ліміт у { $max } байтів
error-formula-depth-exceeded = Рівень вкладеності формули перевищує ліміт у { $max }
error-formula-unexpected-token = Неочікуваний токен «{ $token }»
error-formula-unclosed-paren = Незакрита дужка у формулі
error-formula-unknown-function = Невідома функція «{ $name }»
error-formula-unknown-function-suggest = Невідома функція «{ $name }». Можливо, ви мали на увазі «{ $suggestion }»?
error-formula-wrong-arity = Функція «{ $name }» очікує { $expected } аргументів, отримано { $got }
error-formula-reserved-name = Назва змінної «{ $name }» зарезервована
error-formula-division-by-zero = Ділення на нуль у формулі
error-formula-domain-error = Помилка області визначення у функції «{ $name }»
error-formula-overflow = Арифметичне переповнення у формулі
error-formula-incompatible-quantities = Несумісні фізичні величини: неможливо поєднати { $a } та { $b }
error-formula-type-mismatch = Невідповідність типів: очікується { $expected }, отримано { $got }
error-formula-variable-type = Змінна «{ $name }» має нечисловий тип
error-formula-syntax = Некоректний синтаксис формули
error-formula-eval = Помилка обчислення формули: { $message }

formula-fn-abs = Абсолютне значення числа.
formula-fn-sign = Знак числа (-1, 0 або 1).
formula-fn-min = Мінімальне значення серед аргументів.
formula-fn-max = Максимальне значення серед аргументів.
formula-fn-clamp = Обмежує значення діапазоном [lo, hi].
formula-fn-round = Округлює до найближчого цілого або вказаної кількості знаків.
formula-fn-floor = Найбільше ціле число, менше або рівне x.
formula-fn-ceil = Найменше ціле число, більше або рівне x.
formula-fn-trunc = Ціла частина числа з відкиданням дробової частини.
formula-fn-frac = Дробова частина числа.
formula-fn-sqrt = Квадратний корінь невід'ємного числа.
formula-fn-cbrt = Кубічний корінь числа.
formula-fn-pow = Підносить x до степеня y.
formula-fn-exp = Експоненційна функція e^x.
formula-fn-ln = Натуральний логарифм (за основою e) додатного числа.
formula-fn-log = Логарифм із заданою основою (за замовчуванням e).
formula-fn-log10 = Десятковий логарифм (за основою 10) додатного числа.
formula-fn-log2 = Двійковий логарифм (за основою 2) додатного числа.
formula-fn-hypot = Евклідова відстань sqrt(a^2 + b^2 + ...).
formula-fn-sin = Синус кута в радіанах.
formula-fn-cos = Косинус кута в радіанах.
formula-fn-tan = Тангенс кута в радіанах.
formula-fn-asin = Арксинус у радіанах, область [-1, 1].
formula-fn-acos = Арккосинус у радіанах, область [-1, 1].
formula-fn-atan = Арктангенс у радіанах.
formula-fn-atan2 = Чотириквадрантний арктангенс у радіанах.
formula-fn-sinh = Гіперболічний синус.
formula-fn-cosh = Гіперболічний косинус.
formula-fn-tanh = Гіперболічний тангенс.
formula-fn-deg = Перетворює радіани на градуси.
formula-fn-rad = Перетворює градуси на радіани.
formula-fn-sum = Сума значень усіх аргументів.
formula-fn-avg = Середнє арифметичне аргументів.
formula-fn-median = Медіана значень аргументів.
formula-fn-lerp = Лінійна інтерполяція між a та b з коефіцієнтом t.
formula-fn-map = Масштабує число з діапазону [in_lo, in_hi] у [out_lo, out_hi].
formula-fn-if = Повертає a, якщо умова c істинна, інакше b.
formula-fn-mod = Остача від ділення за знаком дільника b.
formula-fn-gcd = Найбільший спільний дільник цілих чисел.
formula-fn-lcm = Найменше спільне кратне цілих чисел.
formula-fn-between = Повертає true, якщо lo <= x <= hi.
formula-fn-isnan = Повертає true, якщо x є NaN.
formula-fn-isfinite = Повертає true, якщо x є скінченним числом.

formula-const-pi = Число пі (3.14159...)
formula-const-e = Число Ейлера e (2.71828...)
formula-const-tau = Число тау, 2*pi (6.28318...)
formula-const-inf = Позитивна нескінченність
formula-const-nan = Не число (NaN)
formula-const-true = Булеве значення «істина»
formula-const-false = Булеве значення «хибність»

formula-error = Рядок { $line }, поз. { $column }: { $message }
formula-render-error = Помилка обчислення: { $message }
formula-preview-empty = (порожня формула)
formula-preview-waiting = Попередній перегляд з'явиться після отримання значень на входах
formula-preview-samples = Випробувати
formula-preview-sample-badge = приклад
formula-preview-samples-title = Зразкові значення
formula-preview-samples-clear = Очистити
formula-preview-no-waiting-inputs = Усі входи мають значення
formula-lint-unknown-function = Невідома функція «{ $name }»
formula-lint-unknown-function-suggest = Невідома функція «{ $name }». Можливо, ви мали на увазі «{ $suggestion }»?
formula-lint-wrong-arity = Функція «{ $name }» очікує { $expected } аргументів, отримано { $got }


inline-plugin-value-tooltip = { $name } ({ $path }): { $value }
inline-binding-unavailable = Недоступно
inline-binding-type-error = Неможливо прив'язати { $source } до { $target }

wire-detached-title = Дріт від'єднано: пін змінився
node-menu-label = Дії з нодою
node-menu-duplicate = Дублювати
node-menu-copy = Копіювати
node-menu-delete =
    { $count ->
        [one] Видалити { $count } ноду
        [few] Видалити { $count } ноди
       *[other] Видалити { $count } нод
    }
toolbar-help = Керування канвою
sheet-rename = Перейменувати аркуш
sheet-title-label = Назва аркуша
sheet-title-empty = Назва не може бути порожньою
