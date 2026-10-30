# Connection status and editor header
status-online =
    { $count ->
        [one] connected · { $count } node
       *[other] connected · { $count } nodes
    }
status-connecting = connecting…
status-offline = disconnected from server

toolbar-palette = Node palette
toolbar-plugins = Plugins
toolbar-fit = Fit graph to view (F)
toolbar-console = Console
toolbar-theme = Light / dark theme
toolbar-language = Language: English / Українська

# Editor messages in console
log-ui-connected = [UI] Connected to server
log-ui-graph-received =
    { $count ->
        [one] [UI] Graph received: { $count } node
       *[other] [UI] Graph received: { $count } nodes
    }
log-ui-copied = [UI] Copied nodes: { $count }
log-ui-cut = [UI] Cut nodes: { $count }
log-error-offline-send = [ERROR] Disconnected from server — change was not sent
log-error-bad-server-msg = [ERROR] Invalid server message: { $error }
log-error-connection-lost = [ERROR] Connection to server lost, reconnecting…
log-error-empty-clipboard = [ERROR] Clipboard does not contain TickNode nodes
log-error-connect-same-dir = [ERROR] Only an output can be connected to an input
log-error-connect-exec-mismatch = [ERROR] An exec pin can only be connected to an exec pin
log-error-copy-blocked = [ERROR] Browser blocked copying — try Ctrl+C

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
type-exec = exec
type-number = number
type-string = string
type-bool = bool
type-time = time
type-datetime = datetime
type-duration = duration
type-color = color
type-object = object
type-list = list
type-any = any
type-auto = auto
type-list-of = list<{ $inner }>

hint-time = HH:MM
hint-datetime = YYYY-MM-DD HH:MM
hint-duration = 1h 30m / 45s
hint-color = #rrggbb
hint-default = —

parse-error-number = "{ $value }" is not a number
parse-error-bool = "{ $value }" is not true/false
parse-error-time = "{ $value }" is not a time (HH:MM or HH:MM:SS)
parse-error-datetime = "{ $value }" is not a date-time (YYYY-MM-DD HH:MM)
parse-error-duration = "{ $value }" is not a duration (90, 1h 30m, 500ms)
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
overview-hint-5 = Ctrl+C / Ctrl+X / Ctrl+V — clipboard, Ctrl+D — duplicate, Ctrl+A — select all
overview-hint-6 = Paste follows the cursor: left-click places, right-click or Esc cancels, middle-drag pans
overview-hint-7 = Delete removes selection, Esc clears selection
overview-hint-8 = Middle-drag to pan, wheel to zoom, F to fit graph; touch: one finger pans, two fingers zoom

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
