# Home Assistant plugin

The plugin runs `ha-core-client::Session` through the host WebSocket capability. Configure its URL
and access token in the Plugins panel; the Home Assistant add-on supplies the Supervisor settings.

## Entity changes and typed values

Choose an entity in Resources and select **When it changes** to create `ha/entity`. Its outputs are
**Changed** (exec 0), **Value** (1), **Previous** (2), **Unit** (3), **Attributes** (4), and
**Entity state** (`ha.state`, 5). Value and Previous share the selected `value_type`: Number,
On/Off, Text (default), or Date and time (RFC 3339). Choosing an entity known to the live cache
infers its type; manual edits and stored types override inference. Unknown entities retain the
current type. Unknown, unavailable and non-convertible values clear the corresponding typed pin.

`fire_on=state` fires Changed only when state text differs (including absent states); `any` also
fires on attribute-only updates. Data outputs update for every event. Events are queued in order,
one per tick. Initial and reconnect snapshots update current data without firing Changed or
changing Previous. Removal clears current data; offline values remain cached, while missed events
are not replayed. Failed subscriptions retry after 30 seconds.

For example, connect a voltage sensor's Changed and Number Value to Match, use an auto input `v`
and cases `v > 230` and `v < 200`, then connect each case to `ha/action`.

## Automation conditions

Every `ha/condition_<kind>` node has exec **In** (0), **Yes** (1), **No** (2), and boolean data
**Result** (3). Local conditions evaluate immediately; the template condition completes asynchronously.
Definitions and logs are English, with Ukrainian Fluent translations for all editor texts.

| Kind | Properties |
| --- | --- |
| `time` | `after` (inclusive), `before` (exclusive), `weekday` (`mon,tue,...`) |
| `sun` | `after`, `before` (`none`/`sunrise`/`sunset`), `after_offset`, `before_offset` |
| `zone` | `entity_id` (person/tracker picker), `zone` (zone picker) |
| `template` | `value_template` (HA Jinja) |

A shared `state_changed` subscription is confirmed before requesting the initial `get_states`
snapshot. The existing five-minute entity-picker refresh also refreshes this cache. Events racing
a snapshot are retained; timestamps prevent an older update from replacing a newer state. Disconnects,
subscription restoration and lost events invalidate the baseline and require a fresh snapshot.
Missing/unknown/unavailable states produce No, with an English warning once per unchanged failure
situation.

WASI supplies a UTC wall clock, without access to HA's timezone database. `get_config` supplies
`time_zone`, but does **not** supply a UTC offset. A shared `render_template` subscription asks HA for
`now().utcoffset().total_seconds()`, so conversions use HA's offset rather than the host timezone.
No extra timezone crate or database is needed. HA [refreshes templates using `now()` every minute](https://www.home-assistant.io/template-functions/now/);
DST/timezone changes can therefore lag up to a minute plus transport/tick latency. Time and Sun fail
closed until configuration and offset are available. Weekdays refer to the current local day, even
in a window crossing midnight; equal After/Before times describe the whole day.

Sun projects the HA-local wall time of `sun.sun.next_rising`/`next_setting` onto today's local date
and then applies the signed offset. Before and After are independent inclusive tests combined with
AND. This approximates today's solar time when the next event is tomorrow; daily solar drift may
cause a few minutes of difference. Events more than two days away fail closed (polar day/night).
Zone compares the reported person/tracker state with the zone's friendly name, or `home` for
`zone.home`; it does not independently compute GPS/radius containment for overlapping zones.

Template starts a `render_template` subscription on each In, uses the first result, and unsubscribes.
Result and the selected branch arrive a few ticks later. Only boolean true or rendered text `true`
(case-insensitive) passes, matching HA template conditions. Render errors, disconnects and a 30-second
timeout produce No. At most 64 template evaluations may be pending per plugin. Property edits and
node deletion cancel pending evaluations; late replies cannot fire a replaced node.
