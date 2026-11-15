# Telegram plugin

Developer reference split out of [AGENTS.md](../../AGENTS.md).
Keep it current when the described code changes.

- `telegram-plugin` requests only `Http { api.telegram.org }` and `Storage`.
  Settings: required secret `token`, optional comma-separated `allowed_chats`, `directory_ttl_days` (30; 0 never expires).
  Configure rejects empty/invalid settings; asynchronous `getMe` logs the bot username.
  `host::node_error` from plugin-tick reports a namespaced settings error; the engine exposes changes through `take_plugin_settings_changes` and the actor broadcasts `PluginUpdated`.
  Configure clears the error.
  This uses the existing WIT call without changing its ABI.
- `getUpdates`: one plugin-owned request, Telegram timeout 50 s / HTTP timeout 60 s, four update kinds (message, edited_message, callback_query, my_chat_member), 1–30 s back-off, explicit webhook/competing-instance log for 409; 401 stops polling with a translated settings error.
  `offset.json` is atomically saved before the next poll acknowledges updates.
  Trigger queues hold 128 events per node, drop the oldest with an English log on overflow, and emit at most one pulse per tick.
  Callbacks are automatically answered with optional toast text.
- Nodes: `telegram/message`, `telegram/command`, `telegram/callback` (cost 2); `telegram/send`, `telegram/edit`, `telegram/delete`, `telegram/download` (cost 50).
  `telegram/button`, `telegram/keyboard`, and `telegram/compose` are pure builder nodes (`pure: true`, cost 1) in category `Telegram`.
  `telegram/button` builds a `telegram.button` struct from required Text, optional Data, and an `action` selector (`callback`, `url`, `text`, `contact`, `location`).
  Missing Data defaults to the button's Text for callback actions; missing URL for the URL action reports a translated error.
  `telegram/keyboard` arranges `count` (1–24) button or String inputs into rows according to `columns` (1–8).
  String inputs automatically adapt to callback buttons for inline keyboards and text buttons for reply keyboards; empty inputs are skipped.
  Inline keyboards reject text, contact, and location buttons, while reply keyboards reject callback and URL buttons.
  Reply keyboards expose `resize`, `one_time`, `placeholder`, and `persistent` properties, while remove keyboards ignore inputs and output a remove command.
  `telegram/compose` combines optional Text (1–4096 characters without media, or up to 1024-character caption with media), optional Media, and an optional `telegram.keyboard` into a `telegram.outgoing` message value with parse mode and silent options.
  Messages require either text or media.
  `outgoing::parse_outgoing` purely parses a `DataValue::String`, `telegram.outgoing` struct, or bare `media.*` value into an `Outgoing` model and generates Telegram `reply_markup` JSON via `keyboard_markup`.
  Incoming messages project received photo (largest size), audio, video, and document into optional `telegram.message` fields.
  Actions own their HTTP and 429 retry-delay tasks; results arrive through `Trigger::Task`.
  Chat uses a dynamic property with `.or_input(1)`.
  `telegram/send` accepts plain text, a `telegram.outgoing` message value, or a bare `media.*` value on required Message (type Any).
  Without media, Send sends text via `sendMessage` with optional parse mode, silent flag, and keyboard markup from `outgoing::keyboard_markup`.
  With media, Send routes photo, audio, video, or document via `sendPhoto`, `sendAudio`, `sendVideo`, or `sendDocument`, with caption up to 1024 characters, parse mode, and keyboard markup, preferring `file_id`, then `blob`, then `url`.
  Blob sends use host multipart with the same parameter list as JSON sends (including caption, parse mode, reply markup, topic, reply parameters and notification flag).
  File name prefers media metadata, then blob metadata, then a kind/MIME default; content type prefers media MIME, then blob MIME.
  `blob_info` checks photo uploads against 10 MB and other uploads against 50 MB before HTTP; missing/evicted blobs and oversized uploads report translated failures.
  A 429 retries the original JSON or multipart request.
  `telegram/download` accepts any `media.*` with `file_id`: node-owned `getFile` → file URL GET with `save_to_blob` → Done and Media, preserving the media type and input fields while adding/replacing `blob` and filling missing size/MIME.
  The `getFile` size limit is 20 MB; the credential-bearing URL never enters outputs, traces, logs or errors, and error descriptions redact the token.
  A blob without `file_id` passes through unchanged; URL-only input directs users to native `media/fetch`.
  Its persisted `kind` selector (`auto` default, photo/audio/video/document) leaves output 13 Any in auto or selects `media.<kind>` with stable pin IDs; mismatched explicit kinds fail.
  Chain New message Message → Break object (`telegram.message`) → Photo → Download media Media, Received → In, then Done and Media to the destination action.
  Send supports explicit or inferred reply topics, reply parameters, and link preview disabling.
  `telegram/edit` edits text via `editMessageText` using Message (type Any), supporting plain text or a composed message with optional inline keyboard.
  `telegram/edit` rejects media messages and non-inline keyboards with translated errors.
  When an edited message has no keyboard, `keep_buttons` preserves existing buttons (`yes`) or removes them (`no`).
  All properties persist symmetrically and validate both interactive and restored values.
  Help and all UI texts use `locales/{en,uk}.ftl`.
- Identifier types: `telegram.chat_id` labels `chats`, `telegram.user_id` labels `users`, `telegram.message_id` without labels, `telegram.topic_id` labels `topics`.
  Topics are keyed internally by `(chat, topic)`; wire values remain raw integers.
  Since identifier lookup has no chat context, colliding topic IDs share one decimal option with combined `Chat › Topic / Other chat › Topic` labels.
  Use Chat and Topic filters together.
  User/message structs keep optional data absent; trigger projections clear absent output pins.
  Outgoing message types: enum `telegram.parse_mode` (`plain`, `html`, `markdown_v2`), enum `telegram.button_action` (`callback`, `url`, `text`, `contact`, `location`), struct `telegram.button` (`text`, `action`, optional `data`), enum `telegram.keyboard_kind` (`inline`, `reply`, `remove`), struct `telegram.keyboard` (`kind`, nested list `rows`, optional `resize`, `one_time`, `placeholder`, `persistent`), and struct `telegram.outgoing` (`text`, `parse_mode`, `media`, `keyboard`, `silent`).
- `directory.json`: atomic tmp+rename, ~5 s debounce, UTC-date `last_seen` touched only once daily, immediate current-name updates/options publication, startup/daily TTL expiry, 10,000 entries per kind with least-recently-seen eviction.
  Historical quoted messages fill unknown entries without undoing renames.
  Unknown send targets alone request `getChat`, at most once per day per chat across restarts (`lookups.json`).
  The bounded message/topic cache is transient.
- `cargo test -p telegram-plugin` runs the scripted fake API/storage/clock tests without networking.
  `ticknode-core/tests/plugins/telegram.rs` runs a local fake HTTP server for message → send, persisted-offset reload and real multipart blob upload.
  It requires a separately copied `telegram_plugin_test.wasm` built with `--features test-api`; the normal component never accepts an alternate endpoint.
  Exact build instructions are in the plugin README.
  Never bundle that test fixture.
