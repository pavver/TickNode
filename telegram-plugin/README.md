# Telegram plugin

A bot integration using the [Telegram Bot API](https://core.telegram.org/bots/api).
All networking is performed by host HTTP tasks.
The component requests HTTP access to `api.telegram.org` and its own storage directory.

Build with `cargo build --target wasm32-wasip2 -p telegram-plugin`; install `telegram_plugin.wasm` like other TickNode plugins.
It is included in `dev.sh` and the Home Assistant add-on image.

Set the secret **Bot token** from @BotFather.
An empty token is rejected immediately; `getMe` verifies credentials asynchronously and logs the bot username.
A 401 stops polling and shows a translated settings error.
Save settings to retry.
**Allowed chats** accepts optional comma-separated signed decimal IDs and restricts both triggers and actions.
**Directory retention (days)** defaults to 30; 0 disables expiry.
Bot privacy settings in @BotFather and Telegram chat permissions still control which updates reach the bot.

Only one `getUpdates` request is pending, with a 50-second Telegram timeout and a 60-second HTTP timeout.
Failures back off from 1 to 30 seconds; a 409 log explains webhook and competing-poller conflicts.
The plugin does not remove a webhook automatically.
`offset.json` is atomically replaced before the next poll acknowledges updates.
This prevents ordinary restart replays; pending in-memory trigger events may be lost on a crash.
It is not an exactly-once execution guarantee.

## Nodes

| Node | Behavior |
| --- | --- |
| New message | Optional chat/topic/user filters, any/equals/contains/starts-with/regex text matching, optional edited messages; handles reply-keyboard button messages |
| Command | Validated `status` or `/status`; accepts `/status@BotName`; returns raw Arguments and quote-aware Args |
| Button pressed | Callback data equality/prefix matching; automatically answers callbacks, with optional toast text |
| Send message | Chat picker or typed input, optional Topic and Reply to, required Message (plain text, composed message, or media), link preview setting |
| Edit message | Chat, Message id and required Message (plain text or composed with optional inline keyboard), with keep_buttons choice |
| Delete message | Chat and Message id |
| Download media | Downloads a Telegram file into local blob storage; optional Kind selects a typed output |
| Button | Builds a `telegram.button` value from Text and action (`callback`, `url`, `text`, `contact`, `location`) |
| Keyboard | Combines button or String inputs into `telegram.keyboard` rows by columns for inline, reply, or remove layouts |
| Compose message | Combines Text, Media, and Keyboard into an outgoing `telegram.outgoing` message value |

Triggers cost 2.
Each node has a 128-event FIFO queue, emits at most one pulse per tick and logs overflow when dropping the oldest event.
Missing optional values clear their output pins.
Callbacks are acknowledged even if no trigger matches; when multiple triggers match, the first registered node in node-ID order supplies the toast text.
Auxiliary requests have a bounded queue and leave HTTP slots available for polling.

Actions cost 50 and start node-owned HTTP tasks; removing a node cancels its request or retry delay.
A node accepts one pending action.
HTTP results arrive through the node's task callback; errors report translated node messages and fire Failed with English diagnostic text on Error.
A 429 schedules a node-owned delay for `retry_after` before resending the original JSON or multipart request (up to five retries).
Credential changes invalidate pending actions.
Text outside 1–4096 Unicode characters fails without truncation.
Telegram can also reject formatted text or markup.

Send message accepts plain text (typed directly on the node or wired from a template), a structured `telegram.outgoing` value from Compose message, or a bare `media.*` object on its required Message pin.
Without media, the node calls `sendMessage` with text, optional parse mode, silent notification flag, and reply markup generated from the attached keyboard.
With media, the node calls `sendPhoto`, `sendAudio`, `sendVideo`, or `sendDocument` according to the media type.
When media is present, message text is sent as a caption (up to 1024 characters).
Media sources prefer `file_id`, then `blob`, then `url`.
The `file_id` and URL paths use JSON; blob uploads use multipart form-data with the same parameters, including caption, parse mode, keyboard markup, reply parameters, topic and notification flag.
Keyboard markup and reply parameters are encoded as JSON strings in multipart text fields.
File name comes from media `file_name`, then blob metadata, then a default based on media kind and MIME type (for example `photo.jpg` or `file.bin`).
Content type comes from media `mime_type`, then blob metadata.
Blob metadata is checked before upload: photos may be up to 10 MB, audio/video/documents up to 50 MB.
Oversized or evicted blobs fail with a translated error without starting a request.
For a camera snapshot, connect Network **Download media** (`media/fetch`) Done to Send message In and its Media to Message, then choose Chat.
Disable link preview configures the `link_preview_options` property on plain text messages.

Edit message replaces a message's text via `editMessageText`.
Its Message pin accepts plain text or a `telegram.outgoing` value with text, optional parse mode, and an optional inline keyboard.
Editing media or supplying reply/remove keyboards is rejected with a translated error.
When an edited message has no keyboard, the `keep_buttons` property governs button behavior: `yes` preserves the message's existing buttons, while `no` sends an empty inline keyboard to remove them.

Pressing a reply-keyboard button sends an ordinary text message to the chat.
Configure a **New message** trigger with text mode `equals` to handle the button label.

For a `/status` reply, connect Command Received to Send message In and Command Chat to Send message Chat.
Leave the send Chat property empty.
Feed data into a **Text from template** node and connect its Text to Send message Message.
Break the command's Message struct and connect its ID to Reply to.
A known reply topic is inherited when Topic is empty; an explicit Topic takes precedence.
The bounded live message-to-topic cache is populated from incoming and sent messages and quoted replies; it is not persisted.
A reply to a known topic creation message can also infer the topic from the persisted directory.

## Downloading Telegram media

**Download media** (`telegram/download`, category Telegram, cost 50) accepts any of the four `media.*` types on its required Media input (1).
In (0) starts a node-owned `getFile` lookup using `file_id`, followed by a GET of the returned Telegram file path with `save_to_blob: true`.
Files reported by `getFile` as larger than 20 MB fail before the download starts.
The credential-bearing file URL stays inside the HTTP request; it is never added to media outputs, traces, logs or errors.
Transport failures use generic diagnostics, and API error descriptions redact the token before being reported.
A 429 waits for `retry_after` and retries the same request up to five times.
Settings changes invalidate pending work, and removing the node cancels its task.

Done (10) signals success and Media (13) carries the original media type and all input fields, replacing/adding `blob` and filling missing `size` and `mime_type` from the blob response.
Failed (11) signals a translated node error with English diagnostic text on Error (12) and clears the previous Media output.
A media value with a blob but no `file_id` passes through unchanged without a network request.
A URL-only value fails with guidance to use Network **Download media** (`media/fetch`).
The persisted **Kind** selector defaults to `auto` (Media output type Any); `photo`, `audio`, `video` or `document` types the output as the corresponding `media.*` without changing pin IDs.
An input that does not match an explicit Kind fails instead of emitting a value with the wrong output contract.

To send an incoming photo elsewhere, connect New message Message → Break object (`telegram.message`) Value → Photo → Telegram Download media Media.
Connect New message Received → Download media In, then Download media Done and Media to the destination action's execution and media inputs.
Choose Kind `photo` when the next action requires a typed photo pin; the same chain works for Audio, Video or File.
For forwarding to Telegram, connect Done → Send message In and Media → Send message Message and select the destination Chat.
The original `file_id` is preserved, so forwarding to Telegram still uses its preferred existing-file path.

## Outgoing messages and builders

`telegram/button`, `telegram/keyboard`, and `telegram/compose` are pure builder nodes (`pure: true`, cost 1) that construct structured message values without side effects.
They prepare values that are consumed by Send message and Edit message.

`telegram/button` outputs a `telegram.button` struct with `text`, `action`, and optional `data`.
Action supports `callback`, `url`, `text`, `contact`, and `location`.
If data is omitted for callback buttons, the button text is used as callback data.
If the URL action is selected without a valid URL, the node returns no output and reports a translated error.

`telegram/keyboard` outputs a `telegram.keyboard` struct with `kind` (`inline`, `reply`, `remove`) and nested button `rows`.
The node provides `count` (1–24) button inputs and arranges non-empty inputs into rows according to `columns` (1–8).
Each input accepts either a `telegram.button` struct or a plain String.
String inputs automatically convert to callback buttons for inline keyboards and text buttons for reply keyboards.
Unconnected and empty inputs are skipped.
Inline keyboards reject text, contact, and location buttons, while reply keyboards reject callback and URL buttons.
When set to `remove`, all button inputs are ignored and a remove keyboard object is emitted.
When set to `reply`, the node exposes `resize` (default yes), `one_time` (default no), `placeholder`, and `persistent` (default no) properties.

`telegram/compose` outputs a `telegram.outgoing` struct combining optional Text, optional Media, and an optional `telegram.keyboard`.
The node validates text length: 1–4096 characters without media, or a caption up to 1024 characters with media.
A message requires either text or media; an empty message emits no output and reports a translated error.
Media must be a valid `media.*` object with `file_id`, `blob`, or `url`.
`parse_mode` selects Plain text (omitted), HTML, or MarkdownV2 formatting.
`silent` allows muting notifications.
Fields that are not provided remain absent in the emitted struct.

The pure model `outgoing::parse_outgoing` converts a plain String, bare `media.*` object, or `telegram.outgoing` struct into an internal `Outgoing` representation.
`outgoing::keyboard_markup` transforms a `Keyboard` struct into standard Telegram `InlineKeyboardMarkup`, `ReplyKeyboardMarkup`, or `ReplyKeyboardRemove` JSON.

## Directory and identifiers

`telegram.chat_id`, `telegram.user_id`, `telegram.message_id` and `telegram.topic_id` are opaque signed integer types.
Message IDs have no labels.
`telegram.user` and `telegram.message` are typed structs with optional fields and stable positional definitions.
Received messages expose media references via optional `photo`, `audio`, `video`, and `document` fields on `telegram.message`.
Photos select the highest resolution `PhotoSize` entry.
Captions on incoming media messages populate the `text` field when message text is omitted.

Chats, users and topics are learned from all updates, including chat membership, callback senders, topic creation/editing and quoted topic creation service messages.
Current names replace stored names immediately.
Historical quoted messages only fill unknown records, so they cannot undo a later rename.
Options are republished on name changes.
Chat and user usernames appear in option details.
Icons distinguish private users/chats, groups and topics.

`directory.json` stores names, metadata and `last_seen` as a UTC date.
The first sighting of each UTC day updates the date; messages later that day do not dirty storage.
Changes are debounced for approximately five seconds and atomically replaced through a temporary file and rename.
Entries older than the configured retention are removed on configure and daily; each kind is capped at 10,000 entries, evicting the least recently seen.
A corrupt file fails configuration without overwriting it.

Topic IDs are unique only within their chat.
Internally, topics use `(chat, topic)` keys, persisted as `"chat:topic"`.
Wire values remain Telegram's raw topic integer, so actions can send it directly.
The context-free identifier-label API only accepts a single integer: for collisions, the plugin publishes one decimal option whose label combines all `Chat › Topic` names with ` / `.
Select a **Chat filter together with a Topic filter** to disambiguate.
Message and topic IDs must always be interpreted alongside Chat; no synthetic or hashed IDs are sent to Telegram.

`getChat` is only queued when sending to an unknown, unnamed chat, never as background directory discovery.
`lookups.json` atomically records the attempted UTC day, so even failed lookups are limited to once per day across restarts.
A storage failure skips the lookup.

## Tests

```bash
cargo test -p telegram-plugin
```

The host-target tests inject an in-memory filesystem, clock and scripted Telegram transport, requiring no sockets or bot token.

The engine integration test requires local sockets and a separate test-only WASM fixture.
The production component ignores `test_api_url`; only the `test-api` feature accepts that setting directly from the test host (it is not exposed in the settings form).

```bash
cargo build --target wasm32-wasip2 -p telegram-plugin --features test-api
cp target/wasm32-wasip2/debug/telegram_plugin.wasm target/wasm32-wasip2/debug/telegram_plugin_test.wasm
cargo build --target wasm32-wasip2 -p telegram-plugin
TICKNODE_REQUIRE_PLUGINS=1 cargo test -p ticknode-core --test plugins telegram_plugin_
```

The integration test exercises real WASI storage, plugin-owned getMe/getUpdates, trigger emissions, typed Chat wiring, node-owned sendMessage and offset loading after disable/enable.
It also verifies blob uploads as real multipart requests reaching the fake server.
It skips when its fixture is absent, following the other component tests' convention.
Do not bundle the test fixture or enable `test-api` in production builds.
