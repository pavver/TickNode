# TickNode

**TickNode** — це високоефективна система візуального програмування на основі нодів, спеціально розроблена для задач автоматизації (Smart Home, IoT, Industrial).

## Ключові особливості
- **Тактове виконання (Tick-based)**: Запобігає зависанням та нескінченним циклам.
- **Розділення Flow & Data**: Execution Flow (Push) для логіки та Data Flow (Pull) для значень.
- **Чисті ноди (Pure Nodes)**: Рекурсивне обчислення складних формул без затримок.
- **Розумні типи**: Автоматична нормалізація чисел (Int/Float), типи для часу, тривалості та кольору.
- **Безпека**: Архітектурна перевірка на цикли в даних під час з'єднання.
- **Hot-Reloading**: Зміна логіки графа без зупинки системи.

## Технологічний стек
- **Core/Backend**: Rust 2024.
- **Frontend**: Leptos (Rust → WASM), збирається `trunk`.
- **Серіалізація**: `postcard` (бінарний протокол).
- **Математичний рушій**: `evalexpr` для текстових формул.
- **Час**: `chrono`.

## Структура проекту
- `ticknode-shared`: Спільні типи, структури нод та протокол.
- `ticknode-core`: Логічне ядро (Tick Engine) та стандартна бібліотека нод.
- `ticknode-server`: сервер, що виконує граф, зберігає його в `data/graph.json` і роздає редактор (WebSocket `/ws`).
- `ticknode-editor`: веб-редактор на Leptos.

## Як запустити тести
```bash
cargo test
```

## Як запустити
```bash
(cd ticknode-editor && trunk build --release)
cargo run -p ticknode-server   # http://localhost:8099
```

## Ліцензія

[Mozilla Public License 2.0](LICENSE) (`MPL-2.0`). Змінені файли самого TickNode мають лишатися під MPL-2.0 і бути доступними у вихідному коді, якщо ви їх розповсюджуєте. Плагіни, написані на `ticknode-plugin-sdk`, — окремі твори: їх можна випускати під будь-якою ліцензією, зокрема закритою.

Шрифт редактора Cascadia Code (`ticknode-editor/assets/CascadiaCode.ttf`) — © Microsoft Corporation, під [SIL Open Font License 1.1](ticknode-editor/assets/CascadiaCode-OFL.txt).

## Участь у розробці

Див. [CONTRIBUTING.md](CONTRIBUTING.md). Pull request-и приймаються лише на його умовах, які дозволяють мейнтейнеру змінювати ліцензію внесків.
