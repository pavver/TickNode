# Connection status and editor header
status-online =
    { $count ->
        [one] connected · { $count } node
       *[other] connected · { $count } nodes
    }
status-connecting = connecting…
status-offline = disconnected from server

toolbar-undo = Undo (Ctrl/Cmd+Z)
toolbar-redo = Redo (Ctrl/Cmd+Shift+Z / Ctrl/Cmd+Y)
toolbar-palette = Node palette
toolbar-plugins = Plugins
toolbar-fit = Fit graph to view (F)
toolbar-console = Console
toolbar-theme = Light / dark theme
toolbar-language = Language: English / Українська

# Errors for the user (`UiMessage` keys sent by the server; arguments are strings)
error-cycle-detected = Circular reference: node { $node } already depends on { $dependency }
error-type-mismatch = Incompatible data types: cannot connect { $src } to { $dst }
error-invalid-pin-connection = Invalid pin connection: one or both pins do not exist or have the wrong direction/type
error-node-not-found = Node { $node } not found
error-unknown-node-type = Unknown node type: { $type }
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

# Automatic nodes of plugin data types
custom-make-title = Make { $type }
custom-make-description = Builds a "{ $type }" value from fields
custom-break-title = Break { $type }
custom-break-description = Breaks "{ $type }" into fields
custom-make-help =
    Builds a value of type "{ $type }" ({ $name }) from individual fields.

    Each field is an input: connect a source or set the value right on the node. Required fields are required inputs: without them no value is created (an error appears in the node's log). An optional field without a value is skipped.

    Pure node: evaluated whenever its result is needed by another node.
custom-break-help =
    Breaks a value of type "{ $type }" ({ $name }) into individual fields: each field is an output pin.

    An optional field that is absent from the value has no value (the pin is empty).

    Pure node: evaluated whenever its result is needed by another node.

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
prop-field-type = Field type
prop-fields-list = Fields (name:type, …)
prop-interval-ticks = Interval, ticks
prop-all-matches = All matches
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
overview-title-graph = Graph
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
overview-hint-7 = Delete removes selection, Esc clears selection
overview-hint-8 = Middle-drag to pan, wheel to zoom, F to fit graph; touch: one finger pans, two fingers zoom
overview-hint-9 = Shift + right-click copies node settings; Shift + left-click pastes onto the same node type; Esc clears copied settings
settings-copied = Settings copied from { $title }
error-settings-type-mismatch = Cannot paste settings from { $from } to { $to }

inspector-node-not-found = Node not found
inspector-section-actions = Actions
inspector-duplicate = Duplicate (Ctrl+D)
inspector-delete-node = Delete node

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

group-selected =
    { $count ->
        [one] { $count } node selected
       *[other] { $count } nodes selected
    }
group-hint = Drag any of them to move all. Ctrl+V — paste a copy: left-click places, right-click cancels.
group-copy = Copy (Ctrl+C)
group-delete =
    { $count ->
        [one] Delete { $count } node
       *[other] Delete { $count } nodes
    }

conn-type-exec = Exec connection
conn-type-data = Data connection
conn-from = from
conn-to = to
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
wire-detached-title = Wire disconnected: the pin changed
wire-detached-remove = Remove wire

node-resize-title = Drag to resize, double-click to reset
overview-hint-resize = Drag the bottom-right corner to resize a node; double-click it to reset. Esc cancels. In multiline fields, Ctrl/Cmd+Enter saves.
error-node-size = Node dimensions must be finite

# Runtime node errors and trace values on the canvas
error-node-required-missing = Required input “{ $pin }” is not set
error-node-required-conversion = Could not convert the value for input “{ $pin }”
error-node-frozen = Node is frozen: its pins changed — confirm the changes
error-plugin-node-crashed = Plugin crashed: { $reason }
toolbar-show-values = Show values
pin-no-value = No value yet
pin-fired-ago = Fired { $time }
