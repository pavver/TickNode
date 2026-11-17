# Connection status and editor header
status-online =
    { $count ->
        [one] connected · { $count } node
       *[other] connected · { $count } nodes
    }
status-online-home =
    { $count ->
        [one] connected · { $count } sheet
       *[other] connected · { $count } sheets
    }
status-connecting = connecting…
status-offline = disconnected from server

toolbar-undo = Undo (Ctrl/Cmd+Z)
toolbar-redo = Redo (Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y)
toolbar-palette = Node palette
toolbar-plugins = Plugins
toolbar-fit = Fit graph to view (F)
toolbar-add-frame = Add frame
toolbar-console = Console
toolbar-theme = Light / dark theme
toolbar-language = Language: English / Українська

# Errors for the user (`UiMessage` keys sent by the server; arguments are strings)
error-cycle-detected = Circular reference: node { $node } already depends on { $dependency }
error-type-mismatch = Incompatible data types: cannot connect { $src } to { $dst }
error-invalid-pin-connection = Invalid pin connection: one or both pins do not exist or have the wrong direction/type
error-node-not-found = Node { $node } not found
error-unknown-node-type = Unknown node type: { $type }
error-invalid-property = Invalid property: { $property }
error-invalid-variable-ref = Invalid variable reference: { $reference }
error-node-plugin-unavailable = plugin { $plugin } unavailable
error-node-type-unknown = node type { $type } is unknown: the required plugin is missing or disabled
error-paste-too-many = Too many nodes to paste: { $count } (maximum { $max })
error-paste-incomplete = Not everything was pasted: { $details }
error-bad-message = Invalid message: { $error }
error-setting-invalid = Setting "{ $setting }": invalid value
error-setting-required = Required setting "{ $setting }" is not set
error-setting-unknown = Unknown setting "{ $setting }"
error-settings-not-saved = Failed to save the settings: { $details }
error-plugin-not-found = Plugin { $plugin } not found
error-plugin-not-loaded = Plugin { $plugin } is not loaded
error-plugin-crash = Plugin crashed: { $reason }
error-plugin-disable-blocked = Plugin "{ $plugin }" cannot be disabled: its data types are used by { $dependents }; disable them first
error-plugin-config-corrupt = The plugin config is corrupted: { $details }. Saving settings or enabling the plugin will overwrite it
error-plugin-config-not-saved = The plugin state was changed, but the config was not saved: { $details }
error-condition-missing-pin = Missing pin number ("1: a > 0")
error-condition-not-pin-number = "{ $text }" is not a pin number
error-condition-input-format = "{ $text }": an input is written as "pin:name:type"
error-condition-invalid-input-name = "{ $name }" is not a valid input name
error-condition-unknown-type = "{ $type }" is an unknown type
error-condition-expected-join = { $end ->
    [true] Expected "and" / "or", not the end of text
   *[other] Expected "and" / "or", not «{ $found }»
}
error-condition-expected-operand = { $end ->
    [true] Expected an operand, not the end of text
   *[other] Expected an operand, not «{ $found }»
}
error-condition-expected-operator = { $end ->
    [true] Expected an operator, not the end of text
   *[other] Expected an operator, not «{ $found }»
}
error-condition-unknown-operator = "{ $operator }" is an unknown operator
error-condition-unclosed-quote = Unclosed quote
error-condition-not-number = "{ $text }" is not a number
error-condition-doubled-operator = "{ $char }" — probably "{ $char }{ $char }"
error-condition-unexpected-char = Unexpected character "{ $char }"

# Node categories
category-events = Events
category-control-flow = Control Flow
category-time = Time
category-actions = Actions
category-data = Data
category-math = Math
category-logic = Logic
category-string = Strings
category-lists = Lists
category-objects = Objects
category-variables = Variables
category-network = Network
category-debug = Debug
category-test = Test

# Data types and input hints
type-exec = Execution
type-number = Number
type-string = Text
type-bool = Yes/no
type-time = Time
type-datetime = Date and time
type-duration = Duration
type-color = Color
type-object = Object
type-list = List
type-any = Anything
type-auto = Auto
type-list-of = List: { $inner }

hint-time = HH:MM
hint-datetime = YYYY-MM-DD HH:MM
hint-duration = 10t / 1h 30m / 45s
hint-color = #rrggbb
hint-default = —

parse-error-number = "{ $value }" is not a number
parse-error-bool = "{ $value }" is not true/false
parse-error-time = "{ $value }" is not a time (HH:MM or HH:MM:SS)
parse-error-datetime = "{ $value }" is not a date-time (YYYY-MM-DD HH:MM)
parse-error-duration = "{ $value }" is not a duration (10t, 1h 30m, 500ms)
duration-input-hint = Enter a number in the selected unit, or text such as 1h 30m or 10t
duration-unit-title = Duration unit
duration-unit-ticks = tick
duration-unit-ms = ms
duration-unit-s = s
duration-unit-min = min
duration-unit-h = h
duration-ticks = { $count ->
    [one] { $count } tick
   *[other] { $count } ticks
    }
node-timer-active = { $count ->
    [one] One active delay
   *[other] { $count } active delays
    }
node-timer-count = ×{ $count }
parse-error-color = "{ $value }" is not a color (#rrggbb)
parse-error-not-text = value of type { $type } cannot be entered as text

# Shared controls and inline node fields
ui-save = SAVE
inline-value-type-title = Value type
inline-pin-value-title = Value used while the input is unconnected. Empty — node uses its default
inline-pin-string-title = Value used while the input is unconnected. Quoted "var.x" enters a literal text. Empty — node uses its default
inline-variable-tooltip = Variable «{ $name }» ({ $type }): { $value }
inline-variable-not-found = Variable «{ $name }» not found
inline-variable-clear = Remove variable binding
inline-variable-edit-title = Variable reference (var.x, sheet.x, global.x) or literal value

# Canvas and nodes
canvas-empty-title = Empty graph
canvas-empty-hint = Add a node from the palette and drag between pins to connect
node-plugin-unavailable = plugin unavailable
node-missing-required-title = Required input is not set — node will not execute
node-run-title = Run
pin-required-suffix = { " " }· required

# Match condition editor
match-section-inputs = inputs
match-add-input = + input
match-add-input-title = Add input (new data pin) to compare multiple values
match-section-cases = cases — top to bottom
match-add-case = + case
match-add-case-title = Add case (new output)
match-else = else
match-error-bad-input-name = "{ $name }" — invalid input name (letters, digits, _; not an operator)
match-error-duplicate-input = input "{ $name }" already exists
match-input-name-title = Input name — referenced by conditions
match-input-type-title = Input type; auto — any, operators follow the connected value
match-connect-hint = connect
match-remove-input-title = Remove input (and its conditions)
match-move-up-title = Move up
match-move-down-title = Move down
match-add-condition = + condition
match-add-condition-title = Add condition to this case
match-remove-case-title = Remove case (and its output)
match-then = then → { $index }
match-always = always
match-if = if
match-join-title = Toggle "and" / "or" ("and" binds tighter)
match-join-and = and
match-join-or = or
match-left-input-title = Input: { $type }
match-op-title = Operator (depends on input type)
match-right-source-title = Right side — value (✎) or another input
match-right-value-title = Value to compare against
match-remove-condition-title = Remove condition

# Match condition operators in display
match-op-eq = =
match-op-ne = ≠
match-op-lt = <
match-op-le = ≤
match-op-gt = >
match-op-ge = ≥
match-op-lt-time = < earlier
match-op-le-time = ≤ at or before
match-op-gt-time = > later
match-op-ge-time = ≥ at or after
match-op-lt-duration = < shorter
match-op-gt-duration = > longer
match-op-contains = contains
match-op-not-contains = does not contain
match-op-starts-with = starts with
match-op-ends-with = ends with
match-op-is-empty = is empty
match-op-not-empty = is not empty
match-op-is-true = = true
match-op-is-false = = false
match-op-has-field = has field
match-op-no-field = lacks field
match-op-len-eq = length =
match-op-len-gt = length >
match-op-len-lt = length <
match-op-has-value = has value
match-op-no-value = no value

# Node property labels
prop-value = Value
prop-var-name = Variable
prop-expression = Formula
prop-prefix = Prefix
prop-label = Label
prop-element-type = Element type
prop-value-type = Type
prop-field-type = Field type
prop-fields-list = Fields (name:type, …)
prop-interval-ticks = Interval, ticks
prop-cases = Cases

# Relative execution time
time-just-now = just now
time-seconds-ago =
    { $count ->
        [one] { $count } second ago
       *[other] { $count } seconds ago
    }
time-minutes-ago =
    { $count ->
        [one] { $count } minute ago
       *[other] { $count } minutes ago
    }
time-hours-ago =
    { $count ->
        [one] { $count } hour ago
       *[other] { $count } hours ago
    }
time-days-ago =
    { $count ->
        [one] { $count } day ago
       *[other] { $count } days ago
    }

# Node palette
palette-search-placeholder = Search nodes
palette-empty = Nothing found
palette-info-aria = How this node works

# Graph overview and inspector
overview-stats =
    { $nodes ->
        [one] { $nodes } node
       *[other] { $nodes } nodes
    } · { $connections ->
        [one] { $connections } connection
       *[other] { $connections } connections
    }
overview-title-controls = Controls
overview-hint-1 = Palette: click to add a node at center, drag to place it anywhere
overview-hint-2 = Drag from pin to pin to connect; drag a connected data input to move the wire
overview-hint-3 = Left-drag box: left-to-right selects nodes fully inside, right-to-left selects any touched
overview-hint-4 = Shift/Ctrl adds to selection; selected nodes move together
overview-hint-5 = Ctrl+C / Ctrl+X / Ctrl+V — clipboard, Ctrl+D — duplicate, Ctrl+A — select all; Ctrl/Cmd+Z — undo, Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y — redo
overview-hint-6 = Paste follows the cursor: left-click places, right-click or Esc cancels, middle-drag pans
overview-hint-7 = Delete removes selection, Esc clears selection; right-click a node or wire for its menu
overview-hint-8 = Middle-drag to pan, wheel to zoom, F to fit graph; touch: one finger pans, two fingers zoom
overview-hint-9 = Shift + right-click copies node settings; Shift + left-click pastes onto the same node type; Esc clears copied settings
settings-copied = Settings copied from { $title }
error-settings-type-mismatch = Cannot paste settings from { $from } to { $to }

exec-section-title = Execution
exec-section-inputs = Inputs
exec-section-outputs = Outputs
exec-when = Executed { $time }
exec-never = Not executed yet
exec-tick = tick { $tick }
exec-node-never-hint = Node has not executed yet
exec-fired = ▶ fired
exec-not-fired-hint = Did not fire in the last execution
exec-no-value = no value
exec-no-value-hint = Source had no value yet
exec-connected-after-hint = Connected after the last execution
exec-not-connected = not connected
exec-default-used-hint = Node used the default value
exec-output-not-emitted-hint = No value emitted yet

conn-disconnect = Disconnect

# Console
console-title = Console
console-lines =
    { $count ->
        [one] { $count } line
       *[other] { $count } lines
    }
console-clear = Clear
console-hide = Hide console

# Plugins panel
plugins-title = Plugins
plugins-hide = Hide plugins panel
plugins-empty = No plugins found. Place compiled components (*.wasm) in data/plugins/ and restart the server.
plugins-cap-http-any = HTTP: any domains
plugins-cap-http-domains = HTTP: { $domains }
plugins-cap-storage = Storage: plugin's own directory
plugins-cap-ws-any = WebSocket: any hosts
plugins-cap-ws-hosts = WebSocket: { $hosts }
plugins-state-enabled = enabled
plugins-state-disabled = disabled
plugins-state-error = error
plugins-settings-error-chip = settings error
plugins-settings-error-title = Plugin rejected the settings
plugins-capabilities = Permissions
plugins-settings = Settings
plugins-save = Save
plugins-secret-will-clear = will be cleared
plugins-secret-set = set
plugins-secret-unset = not set
plugins-secret-clear = Clear
plugins-secret-clear-title = Remove saved value
plugins-field-error = "{ $field }": { $error }

# Searchable property picker (e.g. a Home Assistant entity)
picker-placeholder = select…
picker-search-placeholder = Search by name or ID
picker-empty = Nothing found
picker-no-options = No list available yet (the plugin has not published one). You can type a value.
picker-more = { $count } more - refine the search
picker-use-typed = Use "{ $value }"

# Implicit conversion on a data connection.
wire-conversion = { $src } → { $dst }

pin-split = Split struct pin
pin-collapse = Recombine struct pin
pin-technical-type = Technical type: { $name }
type-unavailable = Unavailable type

error-batch-command = This command is not allowed inside a batch

inspector-cost = cost { $cost }
inspector-cost-help = Relative work per call; native logic costs 1. The tick budget spreads queued work over ticks.

wire-menu-empty = No compatible nodes
wire-menu-popular = Frequently used
quick-add-recent = Recent
quick-add-all-nodes = All nodes
overview-hint-quick-add = Double-click, Space or Tab on the canvas — quickly add a node

wire-point-add = Add point
wire-point-remove = Remove point
wire-points-clear = Remove all points
error-wire-route = Invalid wire route: the connection must exist and the route must contain at most 32 finite points.

error-plugin-api-mismatch = The plugin was built for plugin API { $plugin_api } (SDK { $plugin_sdk }), but this server uses plugin API { $server_api } (TickNode { $server_version }). Rebuild the plugin with the current SDK.
error-plugin-api-unknown = The plugin was built with an SDK that predates plugin API checks. This server uses plugin API { $server_api } (TickNode { $server_version }). Rebuild the plugin with the current SDK.

# Unconfirmed pin contracts and detached wires
node-frozen-title = Pins changed
node-frozen-confirm = Looks right
node-frozen-help = This node does not run until confirmed. Disconnected wires stay until you remove or reconnect them.
pin-change-removed = Removed { $direction } pin “{ $name }” ({ $kind })
pin-change-changed = Pin “{ $name }”: { $old } → { $new }
pin-change-required = New required input “{ $name }” ({ $kind })
pin-direction-input = input
pin-direction-output = output
wire-detached-remove = Remove wire

node-resize-title = Drag to resize, double-click to reset
overview-hint-resize = Drag the bottom-right corner to resize a node; double-click it to reset. Esc cancels. In multiline fields, Ctrl/Cmd+Enter saves.
overview-hint-frame = C — add a frame around the selected nodes; drag the title to move it with the nodes inside; corner to resize; double-click the title to rename
overview-hint-nudge = Arrow keys move the selected nodes or frame by one grid cell, with Alt by a fine step, with Shift by five cells
error-node-size = Node dimensions must be finite

# Runtime node errors and trace values on the canvas
error-node-required-missing = Required input “{ $pin }” is not set
error-node-required-conversion = Could not convert the value for input “{ $pin }”
error-node-required-variable = Required input “{ $pin }” variable “{ $variable }” is not available
error-node-frozen = Node is frozen: its pins changed — confirm the changes
error-plugin-node-crashed = Plugin crashed: { $reason }
toolbar-show-values = Show values
pin-no-value = No value yet
pin-fired-ago = Fired { $time }

# Left panel tabs and resources
left-tab-nodes = Nodes
left-tab-resources = Resources
left-tab-variables = Variables

variables-coming-soon = Graph variables are coming soon: declared, typed values you can watch and change here.
variables-scope-sheet = This sheet
variables-scope-global = Global
variables-scope-persistent = Persistent — kept after restart
variables-shadowed = Hidden on this sheet by a sheet variable
variables-delete-confirm = Delete variable «{ $name }»?
variables-new = New variable
variables-scope-sheet-short = Sheet
variables-scope-global-short = Global
variables-scope-persistent-short = Persistent
variables-empty = No variables yet. Nodes «Variable: create» or the button above add them.
variables-no-match = No variables match the search
variables-action-get = Get value
variables-action-set = Set value
variables-action-changed = When it changes
variables-action-delete = Delete
variables-search-placeholder = Search variables...
variables-create = Create
variables-cancel = Cancel
variables-name = Name
variables-type = Type
variables-scope = Scope
variables-initial-value = Initial value
variables-expand = Expand
variables-collapse = Collapse
variables-add-to-canvas = Add to canvas
variables-toggle-multiline = Toggle multiline

resources-more = and { $count } more — refine the search
resources-empty = Enabled plugins offer no resources
resources-loading = No items yet

error-frame-invalid = Frame position and size must be finite
error-frame-limit = Too many frames
error-sheet-limit = Too many sheets
error-cross-sheet = Nodes on different sheets cannot be connected
error-no-sheet = Open a sheet first
error-sheet-not-found = Sheet not found
error-variable-missing = Variable "{ $name }" not found
error-variable-type = Value does not match variable type
error-variable-limit = Variable limit reached
error-variable-name = Invalid variable name "{ $name }"
error-variable-exists = Variable already exists in this scope
error-variable-scope = Global and persistent variables cannot have a sheet
frame-default-title = Comment
frame-resize-title = Drag to resize frame
frame-color-gray = Gray
frame-color-red = Red
frame-color-orange = Orange
frame-color-yellow = Yellow
frame-color-green = Green
frame-color-teal = Teal
frame-color-blue = Blue
frame-color-purple = Purple

# Sheets and home page
sheets-back = All sheets
sheet-disabled = Disabled
home-sheets = Scenario sheets
home-variables = Global variables
home-log = Log
home-usage = Usage
home-settings = Settings
usage-title = Usage & Load Statistics
usage-refresh = Refresh
usage-window-minutes = Last 60 minutes
usage-window-hours = Last 48 hours
usage-ticks = Ticks
usage-avg-tick = Avg tick
usage-max-tick = Max tick
usage-spent-work = Spent work
usage-deferred = Deferred
usage-plugins = Plugin fuel
usage-plugin-name = Plugin
usage-fuel = Fuel
usage-calls = Calls
usage-traps = Traps
usage-top-nodes = Top nodes by cost
usage-node-name = Node
usage-sheet = Sheet
usage-cost = Cost
usage-executions = Executions
usage-other-nodes = Other nodes
usage-sheets-title = Work by sheet
usage-empty = No statistics collected yet
usage-current-tick = Current tick
usage-main-sheet = Main
usage-global = Global
sheet-new = New sheet
sheet-new-title = New sheet
sheet-no-caption = No description
sheet-nodes =
    { $count ->
        [one] { $count } node
       *[other] { $count } nodes
    }
sheet-errors =
    { $count ->
        [one] { $count } error
       *[other] { $count } errors
    }
sheet-last-run = Last run { $time }
sheet-never-run = Not run yet
sheet-enabled = Enabled
sheet-edit = Edit
sheet-duplicate = Duplicate
sheet-move-up = Move up
sheet-move-down = Move down
sheet-delete = Delete
sheet-save = Save
sheet-cancel = Cancel
sheet-delete-confirm = Delete sheet «{ $title }»? It is moved to the trash folder on the server.
sheets-empty = No sheets yet — create the first one

# Settings section
settings-interface = Interface
settings-language = Language
settings-language-description = Interface language of the editor
settings-theme = Theme
settings-theme-dark = Dark
settings-theme-light = Light
settings-theme-description = Editor color scheme
settings-show-values = Show values on wires
settings-show-values-description = Display live data pin values directly along connected wires
settings-engine = Engine
settings-engine-note = Applies to the server and every open editor
settings-tick = Tick length (ms)
settings-tick-description = How often the engine runs a step. Each exec hop takes one tick. Shorter ticks react faster and use more CPU.
settings-budget = Work budget per tick
settings-budget-description = Limits the relative work per tick and postpones the rest to the next ticks; 0 means no limit.
settings-save = Save
settings-reset = Reset
settings-invalid-number = Invalid number
error-save-settings = Failed to save settings

settings-formatting-language = Formatting language
settings-formatting-language-description = Language for text produced by the engine, including To text and Text from template.
property-locale-server = Server ({ $language })
language-en = English
language-uk = Ukrainian
quantity-unit-title = Unit
parse-error-quantity = “{ $value }” is not a value of { $type }.
error-plugin-quantity-type = Plugin { $plugin } cannot declare reserved quantity type { $type }.
error-plugin-media-type = Plugin { $plugin } cannot declare reserved media type { $type }.
error-media-missing-source = Media object must have either url, file_id or blob set

settings-display = Units and formatting
settings-display-description = Rules use absolute canonical values. Thresholds are entered in the selected unit. Rules apply to all sheets and server text.
settings-display-default = Reset to default
settings-display-from = From |value|
settings-display-remove = Remove step
settings-display-add = + step
settings-display-digits = Digits
settings-display-invalid = Use 1–8 steps starting at zero with increasing finite thresholds; digits must be 1–15 for significant rounding or 0–15 for decimal places.
error-display-quantity = Unknown quantity in display rule: { $quantity }.
error-display-unit = Unknown unit in display rule: { $quantity }.
error-display-digits = Display rule for { $quantity } needs 1–15 significant digits or 0–15 decimal places.
error-display-steps = Display rule for { $quantity } needs 1–8 steps.
error-display-threshold = Thresholds for { $quantity } must start at zero, be finite, nonnegative and strictly increasing.
settings-display-rounding = Rounding
settings-display-significant = Significant digits
settings-display-decimals = Decimal places
settings-display-numbers = Numbers

error-hysteresis-thresholds = Invalid hysteresis thresholds: rising mode requires On threshold ≥ Off threshold; falling mode requires On threshold ≤ Off threshold

error-property-duration = Enter a valid duration.
error-property-time = Enter a time as HH:MM or HH:MM:SS.
error-property-multi-select = Choose values from the available options.
error-property-text = Enter text.
property-duration-earlier = Earlier (negative offset)
property-duration-later = Later (positive offset)
property-time-placeholder = HH:MM

property-from-input = From input

property-duration-placeholder = 30
property-duration-hint = A number in the selected unit, 1h 30m, or HH:MM:SS

parse-error-identifier = "{ $value }" is not a signed 64-bit integer
identifier-clear = Clear identifier
error-plugin-identifier-output = The plugin returned an invalid identifier value

template-error = Line { $line }, col { $column }: { $message }
template-render-error = Render error: { $message }
template-preview-empty = (empty)
template-preview-waiting = Preview appears after the inputs get values
template-preview-samples = Try values
template-preview-sample-badge = sample
template-preview-samples-title = Sample values
template-preview-samples-clear = Clear
template-preview-no-waiting-inputs = All inputs have values
template-hover-text-of = Formatted like To text ({ $name })
template-lint-unknown-filter = Unknown filter '{ $name }'
template-lint-unknown-filter-suggest = Unknown filter '{ $name }'. Did you mean '{ $suggestion }'?
template-lint-filter-type = Filter '{ $name }' cannot be applied to { $type }
template-lint-unknown-field = Unknown field '{ $field }' on { $type }
template-lint-unknown-field-suggest = Unknown field '{ $field }' on { $type }. Did you mean '{ $suggestion }'?
template-lint-unknown-test = Unknown test '{ $name }'
template-lint-unknown-variable = Unknown variable '{ $name }'
template-lint-unknown-variable-suggest = Unknown variable '{ $name }'. Did you mean '{ $suggestion }'?

template-help-filter-date = Formats a date-time or time: date('DD.MM.YYYY HH:mm') or a preset; type inside the quotes for format codes
template-help-filter-strftime = Formats a date-time or time using strftime format codes
template-help-filter-text = Formats a value using its unit and locale rules
template-help-filter-plural = Selects a plural form: positional forms follow the formatting language's rules (e.g. one, few, many), keywords zero/one/two/few/many/other, and # is replaced by the formatted number
template-help-filter-default = Fallback value when the input is undefined or empty
template-help-function-now = Current date and time as an ISO 8601 / RFC 3339 string
template-help-filter-ago = Formats relative time, e.g. '5 minutes ago' or 'in 2 hours'
template-help-filter-duration = Formats a duration using compact units ('short'), words ('long'), or an 'H:mm' pattern
template-help-filter-unit = Formats a number or physical quantity using project display rules or an explicit unit symbol
template-help-unit-symbol = Unit of measurement for { $quantity }

template-help-date-preset-datetime = Standard date and time
template-help-date-preset-date = Date only
template-help-date-preset-time = Time only
template-help-date-preset-long = Long date with month name in the formatting language
template-help-date-preset-iso = ISO 8601 date and time

template-help-date-token-year4 = Year, 4 digits
template-help-date-token-year2 = Year, 2 digits
template-help-date-token-month-name = Month name (with a day in the pattern, the grammatical form used for dates)
template-help-date-token-month-short = Abbreviated month name
template-help-date-token-month2 = Month, 2 digits (01–12)
template-help-date-token-month1 = Month, 1 or 2 digits (1–12)
template-help-date-token-day2 = Day of the month, 2 digits (01–31)
template-help-date-token-day1 = Day of the month, 1 or 2 digits (1–31)
template-help-date-token-weekday-name = Full weekday name
template-help-date-token-weekday-short = Abbreviated weekday name
template-help-date-token-hour24-2 = Hour 00–23, 2 digits
template-help-date-token-hour24-1 = Hour 0–23, 1 or 2 digits
template-help-date-token-hour12-2 = Hour 01–12, use with A for AM/PM
template-help-date-token-hour12-1 = Hour 1–12, use with A for AM/PM
template-help-date-token-minute2 = Minute, 2 digits (00–59)
template-help-date-token-minute1 = Minute, 1 or 2 digits (0–59)
template-help-date-token-second2 = Second, 2 digits (00–59)
template-help-date-token-second1 = Second, 1 or 2 digits (0–59)
template-help-date-token-millis = Milliseconds, 3 digits
template-help-date-token-ampm-upper = AM or PM, uppercase
template-help-date-token-ampm-lower = am or pm, lowercase
template-help-date-token-offset-plain = UTC offset without colon, e.g. +0200
template-help-date-token-offset-colon = UTC offset with colon, e.g. +02:00
template-help-date-literal = Text in square brackets is printed as is, e.g. [at]

template-help-strftime-upper-y = Year, 4 digits
template-help-strftime-y = Year, 2 digits (00–99)
template-help-strftime-m = Month, 2 digits (01–12)
template-help-strftime-upper-b = Full month name
template-help-strftime-b = Abbreviated month name
template-help-strftime-d = Day of the month, 2 digits (01–31)
template-help-strftime-e = Day of the month, space-padded ( 1–31)
template-help-strftime-upper-a = Full weekday name
template-help-strftime-a = Abbreviated weekday name
template-help-strftime-j = Day of the year, 3 digits (001–366)
template-help-strftime-upper-h = Hour 00–23, 2 digits
template-help-strftime-upper-i = Hour 01–12, 2 digits
template-help-strftime-upper-m = Minute, 2 digits (00–59)
template-help-strftime-upper-s = Second, 2 digits (00–59)
template-help-strftime-f = Microseconds, 6 digits
template-help-strftime-p = AM or PM designation
template-help-strftime-z = UTC offset (+HHMM or -HHMM)
template-help-strftime-upper-z = Time zone name or abbreviation
template-help-strftime-percent = Literal % character

template-help-duration-preset-short = Compact localized units
template-help-duration-preset-long = Spelled-out words with plural forms

template-help-duration-token-days = Whole days
template-help-duration-token-hours2 = Hours, 2 digits (within the day if d is present, otherwise total hours)
template-help-duration-token-hours1 = Hours, 1 or 2 digits (within the day if d is present, otherwise total hours)
template-help-duration-token-minutes2 = Minutes, 2 digits (within the hour if d/H/HH is present, otherwise total minutes)
template-help-duration-token-minutes1 = Minutes, 1 or 2 digits (within the hour if d/H/HH is present, otherwise total minutes)
template-help-duration-token-seconds2 = Seconds within the minute, 2 digits
template-help-duration-token-seconds1 = Seconds within the minute, 1 or 2 digits
template-help-duration-token-millis = Milliseconds, 3 digits
template-help-duration-literal = Text in square brackets is printed as is, e.g. [h]

template-help-filter-abs = Returns the absolute value of a number.
template-help-filter-attr = Looks up an attribute or item on an object.
template-help-param-filter-attr-name = Name of the attribute to look up
template-help-filter-batch = Groups a sequence into slices of the specified size.
template-help-param-filter-batch-size = Number of items per batch
template-help-param-filter-batch-fill_with = Optional filler value for incomplete batches
template-help-filter-bool = Converts a value to a boolean.
template-help-filter-capitalize = Capitalizes the first character and lowercases the rest.
template-help-filter-count = Returns the number of items in a sequence or string.
template-help-param-filter-d-default = Fallback value when undefined or empty
template-help-param-filter-date-pattern = Date/time format pattern or preset
template-help-param-filter-default-value = Fallback value when undefined or empty
template-help-filter-dictsort = Sorts a dictionary by key or value.
template-help-param-filter-dictsort-case_sensitive = Whether sorting is case-sensitive
template-help-param-filter-dictsort-by = Sort by 'key' or 'value'
template-help-param-filter-dictsort-reverse = Sort in reverse order
template-help-filter-e = Alias for escape.
template-help-filter-escape = Escapes HTML characters in a string.
template-help-filter-first = Returns the first item of a sequence or string.
template-help-filter-float = Converts a value to a floating-point number.
template-help-param-filter-float-default = Fallback value if conversion fails
template-help-filter-format = Formats a string using printf-style placeholders.
template-help-param-filter-format-args = Positional values for printf-style placeholders
template-help-filter-groupby = Groups a sequence of objects by an attribute.
template-help-param-filter-groupby-attribute = Attribute name to group by
template-help-filter-indent = Indents text with spaces or a prefix.
template-help-param-filter-indent-first = Whether to indent the first line
template-help-param-filter-indent-blank = Whether to indent blank lines
template-help-filter-int = Converts a value to an integer.
template-help-param-filter-int-default = Fallback value if conversion fails
template-help-param-filter-int-base = Base for integer parsing (e.g. 10, 16)
template-help-filter-items = Returns an iterator over key-value pairs of a map.
template-help-filter-join = Joins elements of a sequence into a string.
template-help-param-filter-join-separator = String separator inserted between items
template-help-filter-last = Returns the last item of a sequence or string.
template-help-filter-length = Returns the number of items in a sequence, map or string.
template-help-filter-lines = Splits a string into a list of lines.
template-help-filter-list = Converts an iterable into a list.
template-help-filter-lower = Converts all characters in a string to lowercase.
template-help-filter-map = Applies a filter or extracts an attribute for each item in a sequence.
template-help-filter-max = Returns the largest item from a sequence.
template-help-param-filter-max-case_sensitive = Whether string comparisons are case-sensitive
template-help-param-filter-max-attribute = Optional attribute to compare on each item
template-help-filter-min = Returns the smallest item from a sequence.
template-help-param-filter-min-case_sensitive = Whether string comparisons are case-sensitive
template-help-param-filter-min-attribute = Optional attribute to compare on each item
template-help-filter-pprint = Pretty-prints a value for debugging.
template-help-filter-reject = Filters a sequence by removing items that pass a test.
template-help-param-filter-reject-test_name = Test name to filter out items that match
template-help-filter-rejectattr = Filters objects by removing items whose attribute passes a test.
template-help-param-filter-rejectattr-attribute = Attribute name to test
template-help-param-filter-rejectattr-test_name = Optional test name to filter out items
template-help-filter-replace = Replaces occurrences of a substring with another string.
template-help-param-filter-replace-old = Substring to find
template-help-param-filter-replace-new = Replacement substring
template-help-param-filter-replace-count = Maximum number of occurrences to replace
template-help-filter-reverse = Reverses a sequence or string.
template-help-filter-round = Rounds a number to a given precision.
template-help-param-filter-round-precision = Number of decimal places to round to
template-help-filter-safe = Marks a string as safe HTML.
template-help-filter-select = Filters a sequence by keeping only items that pass a test.
template-help-param-filter-select-test_name = Test name to keep only items that match
template-help-filter-selectattr = Filters objects by keeping items whose attribute passes a test.
template-help-param-filter-selectattr-attribute = Attribute name to test
template-help-param-filter-selectattr-test_name = Optional test name to match items
template-help-filter-slice = Slices an iterator and returns a list of slices.
template-help-param-filter-slice-slices = Number of slices to create
template-help-param-filter-slice-fill_with = Optional filler value for incomplete slices
template-help-filter-sort = Sorts an iterable.
template-help-param-filter-sort-reverse = Sort in descending order
template-help-param-filter-sort-case_sensitive = Whether sorting is case-sensitive
template-help-param-filter-sort-attribute = Optional attribute to sort by
template-help-filter-split = Splits a string by a separator into a list of strings.
template-help-param-filter-strftime-format = strftime format string
template-help-filter-string = Converts a value to a string.
template-help-filter-sum = Returns the sum of a sequence of numbers or attributes.
template-help-param-filter-sum-attribute = Optional attribute to sum over
template-help-param-filter-sum-start = Initial value for the sum
template-help-filter-title = Capitalizes the first character of each word in a string.
template-help-filter-trim = Strips leading and trailing whitespace or characters.
template-help-param-filter-trim-chars = Optional string of characters to strip
template-help-filter-unique = Returns unique items from a sequence.
template-help-param-filter-unique-case_sensitive = Whether uniqueness is case-sensitive
template-help-param-filter-unique-attribute = Optional attribute to determine uniqueness
template-help-param-filter-unit-symbol = Target unit symbol
template-help-param-filter-unit-digits = Significant digits count
template-help-param-filter-unit-decimals = Fixed decimal places count
template-help-param-filter-unit-units = Units map or unit system override
template-help-filter-upper = Converts all characters in a string to uppercase.
template-help-test-boolean = Checks if a value is a boolean.
template-help-test-defined = Checks if a variable is defined.
template-help-test-divisibleby = Checks if a number is divisible by another number.
template-help-param-test-divisibleby-num = Number to check divisibility by
template-help-test-endingwith = Checks if a string ends with a suffix.
template-help-param-test-endingwith-suffix = Suffix string to check for
template-help-test-eq = Checks if a value is equal to another value.
template-help-param-test-eq-other = Value to compare for equality
template-help-test-escaped = Checks if a string is marked as escaped or safe.
template-help-test-even = Checks if a number is even.
template-help-test-false = Checks if a value is strictly false.
template-help-test-filter = Checks if a filter with this name exists.
template-help-test-float = Checks if a number is a floating-point number.
template-help-test-ge = Checks if a value is greater than or equal to another value.
template-help-param-test-ge-other = Value to compare against
template-help-test-gt = Checks if a value is strictly greater than another value.
template-help-param-test-gt-other = Value to compare against
template-help-test-in = Checks if a value is present in a sequence or container.
template-help-param-test-in-seq = Sequence or container to check
template-help-test-integer = Checks if a number is an integer.
template-help-test-iterable = Checks if a value can be iterated over.
template-help-test-le = Checks if a value is less than or equal to another value.
template-help-param-test-le-other = Value to compare against
template-help-test-lower = Checks if all characters in a string are lowercase.
template-help-test-lt = Checks if a value is strictly less than another value.
template-help-param-test-lt-other = Value to compare against
template-help-test-mapping = Checks if a value is a map or dictionary.
template-help-test-ne = Checks if a value is not equal to another value.
template-help-param-test-ne-other = Value to compare against
template-help-test-none = Checks if a value is none or undefined.
template-help-test-number = Checks if a value is an integer or floating-point number.
template-help-test-odd = Checks if a number is odd.
template-help-test-safe = Checks if a string is marked as safe HTML.
template-help-test-sameas = Checks if two variables reference the exact same object.
template-help-param-test-sameas-other = Object to compare identity against
template-help-test-sequence = Checks if a value is a sequence.
template-help-test-startingwith = Checks if a string starts with a prefix.
template-help-param-test-startingwith-prefix = Prefix string to check for
template-help-test-string = Checks if a value is a string.
template-help-test-test = Checks if a test with this name exists.
template-help-test-true = Checks if a value is strictly true.
template-help-test-undefined = Checks if a variable is undefined.
template-help-test-upper = Checks if all characters in a string are uppercase.
template-help-function-debug = Prints debug information about current template variables.
template-help-function-dict = Creates a dictionary from keyword arguments.
template-help-function-namespace = Creates a mutable namespace object.
template-help-param-function-namespace-kwargs = Initial attributes for the namespace
template-help-function-range = Generates a sequence of integers.
template-help-param-function-range-stop = End of the integer range (exclusive)
template-help-param-function-range-step = Step increment between numbers

template-help-filter-d = Alias for the default filter.

template-help-param-filter-duration-pattern = Duration format pattern or preset ('short', 'long', 'H:mm')
template-help-param-filter-indent-width = Number of spaces or string for each indent level
template-help-param-filter-map-attribute = Filter or attribute name to apply to each item
template-help-param-filter-plural-forms = Positional forms or keywords zero/one/two/few/many/other
template-help-param-filter-round-method = Rounding method: 'common', 'ceil', or 'floor'
template-help-param-filter-split-separator = Optional separator string (defaults to whitespace)
template-help-param-function-dict-kwargs = Key-value pairs to construct a dictionary from
template-help-param-function-range-start = Start of the integer range (defaults to 0)

# Formula functions and errors
error-formula-too-large = Formula source length ({ $len } bytes) exceeds limit of { $max } bytes
error-formula-depth-exceeded = Formula nesting depth exceeds limit of { $max }
error-formula-unexpected-token = Unexpected token '{ $token }'
error-formula-unclosed-paren = Unclosed parenthesis in formula
error-formula-unknown-function = Unknown function '{ $name }'
error-formula-unknown-function-suggest = Unknown function '{ $name }'. Did you mean '{ $suggestion }'?
error-formula-wrong-arity = Function '{ $name }' expects { $expected } arguments, got { $got }
error-formula-reserved-name = Variable name '{ $name }' is reserved
error-formula-division-by-zero = Division by zero in formula
error-formula-domain-error = Mathematical domain error in function '{ $name }'
error-formula-overflow = Arithmetic overflow in formula
error-formula-incompatible-quantities = Incompatible physical quantities: cannot combine { $a } and { $b }
error-formula-type-mismatch = Type mismatch: expected { $expected }, got { $got }
error-formula-variable-type = Variable '{ $name }' has non-numeric type
error-formula-syntax = Invalid formula syntax
error-formula-eval = Formula evaluation failed: { $message }

formula-fn-abs = Absolute value of a number.
formula-fn-sign = Signum of a number (-1, 0, or 1).
formula-fn-min = Minimum value among arguments.
formula-fn-max = Maximum value among arguments.
formula-fn-clamp = Constrains a value to the range [lo, hi].
formula-fn-round = Rounds to the nearest integer or specified decimal places.
formula-fn-floor = Largest integer less than or equal to x.
formula-fn-ceil = Smallest integer greater than or equal to x.
formula-fn-trunc = Integer part of a number, truncated towards zero.
formula-fn-frac = Fractional part of a number.
formula-fn-sqrt = Square root of a non-negative number.
formula-fn-cbrt = Cube root of a number.
formula-fn-pow = Raises x to the power of y.
formula-fn-exp = Exponential function e^x.
formula-fn-ln = Natural logarithm (base e) of a positive number.
formula-fn-log = Logarithm with the specified base (defaults to e).
formula-fn-log10 = Common logarithm (base 10) of a positive number.
formula-fn-log2 = Binary logarithm (base 2) of a positive number.
formula-fn-hypot = Euclidean distance sqrt(a^2 + b^2 + ...).
formula-fn-sin = Sine of an angle in radians.
formula-fn-cos = Cosine of an angle in radians.
formula-fn-tan = Tangent of an angle in radians.
formula-fn-asin = Arcsine in radians, domain [-1, 1].
formula-fn-acos = Arccosine in radians, domain [-1, 1].
formula-fn-atan = Arctangent in radians.
formula-fn-atan2 = Four-quadrant arctangent in radians.
formula-fn-sinh = Hyperbolic sine.
formula-fn-cosh = Hyperbolic cosine.
formula-fn-tanh = Hyperbolic tangent.
formula-fn-deg = Converts radians to degrees.
formula-fn-rad = Converts degrees to radians.
formula-fn-sum = Sum of all argument values.
formula-fn-avg = Arithmetic mean of arguments.
formula-fn-median = Median value of arguments.
formula-fn-lerp = Linear interpolation between a and b by factor t.
formula-fn-map = Re-maps a number from [in_lo, in_hi] to [out_lo, out_hi].
formula-fn-if = Returns a if condition c is true, otherwise b.
formula-fn-mod = Modulo with the sign of divisor b.
formula-fn-gcd = Greatest common divisor of integers.
formula-fn-lcm = Least common multiple of integers.
formula-fn-between = Returns true if lo <= x <= hi.
formula-fn-isnan = Returns true if x is NaN.
formula-fn-isfinite = Returns true if x is a finite number.

formula-const-pi = Archimedes' constant pi (3.14159...)
formula-const-e = Euler's number e (2.71828...)
formula-const-tau = Circle constant tau, 2*pi (6.28318...)
formula-const-inf = Positive infinity
formula-const-nan = Not a number (NaN)
formula-const-true = Boolean true
formula-const-false = Boolean false

formula-error = Line { $line }, col { $column }: { $message }
formula-render-error = Evaluation error: { $message }
formula-preview-empty = (empty)
formula-preview-waiting = Preview appears after the inputs get values
formula-preview-samples = Try values
formula-preview-sample-badge = sample
formula-preview-samples-title = Sample values
formula-preview-samples-clear = Clear
formula-preview-no-waiting-inputs = All inputs have values
formula-lint-unknown-function = Unknown function '{ $name }'
formula-lint-unknown-function-suggest = Unknown function '{ $name }'. Did you mean '{ $suggestion }'?
formula-lint-wrong-arity = Function '{ $name }' expects { $expected } arguments, got { $got }


inline-plugin-value-tooltip = { $name } ({ $path }): { $value }
inline-binding-unavailable = Unavailable
inline-binding-type-error = Cannot bind { $source } to { $target }

wire-detached-title = Wire disconnected: the pin changed
node-menu-label = Node actions
node-menu-duplicate = Duplicate
node-menu-copy = Copy
node-menu-delete =
    { $count ->
        [one] Delete node
       *[other] Delete { $count } nodes
    }
toolbar-help = Canvas controls
sheet-rename = Rename sheet
sheet-title-label = Sheet title
sheet-title-empty = The title cannot be empty
home-navigation = Sections
home-rail-sheets = Sheets
home-rail-variables = Variables
home-rail-plugins = Plugins
home-rail-log = Log
home-rail-usage = Metrics
home-rail-settings = Settings
