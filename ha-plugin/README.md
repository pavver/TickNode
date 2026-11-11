# Home Assistant plugin

The plugin runs `ha-core-client::Session` through the host WebSocket capability. Configure its URL
and access token in the Plugins panel; the Home Assistant add-on supplies the Supervisor settings.

## Entity changes and typed values

Choose an entity in Resources and select **When it changes** to create `ha/entity`. Its outputs are
**Changed** (exec 0), **Value** (1), **Previous** (2), **Unit** (3), **Attributes** (4), and
**Entity state** (`ha.state`, 5). Value and Previous share the selected `value_type`: Number,
On/Off, Text (default), Date and time (RFC 3339), or any of the 17 built-in physical quantities.
Choosing an entity known to the live cache infers its type; manual edits and stored types override inference. Unknown entities retain the
current type. Unknown, unavailable and non-convertible values clear the corresponding typed pin.

`fire_on=state` fires Changed only when state text differs (including absent states); `any` also
fires on attribute-only updates. Data outputs update for every event. Events are queued in order,
one per tick. Initial and reconnect snapshots update current data without firing Changed or
changing Previous. Removal clears current data; offline values remain cached, while missed events
are not replayed. Failed subscriptions retry after 30 seconds.

A recognized `unit_of_measurement` infers a quantity type; `device_class` constrains the mapping
(for example voltage, power, battery/humidity, pressure and particulate concentration). Unknown
units retain Number behavior. Value and Previous convert each state's own unit to the canonical
unit: `2 kW` becomes `2000` on `quantity.power`, and `68 °F` becomes `20` on
`quantity.temperature`. A missing/incompatible unit clears that sample's pin and logs an English
warning once per node until the units recover. Unit keeps the raw HA spelling. Attributes and
`ha.state` are unchanged; the latter still stores the state as text.

For example, connect a voltage sensor's Changed and Voltage Value to Match, use an auto input `v`
and cases `v > 230 V` and `v < 200 V`, then connect each case to `ha/action`.

## Action fields and units

A number selector with a recognized unit becomes a quantity input. Before sending the action,
the plugin converts the canonical input back to the selector's captured unit: a `2000 W` input
for a `kW` field sends `2`. Unmapped or unitless selectors remain Number fields. Field pin IDs,
labels and required flags keep their existing order. An appended `field_units` property stores
units positionally alongside the existing field snapshot; missing metadata defaults to no unit,
so old Number snapshots load without scaling. Reselect the action to adopt a newer HA schema.

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
