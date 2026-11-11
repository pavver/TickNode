# Telegram plugin

A bot integration using the [Telegram Bot API](https://core.telegram.org/bots/api). All networking is performed by host HTTP tasks. The component requests HTTP access to `api.telegram.org` and its own storage directory.

Build with `cargo build --target wasm32-wasip2 -p telegram-plugin`; install `telegram_plugin.wasm` like other TickNode plugins. It is included in `dev.sh` and the Home Assistant add-on image.

Set the secret **Bot token** from @BotFather. An empty token is rejected immediately; `getMe` verifies credentials asynchronously and logs the bot username. A 401 stops polling and shows a translated settings error. Save settings to retry. **Allowed chats** accepts optional comma-separated signed decimal IDs and restricts both triggers and actions. **Directory retention (days)** defaults to 30; 0 disables expiry. Bot privacy settings in @BotFather and Telegram chat permissions still control which updates reach the bot.

Only one `getUpdates` request is pending, with a 50-second Telegram timeout and a 60-second HTTP timeout. Failures back off from 1 to 30 seconds; a 409 log explains webhook and competing-poller conflicts. The plugin does not remove a webhook automatically. `offset.json` is atomically replaced before the next poll acknowledges updates. This prevents ordinary restart replays; pending in-memory trigger events may be lost on a crash. It is not an exactly-once execution guarantee.

## Nodes

| Node | Behavior |
| --- | --- |
| New message | Optional chat/topic/user filters, any/equals/contains/starts-with/regex text matching, optional edited messages |
| Command | Validated `status` or `/status`; accepts `/status@BotName`; returns raw Arguments and quote-aware Args |
| Button pressed | Callback data equality/prefix matching; automatically answers callbacks, with optional toast text |
| Send message | Chat picker or typed input, optional Topic and Reply to, required Text, parse mode, silent sends, link preview setting, inline buttons |
| Edit message | Chat, Message id and Text, with optional parse mode |
| Delete message | Chat and Message id |

Triggers cost 2. Each node has a 128-event FIFO queue, emits at most one pulse per tick and logs overflow when dropping the oldest event. Missing optional values clear their output pins. Callbacks are acknowledged even if no trigger matches; when multiple triggers match, the first registered node in node-ID order supplies the toast text. Auxiliary requests have a bounded queue and leave HTTP slots available for polling.

Actions cost 50 and start node-owned HTTP tasks; removing a node cancels its request or retry delay. A node accepts one pending action. HTTP results arrive through the node's task callback; errors report translated node messages and fire Failed with English diagnostic text on Error. A 429 schedules a node-owned delay for `retry_after` before resending the original body (up to five retries). Credential changes invalidate pending actions. Text outside 1–4096 Unicode characters fails without truncation. Telegram can also reject formatted text or markup.

Buttons use one keyboard row per line and `|` between buttons:

```text
Refresh=status | Open=https://example.org
Turn on=on
```

Each button is `Label=callback_data` (1–64 UTF-8 bytes) or `Label=https://url`. There are at most eight buttons per row and 100 total. URL targets must have a nonempty authority and contain no whitespace.

For a `/status` reply, connect Command Received to Send message In and Command Chat to Send message Chat. Leave the send Chat property empty. Feed data into a **Format text** node and connect its Text to Send message Text. Break the command's Message struct and connect its ID to Reply to. A known reply topic is inherited when Topic is empty; an explicit Topic takes precedence. The bounded live message-to-topic cache is populated from incoming and sent messages and quoted replies; it is not persisted. A reply to a known topic creation message can also infer the topic from the persisted directory.

## Directory and identifiers

`telegram.chat_id`, `telegram.user_id`, `telegram.message_id` and `telegram.topic_id` are opaque signed integer types. Message IDs have no labels. `telegram.user` and `telegram.message` are typed structs with optional fields and stable positional definitions.

Chats, users and topics are learned from all updates, including chat membership, callback senders, topic creation/editing and quoted topic creation service messages. Current names replace stored names immediately. Historical quoted messages only fill unknown records, so they cannot undo a later rename. Options are republished on name changes. Chat and user usernames appear in option details. Icons distinguish private users/chats, groups and topics.

`directory.json` stores names, metadata and `last_seen` as a UTC date. The first sighting of each UTC day updates the date; messages later that day do not dirty storage. Changes are debounced for approximately five seconds and atomically replaced through a temporary file and rename. Entries older than the configured retention are removed on configure and daily; each kind is capped at 10,000 entries, evicting the least recently seen. A corrupt file fails configuration without overwriting it.

Topic IDs are unique only within their chat. Internally, topics use `(chat, topic)` keys, persisted as `"chat:topic"`. Wire values remain Telegram's raw topic integer, so actions can send it directly. The context-free identifier-label API only accepts a single integer: for collisions, the plugin publishes one decimal option whose label combines all `Chat › Topic` names with ` / `. Select a **Chat filter together with a Topic filter** to disambiguate. Message and topic IDs must always be interpreted alongside Chat; no synthetic or hashed IDs are sent to Telegram.

`getChat` is only queued when sending to an unknown, unnamed chat, never as background directory discovery. `lookups.json` atomically records the attempted UTC day, so even failed lookups are limited to once per day across restarts. A storage failure skips the lookup.

## Tests

```bash
cargo test -p telegram-plugin
```

The host-target tests inject an in-memory filesystem, clock and scripted Telegram transport, requiring no sockets or bot token.

The engine integration test requires local sockets and a separate test-only WASM fixture. The production component ignores `test_api_url`; only the `test-api` feature accepts that setting directly from the test host (it is not exposed in the settings form).

```bash
cargo build --target wasm32-wasip2 -p telegram-plugin --features test-api
cp target/wasm32-wasip2/debug/telegram_plugin.wasm target/wasm32-wasip2/debug/telegram_plugin_test.wasm
cargo build --target wasm32-wasip2 -p telegram-plugin
cargo test -p ticknode-core --test plugins telegram_plugin_message_to_send_end_to_end
```

The integration test exercises real WASI storage, plugin-owned getMe/getUpdates, trigger emissions, typed Chat wiring, node-owned sendMessage and offset loading after disable/enable. It skips when its fixture is absent, following the other component tests' convention. Do not bundle the test fixture or enable `test-api` in production builds.
