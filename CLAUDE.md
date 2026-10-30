# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

TickNode — система візуального програмування на нодах для автоматизації. Ціль — add-on для Home Assistant, що замінює його вбудовані автоматизації. Rust 2024 workspace. Документація та коментарі в коді — українською. Концептуальний опис — у `README.md` та `ARCHITECTURE.md`.

**Напрям (у процесі міграції):** рушій працює на сервері (`ticknode-server`), браузер — лише редактор. Редактор — `ticknode-editor` на **Leptos** (CSR). Плагіни — WASM-компоненти (**wasmtime + WIT**, `wit/plugin.wit`). Протокол редактор↔сервер — postcard; типи спільні з `ticknode-shared`.

**Інтеграції (зокрема Home Assistant) — окремими плагінами, а не в ядрі чи сервері:** TickNode має запускатися й самостійно, не лише як HA add-on.

## Команди

```bash
cargo build
cargo test -p ticknode-core -p ticknode-shared -p ticknode-server
cargo test -p ticknode-core --test plugins delay_queue -- --nocapture   # один тест

# Плагіни — компоненти під wasm32-wasip2. Інтеграційні тести (ticknode-core/tests/plugins.rs) читають
# target/wasm32-wasip2/debug/*.wasm і ПРОПУСКАЮТЬСЯ, якщо файлів нема (TICKNODE_REQUIRE_PLUGINS=1 — падають)
cargo build --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p test-plugin
# Для сервера — release-збірка, скопійована в data/plugins/
cargo build --release --target wasm32-wasip2 -p math-plugin && cp target/wasm32-wasip2/release/math_plugin.wasm data/plugins/

# Сервер (env: TICKNODE_BIND=0.0.0.0:8099, TICKNODE_DATA_DIR=data, TICKNODE_WEB_DIR=ticknode-editor/dist, TICKNODE_TICK_MS=100, RUST_LOG)
cargo run -p ticknode-server

# Редактор (Leptos CSR, trunk). Не входить у default-members — `cargo build` у корені його не збирає
cd ticknode-editor && trunk build            # → dist/, який роздає сервер
cd ticknode-editor && trunk serve            # dev на :8080, /ws проксюється на 127.0.0.1:8099
cd ticknode-editor && trunk build --release  # потрібен wasm-opt (trunk завантажує сам)
cargo test -p ticknode-editor --target x86_64-unknown-linux-gnu   # тести геометрії (без браузера)
```

Версія `wasm-bindgen` у `ticknode-editor/Cargo.toml` мусить збігатися з wasm-bindgen-cli, який використовує trunk.

У dev-профілі залежності зібрані з `opt-level = 2`: неоптимізований Cranelift компілює плагіни в десятки разів повільніше.

## Архітектура

### Крейти
- `ticknode-shared` — усі серіалізовані типи: `DataValue`/`DataType`/`NumberValue`, власні типи плагінів (`types.rs`: `TypeDefinition`, `TypeRegistry`), `NodeDefinition`/`PinDefinition`, `NodeOutput`, `Capability`/`PluginManifest`, модель графа (`GraphModel`, `NodeModel`, `NodeInstance`, `ConnectionModel`, `Position`) і протокол редактора (`ClientMessage`, `ServerMessage`).
- `ticknode-core` — рушій: `lib.rs` (трейт `Node`, `NodeRegistry`, `TickEngine`, вбудовані ноди), `host.rs` (`HostState`: задачі, HTTP, логи), `plugin.rs` (wasmtime-хост плагінів, `WasmNode`, `TickEngine::load_plugin`).
- `ticknode-server` — axum: `/ws` (WebSocket, postcard) + статика редактора. `actor.rs` — рушій в окремому OS-потоці (такти й команди послідовно, без блокувань); `storage.rs` — `graph.json`; `plugins.rs` — пошук `data/plugins/*.wasm`, стан плагінів і `data/plugin-config/`; `ws.rs` — сесія редактора.
- `ticknode-editor` — редактор: `state.rs` (`Editor` — Copy-набір сигналів, `apply(ServerMessage)`), `connection.rs` (WebSocket + перепідключення), `canvas.rs` (камера, перетягування, дроти), `panels.rs` (палітра, інспектор, консоль), `plugins_panel.rs` (панель плагінів: стан, перемикач, права, форма налаштувань), `inline.rs` (властивості й значення входів на ноді), `match_editor.rs` (входи й випадки Match на ноді), `ui.rs` (компоненти дизайн-системи), `geometry.rs`.
- `ticknode-plugin-sdk` — SDK плагінів (wit-bindgen): трейти `Plugin` (з `types()`)/`PluginNode`, `host::*`, `export_plugin!`, `StructValue` (будівник тегованого значення), `Inputs::field*`/`field*()` (читання полів). `*-plugin` — плагіни; `test-plugin` — лише для тестів хоста (аварії, пальне, сховище).

### Ноди та граф
- **Власні типи даних плагінів.** `DataType::Named("ha.state")` посилається на `TypeDefinition` (`title`, `color` `#rrggbb`, `TypeKind::Struct{fields: FieldDefinition{name,title,description,data_type,optional}}` | `TypeKind::Enum{values}`); назва — з крапкою (`is_type_name`), текстом (`parse_data_type`/`data_type_name`, `*_type` властивості, входи Match) вона пишеться як є. Значення struct — `DataValue::Object` з `type_name: Some(..)` (`get_type()` → `Named`; `#[serde(default)]`, без `skip_serializing_if` — postcard не самоописовий), enum — `DataValue::String` з переліку (валідується `text::parse_value_in` / `input_value_fits`). Реєстр — `TickEngine::types` (`TypeRegistry`; плагін додає типи атомарно в `load_plugin`/`register_types`; конфлікт назв, невідомий тип у полі чи піні — помилка завантаження). Сумісність пінів — лише `TypeRegistry::compatible` (вільна `types_compatible` знає тільки Any): `Named(a)→Named(a)`, `Named↔Any`, `Named(struct)→Object`, `Named(enum)→String`; зворотні (`Object→Named`, `String→Named`) — ні. Для кожної struct рушій сам додає чисті ноди `type/make/<назва>` (поля → значення, пін 0 — вихід, поля 1..=N) і `type/break/<назва>` (пін 0 — вхід, поля 1..=N; відсутнє необов'язкове поле — без значення), категорія `Data` (`custom_types.rs`). Нові поля структури додавайте в кінець — піни стабільні. Match: struct поводиться як Object (`condition::ops_in`), enum — `=`/`≠`/`є значення` з випадним списком значень. Нода плагіна не може видати на `Named`-вихід об'єкт з чужим/відсутнім тегом — хост відкидає значення й пише помилку в лог ноди.
- **Обов'язкові входи.** `PinDefinition::required` (`serde(default)`; WIT `pin.required`; SDK `data_in(..).required()`). Вхід заданий, якщо під'єднано джерело або (для текстових типів і enum) задано значення на ноді (`in:<пін>`); struct/list — лише з'єднання. Нода з незаданим обов'язковим входом не виконується (чиста не дає значення; активна не викликає `execute`), у лог ноди пишеться «Не задано обов'язковий вхід» (один раз, доки стан не зміниться). Джерело без значення — мовчазний пропуск. У Make обов'язкові входи — це поля без `optional`. Редактор підсвічує такий пін і ноду як Error (`Editor::required_missing`).
- Нода відтворюється з `type_name` + `properties()`: це єдиний механізм збереження/копіювання (`export_graph`, `load_graph`, `export_snippet`, `import_snippet`). Кожна налаштовувана нода мусить реалізувати і `set_property`, і `properties()` симетрично.
- `NodeRegistry::register(creator)` бере дефініцію з екземпляра з дефолтними властивостями — дефініцію в реєстрі окремо не описують.
- Піни можуть залежати від властивостей (`math/expression` виводить вхідні піни зі змінних формули; `list/*`, `object/*` — тип з `element_type`/`field_type`). `set_node_property` прибирає з'єднання, що стали невалідними, і повертає їх у `RemovedConnections`.
- Вхідний пін даних має одне джерело: `connect_data` замінює попереднє з'єднання. `connect_exec` теж валідує піни.
- Непід'єднаному входу даних (number/string/bool/any) можна задати значення прямо на ноді: `ClientMessage::SetInputValue`, у рушії — `TickEngine::set_input_value`. Зберігається серед властивостей як `in:<id піна>` (`input_value_property`), але ноді не передається: `pull_inputs` підставляє його, поки до піна нічого не під'єднано. Значення, що перестали підходити після зміни пінів, прибираються в `set_node_property`. Працює й для плагінів.
- `match` — комбінатор-вирішувач: властивості `inputs` — список `"<пін>:<ім'я>:<тип>"` (тип `any` = «auto») і `cases` — список рядків `"<пін>: a > 10 and b != \"on\""` (розбір, обчислення й показ — `ticknode_shared::condition`; «and» зв'язує сильніше за «or»). Оператори залежать від типу лівого входу (`ops_for`: порівняння для чисел/часу/тривалості, `contains`/`starts_with`/`len_*` для рядків і списків, `has_field` для об'єктів, унарні `is_true`, `is_empty`, `has_value`…); праворуч — стала або інший вхід, стала приводиться до типу входу (`text::coerce_to`: `"07:30"` → час). Граф без `inputs` — старий формат: входи виводяться з імен в умовах (Any, піни 1.. за абеткою), але лише під час `create_node`; після `Node::restored` входи явні — і в новій ноді, і в завантаженій. Кожен випадок — exec-вихід зі стабільним номером піна, Else — пін 0. Окремої ноди Branch немає.
- Текстова форма значень (поля на ноді, умови) — `ticknode_shared::text`: `parse_value(text, &DataType)` / `format_value`, час `HH:MM[:SS]`, дата-час `YYYY-MM-DD HH:MM`, тривалість `90` / `1h 30m` / `500ms`, колір `#rrggbb[aa]`.
- Позиції нод — справа сервера (не рушія); вони пишуться в `NodeModel.position` лише при збереженні.

### Збереження
`data/graph.json` — відформатований JSON зі стабільним порядком (ноди/з'єднання відсортовані, поля об'єктів — `BTreeMap`), щоб файл нормально лягав у git. Запис атомарний (tmp + rename), з дебаунсом ~1 с і при SIGTERM. Пошкоджений файл — помилка старту (не перезаписується порожнім графом). Плагіни — `data/plugins/*.wasm`, їхні файли — `data/plugin-data/<назва>/`, налаштування й стан — `data/plugin-config/<назва>.json` (усе поза git; у конфігах секрети).

`data/plugin-config/<назва>.json` — `{"enabled": bool, "settings": {ключ: DataValue}}`; запис атомарний (tmp + rename), права файлу 0600 (каталог 0700). Плагін без файлу — увімкнений з налаштуваннями за замовчуванням. Пошкоджений файл дає стан `Error` і не перезаписується мовчки (лише явним збереженням налаштувань чи вмиканням).

### Протокол редактора
Після підключення сервер шле `ServerMessage::Init` (дефініції + весь граф), далі — події-зміни всім редакторам (`NodeAdded`, `NodeUpdated`, `DataConnected`, …) і телеметрію (`Executed` щотакту, `Log` від Console/Debug). Сервер — джерело істини: редактор надсилає `ClientMessage` і застосовує зміни лише з подій сервера (події можуть дублюватись — обробляти ідемпотентно). Помилки йдуть лише клієнту-ініціатору (`Error`).

Плагіни: `Init` містить `plugins: Vec<PluginInfo>`; зміну стану чи налаштувань усім редакторам повідомляє `PluginUpdated(PluginInfo)`, а зміну набору нод/типів (вмикання, вимкнення) — `Catalog { definitions, types }` + `NodeUpdated` для перетворених нод (і `*Disconnected` для з'єднань, що стали невалідними). Клієнт шле `SetPluginEnabled { name, enabled }` і `ConfigurePlugin { name, values: [(ключ, SettingUpdate)] }`, де `SettingUpdate` — `Set(значення)` / `Keep` / `Clear`. **Секрети (`SettingKind::Secret`) у редактор не йдуть**: `PluginInfo.values` має для них лише `Secret { is_set }`, а редактор надсилає `Set` (нове значення), `Keep` або `Clear`. `NodeInstance.unavailable` — пояснення для ноди-заглушки.

### Редактор
- Дизайн — Operator UI з `/home/pavver/rust/notifiApp/NotifiApp/design_system/` (Kotlin-компоненти — еталон): токени в `style.css` (`:root` / `[data-theme=light]`), одиниця `--du: 4px`, кути 0, стани контролів Idle/Selected/Pending/Success/Error (`ui::ControlState`).
- Дроти будуються з обчислених позицій пінів (`geometry.rs`), а не вимірюються в DOM: константи `DU`, `NODE_WIDTH` / `MATCH_WIDTH` (ширина — `NodeLayout::width`), висоти заголовка/рядка мусять збігатися з CSS `.node-header`, `.node-props`, `.pins`, `.pin`. Позиції пінів і розмір ноди дає `NodeLayout`: прості властивості (типи, що набираються текстом, крім `in:*`) — рядки між заголовком і пінами, а в Match після In, рядків входів і рядка «+ вхід» ідуть блоки випадків (`case_rows`), і exec-вихід кожного стоїть у заголовку свого блоку, Else — в останньому рядку.
- Усі налаштування — на самій ноді (властивості, типи `*_type` випадним списком, значення входів, умови Match, ▶ «запустити» в заголовку); інспектор показує опис, останнє виконання й дії. Елементи керування на ноді мусять не пускати `pointerdown` до полотна (`stop_on_controls`), інакше почнеться перетягування.
- Динамічний `class=move || …` перезаписує весь `className`, тож не поєднувати його з `class:x=…` на тому ж елементі — збирати клас одним рядком.
- Поля, що можуть втратити фокус уже після того, як їх прибрали з DOM, читають сигнали через `try_*`. Сигнали нод (`ClientNode`) звільняються відкладено (`dispose_later`), а `Init` оновлює наявні ноди на місці.
- У `view!` вираз атрибута з `<` чи `>` треба брати в `{…}`, інакше макрос сприйме його як кінець тегу.
- Редактор не змінює граф локально: дія → `ClientMessage`, стан оновлюється лише з `ServerMessage` (виняток — позиція ноди під час перетягування).
- Нижче 900 px панелі стають оверлеями (`Editor::narrow`).
- Панель «Плагіни» (кнопка в тулбарі, `Editor::show_plugins`) займає місце інспектора. Картка плагіна: назва, версія/автор, чип стану (`ControlState`: Success — працює, Idle — вимкнено, Error — помилка з текстом; «помилка налаштувань» — окремий чип), перемикач увімкнення (`Switch` у Pending до відповіді сервера), список прав, розгортувана форма налаштувань за схемою й «Зберегти» → `ConfigurePlugin`; помилка `configure` — під формою. Поле секрету — пароль із плейсхолдером «задано» і кнопкою «Очистити»; порожнє поле = `Keep`. Нода-заглушка (`NodeInstance.unavailable`) на полотні приглушена, у заголовку «плагін недоступний», властивості й значення входів не редагуються (CSS `.node.unavailable`).
- Керування полотном: середня кнопка — зсув (на сенсорі — палець по фону), ЛКМ по фону — рамка виділення (зліва направо — «вікно», лише повністю охоплені ноди; справа наліво — «перетин»). Палітра: клік додає ноду, перетягування — дроп під курсор.
- Буфер обміну — події `copy`/`cut`/`paste` (працюють без HTTPS); фрагмент — JSON `{"format":"ticknode/snippet@1","graph":GraphModel}` (`clipboard.rs`). Ctrl+V не створює ноди одразу: фрагмент стає `Editor::pending_paste` і їде за курсором (ЛКМ — `ClientMessage::Paste`, ПКМ/Esc — скасувати; середня кнопка зсуває полотно). Подія `paste` одразу після середньої кнопки ігнорується — це вставка первинного виділення Linux. Вставка — `ClientMessage::Paste`, сервер дає нові ID і відповідає ініціатору `Created` (так само після `AddNode`), щоб виділити створене. `Created` іде після `NodeAdded`: актор шле відповідь після розсилки, а `ws.rs` (`select!` з `biased`) спершу віддає події.
- Інспектор показує останнє виконання ноди з `NodeTrace` (входи, виходи, спрацьовані exec-піни, «N секунд тому»). Сервер шле траси в `Executed` щотакту, а в `Init` — останні з віком у мс; зберігаються лише в пам'яті.

### Модель виконання (`TickEngine::tick`)
- **Exec flow = push**: `NodeOutput::Execution` ставить наступні ноди в чергу **наступного** такту, тому exec-цикли безпечні.
- **Data flow = pull**: перед `execute` ядро рекурсивно тягне значення входів. Чисті ноди (`is_pure()`) кешуються в `pure_cache`, який скидається щотакту і після кожного виконання, що змінило дані/змінні; результати активних нод живуть у `data_cache`.
- `NodeOutput::Variable` пише в глобальний `variables` (спільний для всього графа). Плагіни до змінних доступу не мають — лише через піни.
- Асинхронні задачі (затримки в тактах, HTTP у tokio `spawn_blocking`) — у `HostState::active_tasks`. На початку такту завершені задачі прибираються й доставляються ноді-власниці через `Node::on_task` (виходи обробляються так само, як у `execute`); далі `poll` джерел подій, далі черга. `remove_node` скасовує задачі ноди.
- `HostState::logs` збирає повідомлення нод за такт (зокрема помилки плагінів), `TickEngine::executed` — ноди, що виконались (сервер це розсилає).

### WASM-плагіни (`wit/plugin.wit`, `plugin.rs`)
- Плагін — компонент (`cdylib`, `wasm32-wasip2`). Експортує `manifest`, `definitions`, `types` (власні типи; хост читає їх разом з `definitions`), `create-node` і ресурс `node` (`definition`, `properties`, `set-property`, `execute`, `poll`). Кожна нода графа — окремий ресурс, тож стан живе в полях ноди. Імпортує `host`: `log`, `delay`, `start-http`, `cancel-task`, такт. `poll` хост викликає лише для нод з `polls: true` у дефініції (SDK: `PluginNode::polls`).
- WIT не має рекурсивних типів: `DataValue` передається як postcard-байти (`type value = list<u8>`), `DataType` — як `{base, list-depth}` (`base` — variant; власний тип — `named(string)`). Обидві сторони кодують через `ticknode-shared`.
- Асинхронна робота — задачі хоста (`HostState::start_delay`/`start_http`): плагін отримує `task-id`, результат приходить пізніше як `trigger::task` (у ядрі — `Node::on_task`). Задача належить ноді: скасовується при видаленні ноди, чужу скасувати не можна.
- Права: `Http` (домени перевіряє `start-http`; перенаправлень хост не виконує — 3xx отримує нода), `Storage` (WASI preopen каталогу плагіна як `.` — плагін користується `std::fs`). Інших WASI-ресурсів (мережа, env, args) немає. Сервер видає права, запитані в маніфесті.
- Один `Store` на плагін; кожен виклик обмежений пальним (`FUEL_PER_CALL`) і пам'яттю. Trap → екземпляр перестворюється, ноди ліниво відтворюються з `properties` (стан у полях втрачається), нода-винуватець зупиняється до зміни властивості. Повний backtrace — у лозі сервера, у консоль редактора — коротка причина.
- **Синхронний wasmtime-wasi не можна викликати з tokio-задачі** (`in_tokio` → `Handle::block_on` панікує). Тому рушій на сервері живе в окремому OS-потоці, а тести плагінів — звичайні `#[test]`; tokio-runtime для HTTP передається через `HostState::runtime`.
- `NodeDefinition::help` — детальне пояснення ноди (підказка «інфо» в палітрі й інспекторі); `description` — один рядок для палітри. Тест ядра вимагає `help` у кожної вбудованої ноди. Піни/кольори власних типів у редакторі: `Editor::types`, `geometry::type_color` (колір із `color`, інакше хеш назви), `Editor::type_title`; непід'єднаний вхід enum — випадний список (`InlineValue`).
- Категорія — рядок `NodeDefinition::category` англійською (`Events`, `Control Flow`, `Time`, `Data`, `Math`, `Logic`, `String`, `Lists`, `Objects`, `Network`, `Debug`, `Actions`, `Test`). Порядок, українська назва, колір та іконка — у `ticknode-editor/src/categories.rs`; невідомі категорії плагінів показуються як є.
- Плагін не може перехопити тип ноди, що вже зареєстрований (вбудований чи з іншого плагіна).
- **Налаштування плагіна.** Схема — `PluginManifest.settings: Vec<SettingDefinition { key, title, description, kind: SettingKind (Text/Number/Bool/Secret/Url/Select), required, default }>` (будівники: `SettingDefinition::text/secret/select(..).required().default_value(..)`, `PluginManifest::new(..).setting(..)`). Значення задає користувач у панелі «Плагіни»; хост передає їх експортом WIT `configure(settings: list<property>) -> result<_, string>` (значення — postcard `DataValue`; відсутні замінюються `default`, порожній рядок = не задано, див. `effective_settings`). Хост викликає `configure` після вмикання плагіна, при кожній зміні налаштувань і **після перестворення екземпляра через аварію — до відтворення нод** (`Runtime::instantiate`). `Err` не вимикає плагін: він лишається завантаженим, помилка показується в панелі (`PluginInfo.settings_error`, `TickEngine::plugin_settings_error`). SDK: `Plugin::configure(&[(String, DataValue)]) -> Result<(), String>` (типово `Ok`), хелпери `setting_str/number/bool`; екземпляр один на плагін, тож налаштування тримають у `thread_local!`.
- **Вмикання й вимкнення на льоту.** `TickEngine::add_plugin` лише компілює плагін і читає маніфест; `enable_plugin(name, settings)` реєструє типи нод (`NodeRegistry::register_owned`) і типи даних (`TypeRegistry`, власник — `type_owners`), викликає `configure` і перетворює заглушки його типів на справжні ноди (невалідні з'єднання прибираються й повертаються в `PluginChange.removed`); `disable_plugin(name)` замінює ноди плагіна (разом із Make/Break його структур) заглушками, знімає типи з реєстрів і звільняє екземпляр. Вимкнення відхиляється, якщо типи плагіна використовує інший увімкнений плагін (у пінах нод чи полях типів). `load_plugin` = `add_plugin` + `enable_plugin` з типовими налаштуваннями (для тестів).
- **Ноди-заглушки (`StubNode`).** Зберігають `type_name`, усі властивості (включно з `in:*`) та останню відому дефініцію, тож з'єднання лишаються, а `export_graph` дає той самий результат. Заглушка не виконується (у лог — «плагін <назва> недоступний»), `set_node_property`/`set_input_value` повертають `EngineError::PluginUnavailable`. **Невідомий тип у `load_graph`/`import_snippet` теж дає заглушку** (піни відновлюються з з'єднань графа, тип піна даних — з іншого кінця або `Any`), тому граф із нодою невідомого плагіна не губить її при збереженні. Коли тип з'являється в реєстрі, заглушка оживає.

<!-- imported-from: gemini:project:instructions -->
# Інструкції для проекту TickNode

Ви — CLI-агент, що працює над проектом TickNode.

## Основні правила:
1. **Мова**: Завжди спілкуватися українською мовою.
2. **Технології**: 
   - Rust для логіки та сервера.
   - Leptos (Rust → WASM) для редактора; WASM-компоненти (wasmtime + WIT) для **динамічних плагінів нод**.
   - `postcard` для протоколу редактор↔сервер; JSON для збереження графа на диск.
3. **Архітектура**: Дотримуватися Tick-based моделі виконання. Розділяти Execution Flow та Data Flow.
4. **Результати нод**: Використовувати систему `NodeOutput` для повернення результатів (Execution, Data, Variable).
5. **Математика**: Для складних формул використовувати `MathExpressionNode` з бібліотекою `evalexpr`.
6. **Ізоляція**: Всі сторонні ноди мають працювати через WASM-плагіни з обмеженими правами (Capabilities).
7. **Структура**: 
   - Спільні типи — у `ticknode-shared`.
   - Рушій — у `ticknode-core`.


## Робочий процес:
- При зміні формату графа чи протоколу — оновлювати типи в `ticknode-shared` (вони спільні для сервера й редактора).
- Для тестів використовувати `cargo test` у відповідних пакетах.
