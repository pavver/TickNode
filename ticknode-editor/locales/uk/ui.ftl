# Connection status and editor header
status-online = підключено · { $count } нод
status-connecting = підключення…
status-offline = немає з'єднання з сервером

toolbar-undo = Скасувати (Ctrl/Cmd+Z)
toolbar-redo = Повторити (Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y)
toolbar-palette = Палітра нод
toolbar-plugins = Плагіни
toolbar-fit = Показати весь граф (F)
toolbar-console = Консоль
toolbar-theme = Світла / темна тема
toolbar-language = Мова: Українська / English

# Помилки для користувача (ключі `UiMessage` від сервера; аргументи — рядки)
error-cycle-detected = Циклічне посилання: нода { $node } вже залежить від { $dependency }
error-type-mismatch = Несумісні типи даних: не можна з'єднати { $src } з { $dst }
error-invalid-pin-connection = Некоректне з'єднання пінів: один або обидва піни не існують або мають невірний напрямок чи тип
error-node-not-found = Ноду { $node } не знайдено
error-unknown-node-type = Невідомий тип ноди: { $type }
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
hint-duration = 1h 30m / 45s
hint-color = #rrggbb
hint-default = —

parse-error-number = «{ $value }» — не число
parse-error-bool = «{ $value }» — не true/false
parse-error-time = «{ $value }» — не час (ГГ:ХХ або ГГ:ХХ:СС)
parse-error-datetime = «{ $value }» — не дата-час (РРРР-ММ-ДД ГГ:ХХ)
parse-error-duration = «{ $value }» — не тривалість (90, 1h 30m, 500ms)
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
prop-field-type = Тип поля
prop-fields-list = Поля (name:type, …)
prop-interval-ticks = Інтервал, тактів
prop-all-matches = Усі збіги
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
