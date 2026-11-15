# Built-in core nodes

# const
node-const-title = Constant
node-const-description = Returns a constant value
node-const-help =
    Emits the value set directly on the node ("Value" row) or in the right panel on output Value. The value type (number, string, boolean, etc.) is chosen on the node, and the pin type follows it.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-const-pin-Value = Value

# math/add
node-math-add-title = Math Add
node-math-add-description = Arithmetic operation on two numbers
node-math-add-help =
    Result = A + B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-add-pin-A = A
node-math-add-pin-B = B
node-math-add-pin-Result = Result

# math/sub
node-math-sub-title = Math Subtract
node-math-sub-description = Arithmetic operation on two numbers
node-math-sub-help =
    Result = A − B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-sub-pin-A = A
node-math-sub-pin-B = B
node-math-sub-pin-Result = Result

# math/mul
node-math-mul-title = Math Multiply
node-math-mul-description = Arithmetic operation on two numbers
node-math-mul-help =
    Result = A × B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-mul-pin-A = A
node-math-mul-pin-B = B
node-math-mul-pin-Result = Result

# math/div
node-math-div-title = Math Divide
node-math-div-description = Arithmetic operation on two numbers
node-math-div-help =
    Result = A ÷ B. Division by 0 yields infinity.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-div-pin-A = A
node-math-div-pin-B = B
node-math-div-pin-Result = Result

# math/expression
node-math-expression-title = Formula
node-math-expression-description = Evaluates an arbitrary mathematical formula
node-math-expression-help =
    Evaluates an arbitrary mathematical formula set directly on the node ("Formula" row).

    Syntax:
    • Numbers: integers (42, 1_000_000), decimals (3.14), scientific notation (1e-3).
    • Booleans: true, false.
    • Constants: pi, e, tau, inf, nan.
    • Arithmetic: + - * / % (modulo with sign of divisor), ^ or ** (power, right-associative).
    • Comparisons: == != < <= > >=; Logic: && || ! (and or not); Ternary: c ? a : b.
    • Functions: abs, sign, min, max, clamp(x, lo, hi), round(x[, digits]), floor, ceil, trunc, frac, sqrt, cbrt, pow, exp, ln, log(x[, base]), log10, log2, hypot, sin, cos, tan, asin, acos, atan, atan2, sinh, cosh, tanh, deg, rad, sum, avg/mean, median, lerp(a, b, t), map(x, in_lo, in_hi, out_lo, out_hi), if(c, a, b), mod(a, b), gcd, lcm, between(x, lo, hi), isnan, isfinite/finite.
    • Compatibility: supports evalexpr names such as `math::sqrt(x)`.

    Variables and pins:
    Each variable in the formula automatically becomes an input pin. Undeclared variables stay optional with an inline default of 0. Renaming a single variable in place preserves its pin ID and wire.

    Physical quantities:
    Inputs and outputs support physical quantities (quantity.*). Operations between the same quantity keep that quantity. Adding or subtracting a plain number treats it as canonical units. Multiplying or dividing a quantity by a number keeps that quantity. Dividing two identical quantities produces a dimensionless Number. Output type defaults to 'auto' (inferred) or can be fixed to a specific quantity or Number.

    Pure node: evaluated whenever its output is needed.
node-math-expression-pin-Result = Result
node-math-expression-prop-expression-title = Formula
node-math-expression-prop-expression-description = Mathematical formula; variables become input pins.
node-math-expression-prop-value_type-title = Output type
node-math-expression-prop-value_type-description = Output data type ('auto' inferred from inputs or an explicit number/quantity).

# logical/and
node-logical-and-title = Logical And
node-logical-and-description = Logical operation
node-logical-and-help =
    Result = true only if both A and B are true.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-and-pin-A = A
node-logical-and-pin-B = B
node-logical-and-pin-Result = Result

# logical/or
node-logical-or-title = Logical Or
node-logical-or-description = Logical operation
node-logical-or-help =
    Result = true if at least one of A or B is true.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-or-pin-A = A
node-logical-or-pin-B = B
node-logical-or-pin-Result = Result

# logical/not
node-logical-not-title = Logical Not
node-logical-not-description = Logical operation
node-logical-not-help =
    Result is the logical negation of A.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-not-pin-A = A
node-logical-not-pin-Result = Result

# variable/get
node-variable-get-title = Get Variable
node-variable-get-description = Returns the value of a variable by name
node-variable-get-help =
    Reads the variable named in the "Name" property and outputs its value on Value, indicating on Exists whether it was found.

    Lookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-variable-get-pin-Value = Value
node-variable-get-pin-Exists = Exists
node-variable-get-prop-name-title = Name
node-variable-get-prop-lookup-title = Lookup
node-variable-get-prop-lookup-option-auto = Auto
node-variable-get-prop-lookup-option-sheet = Sheet
node-variable-get-prop-lookup-option-global = Global

# variable/set
node-variable-set-title = Set Variable
node-variable-set-description = Writes a value to a variable
node-variable-set-help =
    On a signal at In, writes the value from Value to the resolved variable and passes execution to Out. If the variable is missing or the value does not fit its type, execution passes to Failed and an error is recorded.

    Lookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).
node-variable-set-pin-In = In
node-variable-set-pin-Out = Out
node-variable-set-pin-Failed = Failed
node-variable-set-pin-Value = Value
node-variable-set-prop-name-title = Name
node-variable-set-prop-lookup-title = Lookup
node-variable-set-prop-lookup-option-auto = Auto
node-variable-set-prop-lookup-option-sheet = Sheet
node-variable-set-prop-lookup-option-global = Global

# variable/create
node-variable-create-title = Create Variable
node-variable-create-description = Creates a new variable if it does not already exist
node-variable-create-help =
    On a signal at In, creates a new variable in the chosen scope with the initial value from Initial (defaulting to the type's default value if unconnected).

    If the variable already existed in that scope, execution passes to Exists without modifying its value; otherwise execution passes to Created.
node-variable-create-pin-In = In
node-variable-create-pin-Created = Created
node-variable-create-pin-Exists = Exists
node-variable-create-pin-Initial = Initial
node-variable-create-prop-name-title = Name
node-variable-create-prop-scope-title = Scope
node-variable-create-prop-scope-option-sheet = Sheet
node-variable-create-prop-scope-option-global = Global
node-variable-create-prop-scope-option-persistent = Persistent

# variable/delete
node-variable-delete-title = Delete Variable
node-variable-delete-description = Deletes a variable by name
node-variable-delete-help =
    On a signal at In, deletes the resolved variable and passes execution to Out. Deleting a variable that does not exist is not an error.

    Lookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).
node-variable-delete-pin-In = In
node-variable-delete-pin-Out = Out
node-variable-delete-prop-name-title = Name
node-variable-delete-prop-lookup-title = Lookup
node-variable-delete-prop-lookup-option-auto = Auto
node-variable-delete-prop-lookup-option-sheet = Sheet
node-variable-delete-prop-lookup-option-global = Global

# variable/changed
node-variable-changed-title = Variable Changed
node-variable-changed-description = Fires when a variable is created or updated with a new value
node-variable-changed-help =
    Triggers execution on Changed whenever the resolved variable is created or set to a different value. Deletion does not fire. Value outputs the new value.

    Lookup chooses whether to watch the current sheet, globally (including persistent), or automatically (sheet first, then global).
node-variable-changed-pin-Changed = Changed
node-variable-changed-pin-Value = Value
node-variable-changed-prop-name-title = Name
node-variable-changed-prop-lookup-title = Lookup
node-variable-changed-prop-lookup-option-auto = Auto
node-variable-changed-prop-lookup-option-sheet = Sheet
node-variable-changed-prop-lookup-option-global = Global

# match
node-match-title = Match
node-match-description = Selects an output by conditions, like a decider combinator
node-match-help =
    On a signal at In, checks cases from top to bottom and passes execution to the output of the first matching case. If none match — to Else.

    Mode: "First match" fires only the first matching case; "All matches" fires every matching case; "Only on change" fires the first matching case only when it differs from the case that fired last time. When no case matches in that mode, Else fires once and the last case is remembered, so a gap between thresholds works as hysteresis: with cases "v > 230" and "v < 200", values between 200 and 230 do not let "v > 230" fire again until "v < 200" has fired. The memory resets when cases, inputs or the mode change and after a restart.

    Inputs: "+ input" adds an input pin. The input name can be renamed (conditions update automatically) and its type selected; "auto" accepts anything, picking operators based on the connected source type. An unconnected input can be given a value directly on the node.

    A condition is "input  operator  operand". Operators depend on the input type:
    • numbers, time, duration — =, ≠, <, ≤, >, ≥ (for time — earlier / later);
    • strings — equals, contains, starts with, ends with, is empty, length;
    • booleans — = true / = false;
    • lists — contains, is empty, length;
    • objects — has field;
    • any — has value / no value.
    The right side is a value or another input (comparing two variables). Values are coerced to the input type: "07:30" — time, "1h 30m" or 90 — duration, "#ff0000" — color.

    Conditions are joined with "and" / "or"; "and" binds tighter: "a > 1 and b > 1 or c" = "(a > 1 and b > 1) or c". A case with no conditions always matches. An input without a value does not satisfy a condition (except "no value").
node-match-pin-In = In
node-match-pin-Else = Else
node-match-prop-mode-title = Mode
node-match-prop-mode-description = Which matching cases fire: the first one, all of them, or the first one only when it changes
node-match-prop-mode-option-first = First match
node-match-prop-mode-option-all = All matches
node-match-prop-mode-option-on-change = Only on change

# clock
node-clock-title = Clock
node-clock-description = Emits an execution pulse every interval_ticks ticks
node-clock-help =
    Triggers execution on its own: every N ticks ("Interval" property) it emits a signal on Out.

    The server tick is 100 ms, so 10 ticks ≈ 1 second. This is the primary event source for chains that run continuously.
node-clock-pin-Out = Out

# console
node-console-title = Console Log
node-console-description = Prints a value to the console when triggered
node-console-help =
    On a signal at In, prints the value of Value to the editor console and server log. Without Value connected, prints "None".

    Has no output — this is a terminal node in a chain. To log a value in the middle of a chain, use Debug Log.
node-console-pin-In = In
node-console-pin-Value = Value

# test/click_button
node-test-click-button-title = Click Button
node-test-click-button-description = Emits an exec signal on the next tick after the user clicks the button
node-test-click-button-help =
    Emits a signal on Out when you click the Run button on the node.

    Handy for manually triggering a chain while debugging.
node-test-click-button-pin-Out = Out

# test/debug
node-test-debug-title = Debug Log
node-test-debug-description = Accepts any data and logs it with a text prefix
node-test-debug-help =
    On a signal at In, logs the "Prefix" together with the value of Value to the console and passes execution to Out.

    Can be inserted in the middle of a chain to inspect the value passing through it.
node-test-debug-pin-In = In
node-test-debug-pin-Value = Value
node-test-debug-pin-Out = Out

# compare
node-compare-title = Compare
node-compare-description = Compares two numbers
node-compare-help =
    Compares numbers A and B:
    • Eq — A equals B (within 1e-9);
    • Gt — A is greater than B;
    • Lt — A is less than B.

    Connecting a physical quantity (for example temperature) gives both inputs its type, so B can be entered in any of its units (20 °C or 68 °F).

    An unconnected input uses the value set next to the pin on the node, or defaults to 0.

    For branching execution on a condition, the Match node is more convenient.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-compare-pin-A = A
node-compare-pin-B = B
node-compare-pin-Eq = Eq
node-compare-pin-Gt = Gt
node-compare-pin-Lt = Lt

# for_loop
node-for-loop-title = For Loop
node-for-loop-description = Asynchronous loop from 0 to Count-1 with step 1
node-for-loop-help =
    Step-by-step loop:
    1. A signal at In starts the loop with Index = 0 and fires Loop body.
    2. When the loop body finishes, send a signal to Next — Index increments by 1 and Loop body fires again.
    3. After Count iterations or a signal at Break, Completed fires.

    If Count ≤ 0, Completed fires immediately. Each step takes at least one tick, so the loop never blocks the engine.
node-for-loop-pin-In = In
node-for-loop-pin-Count = Count
node-for-loop-pin-Break = Break
node-for-loop-pin-Next = Next
node-for-loop-pin-loop-body = Loop body
node-for-loop-pin-Index = Index
node-for-loop-pin-Completed = Completed

# sleep
node-sleep-title = Sleep
node-sleep-description = Delays execution by a duration in ticks or time
node-sleep-help =
    A signal at In starts a delay, and when it elapses, emits a signal on Out.

    • Delay — duration in ticks or time, e.g. 10t or 5s. Time is rounded up to whole server ticks. Without a value, waits one tick.

    Each signal at In starts its own independent delay, like Queue Delay.
node-sleep-pin-In = In
node-sleep-pin-Delay = Delay
node-sleep-pin-Out = Out

# media/fetch
node-media-fetch-title = Download media
node-media-fetch-description = Downloads a media file into blob storage
node-media-fetch-help =
    Downloads a media file from an HTTP(S) URL into host blob storage and produces a media descriptor.

    • URL — HTTP(S) URL to download from. Basic authentication in the URL (http://user:pass@host/...) is supported.
    • Headers — optional HTTP headers, one 'Name: value' per line.
    • Kind — target media type (photo, audio, video, document).

    Fires Done on success or Failed on error.
node-media-fetch-pin-In = In
node-media-fetch-pin-URL = URL
node-media-fetch-pin-Headers = Headers
node-media-fetch-pin-Done = Done
node-media-fetch-pin-Failed = Failed
node-media-fetch-pin-Error = Error
node-media-fetch-pin-Media = Media
node-media-fetch-prop-kind-title = Kind
node-media-fetch-prop-kind-description = Media kind
node-media-fetch-prop-kind-option-photo = Photo
node-media-fetch-prop-kind-option-audio = Audio
node-media-fetch-prop-kind-option-video = Video
node-media-fetch-prop-kind-option-document = File

# list/create
node-list-create-title = Create List
node-list-create-description = Creates an empty list
node-list-create-help =
    Outputs an empty list. The element type is selected on the node.

    Populate the list using the List Add node.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-list-create-pin-List = List

# list/add
node-list-add-title = List Add
node-list-add-description = Appends an item to a list
node-list-add-help =
    Outputs OutList — a copy of List with Item appended at the end. The input list is not modified.

    The element type is selected on the node and must match the list type.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-list-add-pin-List = List
node-list-add-pin-Item = Item
node-list-add-pin-OutList = OutList

# list/remove
node-list-remove-title = List Remove
node-list-remove-description = Removes an item from a list by index
node-list-remove-help =
    Outputs OutList — a copy of the list without the element at Index (0-based). If the index is out of bounds, the list is returned unchanged.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-list-remove-pin-List = List
node-list-remove-pin-Index = Index
node-list-remove-pin-OutList = OutList

# list/get
node-list-get-title = List Get
node-list-get-description = Gets an item from a list by index
node-list-get-help =
    Outputs Item — the list element at Index (0-based). If the index is out of bounds, the output has no value.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-list-get-pin-List = List
node-list-get-pin-Index = Index
node-list-get-pin-Item = Item

# list/length
node-list-length-title = List Length
node-list-length-description = Returns the length of a list
node-list-length-help =
    Outputs Len — the number of elements in the list. If no list is connected, outputs 0.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-list-length-pin-List = List
node-list-length-pin-Len = Len

# object/create
node-object-create-title = Create Object
node-object-create-description = Creates an empty object
node-object-create-help =
    Outputs an empty object — a collection of named fields.

    Add fields using the Set Object Field node.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-create-pin-Obj = Obj

# object/set_field
node-object-set-field-title = Set Object Field
node-object-set-field-description = Sets the value of an object field
node-object-set-field-help =
    Outputs OutObj — a copy of object Obj where field Name has value Value. If the field did not exist, it is added; otherwise it is replaced.

    The type of Value is selected on the node. Until all inputs are provided, no output is produced.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-set-field-pin-Obj = Obj
node-object-set-field-pin-Name = Name
node-object-set-field-pin-Value = Value
node-object-set-field-pin-OutObj = OutObj

# object/get_field
node-object-get-field-title = Get Object Field
node-object-get-field-description = Gets the value of an object field
node-object-get-field-help =
    Looks up field Name in object Obj:
    • Value — the field value;
    • Found — whether the field exists.

    The type of Value is selected on the node.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-get-field-pin-Obj = Obj
node-object-get-field-pin-Name = Name
node-object-get-field-pin-Value = Value
node-object-get-field-pin-Found = Found

# object/unpack
node-object-unpack-title = Unpack Object
node-object-unpack-description = Unpacks fields of an object
node-object-unpack-help =
    Outputs several object fields at once. Fields are listed on the node or in the right panel separated by commas as "name:type", for example `temp:number, city:string` (without a type — string).

    Each field becomes a separate output pin. A field missing from the object has no value.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-unpack-pin-Obj = Obj

node-object-break-title = Break Object
node-object-break-description = Breaks a typed object into individual fields
node-object-break-help =
    Breaks a typed object into individual fields: each field is an output pin.

    Select Type on the node or connect a typed object to Value to adapt it.

    An optional field that is absent from the value has no value (the pin is empty).

    Pure node: evaluated whenever its result is needed by another node.
node-object-break-pin-Value = Value
node-object-break-prop-type-title = Type
node-object-break-prop-type-description = Object struct type to break into fields

node-object-make-title = Make Object
node-object-make-description = Constructs a typed object from individual fields
node-object-make-help =
    Constructs a typed object from individual fields.

    Select Type on the node to choose the object structure.

    Required fields are required inputs: without them no value is created. An optional field without a value is skipped.

    Pure node: evaluated whenever its result is needed by another node.
node-object-make-pin-Value = Value
node-object-make-prop-type-title = Type
node-object-make-prop-type-description = Object struct type to construct from fields

# Physical quantities and unit symbols.
number-decimal-separator = .
type-quantity-power-title = Power
type-quantity-power-description = Power in physical units
unit-watt = W
unit-kilowatt = kW
unit-megawatt = MW
unit-milliwatt = mW
type-quantity-energy-title = Energy
type-quantity-energy-description = Energy in physical units
unit-watt-hour = Wh
unit-kilowatt-hour = kWh
unit-megawatt-hour = MWh
unit-joule = J
unit-kilojoule = kJ
unit-megajoule = MJ
type-quantity-voltage-title = Voltage
type-quantity-voltage-description = Voltage in physical units
unit-volt = V
unit-millivolt = mV
unit-kilovolt = kV
type-quantity-current-title = Current
type-quantity-current-description = Current in physical units
unit-ampere = A
unit-milliampere = mA
type-quantity-apparent-power-title = Apparent power
type-quantity-apparent-power-description = Apparent power in physical units
unit-volt-ampere = VA
unit-kilovolt-ampere = kVA
type-quantity-frequency-title = Frequency
type-quantity-frequency-description = Frequency in physical units
unit-hertz = Hz
unit-kilohertz = kHz
unit-megahertz = MHz
unit-gigahertz = GHz
type-quantity-temperature-title = Temperature
type-quantity-temperature-description = Temperature in physical units
unit-celsius = °C
unit-fahrenheit = °F
unit-kelvin = K
type-quantity-percentage-title = Percentage
type-quantity-percentage-description = Percentage in physical units
unit-percent = %
type-quantity-pressure-title = Pressure
type-quantity-pressure-description = Pressure in physical units
unit-pascal = Pa
unit-hectopascal = hPa
unit-kilopascal = kPa
unit-millibar = mbar
unit-bar = bar
unit-psi = psi
unit-mmhg = mmHg
unit-inhg = inHg
type-quantity-speed-title = Speed
type-quantity-speed-description = Speed in physical units
unit-meter-per-second = m/s
unit-kilometer-per-hour = km/h
unit-mile-per-hour = mph
unit-knot = kn
unit-foot-per-second = ft/s
type-quantity-length-title = Length
type-quantity-length-description = Length in physical units
unit-meter = m
unit-millimeter = mm
unit-centimeter = cm
unit-kilometer = km
unit-inch = in
unit-foot = ft
unit-yard = yd
unit-mile = mi
type-quantity-volume-title = Volume
type-quantity-volume-description = Volume in physical units
unit-liter = L
unit-milliliter = mL
unit-cubic-meter = m³
unit-gallon = gal
unit-cubic-foot = ft³
type-quantity-mass-title = Mass
type-quantity-mass-description = Mass in physical units
unit-kilogram = kg
unit-gram = g
unit-milligram = mg
unit-tonne = t
unit-pound = lb
unit-ounce = oz
type-quantity-illuminance-title = Illuminance
type-quantity-illuminance-description = Illuminance in physical units
unit-lux = lx
type-quantity-irradiance-title = Irradiance
type-quantity-irradiance-description = Irradiance in physical units
unit-watt-per-square-meter = W/m²
type-quantity-concentration-title = Concentration
type-quantity-concentration-description = Concentration in physical units
unit-microgram-per-cubic-meter = µg/m³
unit-milligram-per-cubic-meter = mg/m³
type-quantity-parts-title = Parts
type-quantity-parts-description = Parts in physical units
unit-ppm = ppm
unit-ppb = ppb


node-text-to-text-title = To Text
node-text-to-text-description = Converts any value to readable text
node-text-to-text-help = Converts Value to text. Connect a wire to select its type. Quantities follow project display rules: 1970 W → 1.97 kW; a fixed unit overrides the rule. Numbers and quantities support significant digits or fixed decimal places. Project rounding with empty digits follows the project rule; a selected mode defaults to 3 significant digits or 2 decimal places. Hide units to output a bare number; durations then use seconds. Booleans use localized words unless custom texts are set. Date/time formats use chrono strftime, e.g. %H:%M or %d.%m.%Y; date-times use the server local time zone. Lists join elements with commas; objects show their fields. Server uses the engine formatting language.
node-text-to-text-pin-Value = Value
node-text-to-text-pin-Text = Text
node-text-to-text-prop-value_type-title = Value type
node-text-to-text-prop-value_type-description = Declared input type; connecting a wire adapts it.
node-text-to-text-prop-show_units-title = Show units
node-text-to-text-prop-show_units-description = Append unit symbols; without units, durations use seconds.
node-text-to-text-prop-unit-title = Unit
node-text-to-text-prop-unit-description = Empty or Auto uses the project display rule.
node-text-to-text-prop-digits-title = Digits
node-text-to-text-prop-digits-description = Significant digits (1–15) or decimal places (0–15); empty uses the default for the selected rounding.
node-text-to-text-prop-locale-title = Language
node-text-to-text-prop-locale-description = Server follows the engine formatting language.
node-text-to-text-prop-true_text-title = True text
node-text-to-text-prop-true_text-description = Empty uses the localized default.
node-text-to-text-prop-false_text-title = False text
node-text-to-text-prop-false_text-description = Empty uses the localized default.
node-text-to-text-prop-datetime_format-title = Date and time format
node-text-to-text-prop-datetime_format-description = Chrono strftime; empty uses %Y-%m-%d %H:%M or %H:%M.
node-text-to-text-prop-unit-option-auto = Project rule
node-text-to-text-prop-locale-option-server = Server
node-text-to-text-prop-locale-option-en = English
node-text-to-text-prop-locale-option-uk = Ukrainian
node-text-to-text-prop-show_units-option-yes = Yes
node-text-to-text-prop-show_units-option-no = No
text-true = true
text-false = false
text-duration-d = d
text-duration-h = h
text-duration-m = m
text-duration-s = s
text-duration-ms = ms
text-date-format-datetime = YYYY-MM-DD HH:mm
text-date-format-date = YYYY-MM-DD
text-date-format-time = HH:mm
text-date-format-long = MMMM D, YYYY HH:mm
text-month-1 = January
text-month-1-format = January
text-month-1-short = Jan
text-month-2 = February
text-month-2-format = February
text-month-2-short = Feb
text-month-3 = March
text-month-3-format = March
text-month-3-short = Mar
text-month-4 = April
text-month-4-format = April
text-month-4-short = Apr
text-month-5 = May
text-month-5-format = May
text-month-5-short = May
text-month-6 = June
text-month-6-format = June
text-month-6-short = Jun
text-month-7 = July
text-month-7-format = July
text-month-7-short = Jul
text-month-8 = August
text-month-8-format = August
text-month-8-short = Aug
text-month-9 = September
text-month-9-format = September
text-month-9-short = Sep
text-month-10 = October
text-month-10-format = October
text-month-10-short = Oct
text-month-11 = November
text-month-11-format = November
text-month-11-short = Nov
text-month-12 = December
text-month-12-format = December
text-month-12-short = Dec
text-weekday-1 = Monday
text-weekday-1-short = Mon
text-weekday-2 = Tuesday
text-weekday-2-short = Tue
text-weekday-3 = Wednesday
text-weekday-3-short = Wed
text-weekday-4 = Thursday
text-weekday-4-short = Thu
text-weekday-5 = Friday
text-weekday-5-short = Fri
text-weekday-6 = Saturday
text-weekday-6-short = Sat
text-weekday-7 = Sunday
text-weekday-7-short = Sun
text-am = AM
text-pm = PM

text-ago-now = just now
text-ago-past-seconds-one = { $n } second ago
text-ago-past-seconds-few = { $n } seconds ago
text-ago-past-seconds-many = { $n } seconds ago
text-ago-past-seconds-other = { $n } seconds ago
text-ago-future-seconds-one = in { $n } second
text-ago-future-seconds-few = in { $n } seconds
text-ago-future-seconds-many = in { $n } seconds
text-ago-future-seconds-other = in { $n } seconds

text-ago-past-minutes-one = { $n } minute ago
text-ago-past-minutes-few = { $n } minutes ago
text-ago-past-minutes-many = { $n } minutes ago
text-ago-past-minutes-other = { $n } minutes ago
text-ago-future-minutes-one = in { $n } minute
text-ago-future-minutes-few = in { $n } minutes
text-ago-future-minutes-many = in { $n } minutes
text-ago-future-minutes-other = in { $n } minutes

text-ago-past-hours-one = { $n } hour ago
text-ago-past-hours-few = { $n } hours ago
text-ago-past-hours-many = { $n } hours ago
text-ago-past-hours-other = { $n } hours ago
text-ago-future-hours-one = in { $n } hour
text-ago-future-hours-few = in { $n } hours
text-ago-future-hours-many = in { $n } hours
text-ago-future-hours-other = in { $n } hours

text-ago-past-days-one = { $n } day ago
text-ago-past-days-few = { $n } days ago
text-ago-past-days-many = { $n } days ago
text-ago-past-days-other = { $n } days ago
text-ago-future-days-one = in { $n } day
text-ago-future-days-few = in { $n } days
text-ago-future-days-many = in { $n } days
text-ago-future-days-other = in { $n } days

text-ago-past-months-one = { $n } month ago
text-ago-past-months-few = { $n } months ago
text-ago-past-months-many = { $n } months ago
text-ago-past-months-other = { $n } months ago
text-ago-future-months-one = in { $n } month
text-ago-future-months-few = in { $n } months
text-ago-future-months-many = in { $n } months
text-ago-future-months-other = in { $n } months

text-ago-past-years-one = { $n } year ago
text-ago-past-years-few = { $n } years ago
text-ago-past-years-many = { $n } years ago
text-ago-past-years-other = { $n } years ago
text-ago-future-years-one = in { $n } year
text-ago-future-years-few = in { $n } years
text-ago-future-years-many = in { $n } years
text-ago-future-years-other = in { $n } years

text-duration-long-days-one = { $n } day
text-duration-long-days-few = { $n } days
text-duration-long-days-many = { $n } days
text-duration-long-days-other = { $n } days

text-duration-long-hours-one = { $n } hour
text-duration-long-hours-few = { $n } hours
text-duration-long-hours-many = { $n } hours
text-duration-long-hours-other = { $n } hours

text-duration-long-minutes-one = { $n } minute
text-duration-long-minutes-few = { $n } minutes
text-duration-long-minutes-many = { $n } minutes
text-duration-long-minutes-other = { $n } minutes

text-duration-long-seconds-one = { $n } second
text-duration-long-seconds-few = { $n } seconds
text-duration-long-seconds-many = { $n } seconds
text-duration-long-seconds-other = { $n } seconds

# Extended sensor quantities and units.
type-quantity-reactive-power-title = Reactive power
type-quantity-reactive-power-description = Reactive power in physical units
type-quantity-reactive-energy-title = Reactive energy
type-quantity-reactive-energy-description = Reactive energy in physical units
type-quantity-area-title = Area
type-quantity-area-description = Area in physical units
type-quantity-data-rate-title = Data rate
type-quantity-data-rate-description = Data rate in physical units
type-quantity-data-size-title = Data size
type-quantity-data-size-description = Data size in physical units
type-quantity-precipitation-intensity-title = Precipitation intensity
type-quantity-precipitation-intensity-description = Precipitation intensity in physical units
type-quantity-volume-flow-rate-title = Volume flow rate
type-quantity-volume-flow-rate-description = Volume flow rate in physical units
type-quantity-conductivity-title = Conductivity
type-quantity-conductivity-description = Conductivity in physical units
type-quantity-signal-strength-title = Signal strength
type-quantity-signal-strength-description = Signal strength in physical units
type-quantity-sound-pressure-title = Sound pressure
type-quantity-sound-pressure-description = Sound pressure in physical units
type-quantity-angle-title = Angle
type-quantity-angle-description = Angle in physical units
type-quantity-blood-glucose-title = Blood glucose
type-quantity-blood-glucose-description = Blood glucose in physical units
unit-gigawatt = GW
unit-terawatt = TW
unit-btu-per-hour = BTU/h
unit-gigawatt-hour = GWh
unit-terawatt-hour = TWh
unit-milliwatt-hour = mWh
unit-gigajoule = GJ
unit-calorie = cal
unit-kilocalorie = kcal
unit-megacalorie = Mcal
unit-gigacalorie = Gcal
unit-microvolt = µV
unit-megavolt = MV
unit-millivolt-ampere = mVA
unit-centibar = cbar
unit-inch-of-water = inH2O
unit-inch-per-day = in/d
unit-inch-per-hour = in/h
unit-inch-per-second = in/s
unit-millimeter-per-day = mm/d
unit-millimeter-per-second = mm/s
unit-nautical-mile = nmi
unit-fluid-ounce = fl. oz.
unit-ccf = CCF
unit-mcf = MCF
unit-microgram = µg
unit-stone = st
unit-btu-per-hour-square-foot = BTU/(h⋅ft²)
unit-gram-per-cubic-meter = g/m³
unit-var = var
unit-kilovar = kvar
unit-millivar = mvar
unit-varh = varh
unit-kvarh = kvarh
unit-square-meter = m²
unit-square-centimeter = cm²
unit-square-millimeter = mm²
unit-square-kilometer = km²
unit-square-inch = in²
unit-square-foot = ft²
unit-square-yard = yd²
unit-square-mile = mi²
unit-acre = ac
unit-hectare = ha
unit-bit-per-second = bit/s
unit-kilobit-per-second = kbit/s
unit-megabit-per-second = Mbit/s
unit-gigabit-per-second = Gbit/s
unit-byte-per-second = B/s
unit-kilobyte-per-second = kB/s
unit-megabyte-per-second = MB/s
unit-gigabyte-per-second = GB/s
unit-kibibyte-per-second = KiB/s
unit-mebibyte-per-second = MiB/s
unit-gibibyte-per-second = GiB/s
unit-byte = B
unit-bit = bit
unit-kilobit = kbit
unit-megabit = Mbit
unit-gigabit = Gbit
unit-kilobyte = kB
unit-megabyte = MB
unit-gigabyte = GB
unit-terabyte = TB
unit-petabyte = PB
unit-kibibyte = KiB
unit-mebibyte = MiB
unit-gibibyte = GiB
unit-tebibyte = TiB
unit-pebibyte = PiB
unit-millimeter-per-hour = mm/h
unit-cubic-meter-per-hour = m³/h
unit-cubic-meter-per-second = m³/s
unit-cubic-foot-per-minute = ft³/min
unit-liter-per-hour = L/h
unit-liter-per-minute = L/min
unit-liter-per-second = L/s
unit-gallon-per-minute = gal/min
unit-milliliter-per-second = mL/s
unit-microsiemens-per-centimeter = µS/cm
unit-millisiemens-per-centimeter = mS/cm
unit-siemens-per-centimeter = S/cm
unit-decibel-milliwatt = dBm
unit-decibel = dB
unit-a-weighted-decibel = dBA
unit-degree = °
unit-radian = rad
unit-milligram-per-deciliter = mg/dL
unit-millimole-per-liter = mmol/L
node-text-to-text-prop-rounding-title = Rounding
node-text-to-text-prop-rounding-description = Use the project rule or choose a rounding mode.
node-text-to-text-prop-rounding-option-project = Project rule
node-text-to-text-prop-rounding-option-significant = Significant digits
node-text-to-text-prop-rounding-option-decimals = Decimal places

# Currency quantities and additional sensor scales.
type-quantity-currency-uah-title = Ukrainian hryvnia
type-quantity-currency-uah-description = Money in UAH; currencies never convert
unit-uah = UAH
type-quantity-currency-usd-title = US dollar
type-quantity-currency-usd-description = Money in USD; currencies never convert
unit-usd = USD
type-quantity-currency-eur-title = Euro
type-quantity-currency-eur-description = Money in EUR; currencies never convert
unit-eur = EUR
type-quantity-currency-gbp-title = Pound sterling
type-quantity-currency-gbp-description = Money in GBP; currencies never convert
unit-gbp = GBP
type-quantity-currency-pln-title = Polish zloty
type-quantity-currency-pln-description = Money in PLN; currencies never convert
unit-pln = PLN
type-quantity-currency-chf-title = Swiss franc
type-quantity-currency-chf-description = Money in CHF; currencies never convert
unit-chf = CHF
type-quantity-currency-czk-title = Czech koruna
type-quantity-currency-czk-description = Money in CZK; currencies never convert
unit-czk = CZK
type-quantity-currency-huf-title = Hungarian forint
type-quantity-currency-huf-description = Money in HUF; currencies never convert
unit-huf = HUF
type-quantity-currency-ron-title = Romanian leu
type-quantity-currency-ron-description = Money in RON; currencies never convert
unit-ron = RON
type-quantity-currency-bgn-title = Bulgarian lev
type-quantity-currency-bgn-description = Money in BGN; currencies never convert
unit-bgn = BGN
type-quantity-currency-sek-title = Swedish krona
type-quantity-currency-sek-description = Money in SEK; currencies never convert
unit-sek = SEK
type-quantity-currency-nok-title = Norwegian krone
type-quantity-currency-nok-description = Money in NOK; currencies never convert
unit-nok = NOK
type-quantity-currency-dkk-title = Danish krone
type-quantity-currency-dkk-description = Money in DKK; currencies never convert
unit-dkk = DKK
type-quantity-currency-isk-title = Icelandic krona
type-quantity-currency-isk-description = Money in ISK; currencies never convert
unit-isk = ISK
type-quantity-currency-try-title = Turkish lira
type-quantity-currency-try-description = Money in TRY; currencies never convert
unit-try = TRY
type-quantity-currency-gel-title = Georgian lari
type-quantity-currency-gel-description = Money in GEL; currencies never convert
unit-gel = GEL
type-quantity-currency-mdl-title = Moldovan leu
type-quantity-currency-mdl-description = Money in MDL; currencies never convert
unit-mdl = MDL
type-quantity-currency-kzt-title = Kazakhstani tenge
type-quantity-currency-kzt-description = Money in KZT; currencies never convert
unit-kzt = KZT
type-quantity-currency-ils-title = Israeli new shekel
type-quantity-currency-ils-description = Money in ILS; currencies never convert
unit-ils = ILS
type-quantity-currency-jpy-title = Japanese yen
type-quantity-currency-jpy-description = Money in JPY; currencies never convert
unit-jpy = JPY
type-quantity-currency-cny-title = Chinese yuan
type-quantity-currency-cny-description = Money in CNY; currencies never convert
unit-cny = CNY
type-quantity-currency-krw-title = South Korean won
type-quantity-currency-krw-description = Money in KRW; currencies never convert
unit-krw = KRW
type-quantity-currency-inr-title = Indian rupee
type-quantity-currency-inr-description = Money in INR; currencies never convert
unit-inr = INR
type-quantity-currency-cad-title = Canadian dollar
type-quantity-currency-cad-description = Money in CAD; currencies never convert
unit-cad = CAD
type-quantity-currency-aud-title = Australian dollar
type-quantity-currency-aud-description = Money in AUD; currencies never convert
unit-aud = AUD
type-quantity-currency-nzd-title = New Zealand dollar
type-quantity-currency-nzd-description = Money in NZD; currencies never convert
unit-nzd = NZD
type-quantity-currency-sgd-title = Singapore dollar
type-quantity-currency-sgd-description = Money in SGD; currencies never convert
unit-sgd = SGD
type-quantity-currency-hkd-title = Hong Kong dollar
type-quantity-currency-hkd-description = Money in HKD; currencies never convert
unit-hkd = HKD
type-quantity-currency-brl-title = Brazilian real
type-quantity-currency-brl-description = Money in BRL; currencies never convert
unit-brl = BRL
type-quantity-currency-mxn-title = Mexican peso
type-quantity-currency-mxn-description = Money in MXN; currencies never convert
unit-mxn = MXN
type-quantity-currency-zar-title = South African rand
type-quantity-currency-zar-description = Money in ZAR; currencies never convert
unit-zar = ZAR
type-quantity-currency-aed-title = UAE dirham
type-quantity-currency-aed-description = Money in AED; currencies never convert
unit-aed = AED
type-quantity-beaufort-title = Wind force (Beaufort)
type-quantity-beaufort-description = Wind force (Beaufort) in physical units
type-quantity-energy-consumption-title = Energy per distance
type-quantity-energy-consumption-description = Energy per distance in physical units
type-quantity-aqi-title = Air quality index
type-quantity-aqi-description = Air quality index in physical units
type-quantity-ph-title = pH
type-quantity-ph-description = pH in physical units
unit-beaufort = Bft
unit-kilowatt-hour-per-100-kilometers = kWh/100km
unit-watt-hour-per-kilometer = Wh/km
unit-kilometer-per-kilowatt-hour = km/kWh
unit-mile-per-kilowatt-hour = mi/kWh
unit-aqi = AQI
unit-ph = pH
unit-exabyte = EB
unit-zettabyte = ZB
unit-yottabyte = YB
unit-exbibyte = EiB
unit-zebibyte = ZiB
unit-yobibyte = YiB
type-quantity-color-temperature-title = Color temperature
type-quantity-color-temperature-description = Color temperature in physical units
unit-mired = mired
unit-signal-milliwatt = mW
unit-signal-watt = W

# Explicit directional conversions
node-convert-title = Convert
node-convert-description = Converts values using explicit, directional conversions
node-convert-help = Connect Value to select its type, then choose To. Only available conversions are listed. Quantity → Number expresses the value in Unit; Number → Quantity interprets the number in Unit. Some conversions are one-way: color temperature → RGB exists, RGB → color temperature does not. Speed ↔ Beaufort and signal strength ↔ power convert between quantities. Date and time → Time uses the server local time zone; Unix time and durations use seconds. Tick durations have no seconds conversion. Missing, invalid or non-finite values produce no result.
node-convert-pin-Value = Value
node-convert-pin-Result = Result
node-convert-prop-value_type-title = Value type
node-convert-prop-value_type-description = Declared input type; connecting a wire adapts it.
node-convert-prop-target-title = To
node-convert-prop-target-description = Only conversions available for the input type are listed.
node-convert-prop-unit-title = Unit
node-convert-prop-unit-description = Unit used to interpret or express the number.
node-convert-prop-target-option-number = Number in unit
node-convert-prop-target-option-beaufort = Beaufort force
node-convert-prop-target-option-speed = Speed
node-convert-prop-target-option-power = Power
node-convert-prop-target-option-signal-strength = Signal strength
node-convert-prop-target-option-color = Color (RGB)
node-convert-prop-target-option-time = Time of day
node-convert-prop-target-option-unix-seconds = Unix time (seconds)
node-convert-prop-target-option-datetime = Date and time from Unix seconds
node-convert-prop-target-option-seconds = Seconds
node-convert-prop-target-option-duration = Duration from seconds

# Hysteresis
node-logic-hysteresis-title = Hysteresis
node-logic-hysteresis-description = Switches a state using separate on and off thresholds
node-logic-hysteresis-help =
    On In, evaluates Value against On threshold and Off threshold. Rising mode turns on strictly above On threshold and off strictly below Off threshold. For example, allow charging above 230 V and forbid it below 200 V. Falling mode reverses the comparisons: heating turns on below 19 °C and off above 21 °C.

    Between thresholds, including equality, the previous state is kept. The initial state is off, so the first evaluation in the dead band yields off. State lives in memory and resets to off after a restart or node recreation.

    Value and thresholds share a Number or physical quantity type, adapted when Value is connected. Unconnected thresholds can be entered next to their pins with a quantity unit control. All three inputs are required.

    Every evaluation emits State first, then Turned on or Turned off only if the state changed, then Out. Rising mode requires On threshold ≥ Off threshold; falling mode requires On threshold ≤ Off threshold. Invalid thresholds report an error and keep the state, but still emit State and Out.
node-logic-hysteresis-pin-In = In
node-logic-hysteresis-pin-Value = Value
node-logic-hysteresis-pin-on-threshold = On threshold
node-logic-hysteresis-pin-off-threshold = Off threshold
node-logic-hysteresis-pin-turned-on = Turned on
node-logic-hysteresis-pin-turned-off = Turned off
node-logic-hysteresis-pin-Out = Out
node-logic-hysteresis-pin-State = State
node-logic-hysteresis-prop-mode-title = Mode
node-logic-hysteresis-prop-mode-description = Whether rising or falling values turn the state on
node-logic-hysteresis-prop-mode-option-rising = Rising
node-logic-hysteresis-prop-mode-option-falling = Falling

# RS trigger
node-logic-rs-trigger-title = RS Trigger
node-logic-rs-trigger-description = Bistable flip-flop with pulse and level inputs
node-logic-rs-trigger-help =
    Bistable trigger (flip-flop) that holds a boolean state.

    Exec inputs provide pulse-driven control: Set turns the state on, Reset turns it off, and Toggle inverts the current state.

    Data inputs S (Set) and R (Reset) provide level-driven control: they are evaluated whenever the node runs on any exec input. A pulse at Evaluate re-evaluates S and R without an accompanying Set, Reset, or Toggle pulse. While S is true and R is false, the trigger turns on. While R is true and S is false, it turns off. When neither S nor R is true, the trigger latches and keeps its previous state.

    When both Set and Reset are active at the same time — whether both level inputs S and R are true, or Set and Reset exec pulses arrive in the same tick — the Priority property determines the winning state (Reset by default, or Set).

    The Initial state property chooses whether the trigger starts in the off (default) or on state after creation or server restart. State lives in memory and is not persisted across restarts.

    Every evaluation emits State first, then Turned on or Turned off only if the state changed, then Out.
node-logic-rs-trigger-pin-Set = Set
node-logic-rs-trigger-pin-Reset = Reset
node-logic-rs-trigger-pin-Toggle = Toggle
node-logic-rs-trigger-pin-S = S
node-logic-rs-trigger-pin-R = R
node-logic-rs-trigger-pin-Evaluate = Evaluate
node-logic-rs-trigger-pin-State = State
node-logic-rs-trigger-pin-turned-on = Turned on
node-logic-rs-trigger-pin-turned-off = Turned off
node-logic-rs-trigger-pin-Out = Out
node-logic-rs-trigger-prop-priority-title = Priority
node-logic-rs-trigger-prop-priority-description = Which state wins when both set and reset are active at the same time
node-logic-rs-trigger-prop-priority-option-reset = Reset
node-logic-rs-trigger-prop-priority-option-set = Set
node-logic-rs-trigger-prop-initial-title = Initial state
node-logic-rs-trigger-prop-initial-description = State of the trigger after creation or restart
node-logic-rs-trigger-prop-initial-option-off = Off
node-logic-rs-trigger-prop-initial-option-on = On

# text/template
node-text-template-title = Text from Template
node-text-template-description = Builds text from a template, substituting input values
node-text-template-help =
    Formats a MiniJinja template. Undeclared variables become optional Any inputs in order of first appearance; their pin IDs survive edits, and renaming a single variable in place keeps its pin. A text.name reference also creates the name input. The name text is reserved for the global presentation map and cannot be an input; do not assign to it in templates. Missing inputs and their text entries are undefined: { "{" }{ "{" } name | default('—') { "}" }{ "}" } supplies a fallback.

    Examples: Power: { "{" }{ "{" } text.power { "}" }{ "}" }; { "{" }% if power > 1000 %{ "}" }high{ "{" }% endif %{ "}" }; { "{" }{ "{" } (power / 1000) | round(1) { "}" }{ "}" } kW; { "{" }% for item in items %{ "}" }{ "{" }{ "{" } item { "}" }{ "}" } { "{" }% endfor %{ "}" }; { "{" }{ "{" } name | default('—') { "}" }{ "}" }.

    Date-times and times: { "{" }{ "{" } when | date { "}" }{ "}" } uses the formatting language's default, { "{" }{ "{" } when | date('DD.MM.YYYY HH:mm') { "}" }{ "}" } takes a readable pattern (YYYY, MM, MMMM month name, DD, ddd/dddd weekday, HH, hh, mm, ss, A; text in [brackets] is kept), presets 'date', 'time', 'datetime', 'long' and 'iso' are available, and { "{" }{ "{" } when | strftime('%d.%m.%Y') { "}" }{ "}" } accepts chrono strftime formats. Date-times are shown in the server time zone. Plural forms: { "{" }{ "{" } count | plural('file', 'files') { "}" }{ "}" } selects text using CLDR cardinal rules for the engine language (positional categories in integer order or keywords zero/one/two/few/many/other), replacing # with the formatted number. Current time, relative time and duration: { "{" }{ "{" } now() { "}" }{ "}" } returns the current instant, { "{" }{ "{" } when | ago { "}" }{ "}" } renders relative time such as '5 minutes ago', and { "{" }{ "{" } elapsed | duration { "}" }{ "}" } formats wall-clock durations using 'short', 'long' or 'H:mm' patterns. Physical quantities and numbers: { "{" }{ "{" } power | unit { "}" }{ "}" } formats using project display rules and the input's declared type (e.g. 1.97 kW), { "{" }{ "{" } power | unit('W') { "}" }{ "}" } fixes the unit symbol, and keyword arguments digits, decimals or units=false override rounding or hide the unit.

    Inputs are plain Jinja values, so arithmetic, comparisons, tests and standard filters work normally. The global text map formats each input exactly like To text, using the connected source's declared type, project display rules and engine formatting language: { "{" }{ "{" } text.power { "}" }{ "}" } can render 1.2 kW. Lists and structures retain their declared element and field types in this presentation. Units come from text.name.

    The text filter formats runtime kinds only, including values computed in the template: numbers use plain-number display rules, booleans use localized words, lists and maps are formatted recursively, and strings stay unchanged. It has no source type or quantity units: { "{" }{ "{" } (power / 1000) | text { "}" }{ "}" } formats a plain number. Globals and filters have separate namespaces, so both text.name and | text are available.

    Numbers retain integer/float precision; date-times use RFC 3339, and time, duration and color use canonical text. Structures expose field names. Tag lines selects what happens to the line of a { "{" }% %{ "}" } tag: keep preserves tag lines as-is, while trim removes the line break after a tag and spaces before it. Pure node: evaluated when Text is needed. Rendering has a fixed 10,000 instruction fuel budget and the default recursion limit. A rendering error produces no output and reports a node error.
node-text-template-pin-Text = Text
node-text-template-prop-template-title = Template
node-text-template-prop-template-description = MiniJinja template; variables become input pins.
node-text-template-prop-blocks-title = Tag lines
node-text-template-prop-blocks-description = Trim removes the line break after a { "{" }% %{ "}" } tag and the spaces before it
node-text-template-prop-blocks-option-keep = Keep
node-text-template-prop-blocks-option-trim = Trim
error-object-unknown-type = Unknown struct type: { $type }
error-template-syntax = Invalid template syntax: { $message }
error-template-render = Template rendering failed: { $message }
error-formula-too-large = Formula source length ({ $len } bytes) exceeds limit of { $max } bytes
error-formula-depth-exceeded = Formula nesting depth exceeds limit of { $max }
error-formula-unexpected-token = Unexpected token '{ $token }'
error-formula-unclosed-paren = Unclosed parenthesis in formula
error-formula-unknown-function = Unknown function '{ $name }'
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
error-variable-missing = Variable "{ $name }" not found

# time/now
node-time-now-title = Now
node-time-now-description = Current date-time and local time of day
node-time-now-help =
    Outputs the current date-time in UTC on Now and the current time of day in the server local time zone on Time of day.

    Useful as a time source for comparisons, conditions, and calculations.

    Pure node: evaluated whenever its outputs are pulled by another node.
node-time-now-pin-Now = Now
node-time-now-pin-time-of-day = Time of day

# time/difference
node-time-difference-title = Time Difference
node-time-difference-description = Duration between two date-times or times of day
node-time-difference-help =
    Computes the difference between two moments: Result = A − B.

    Works with both date-times and times of day. Connecting a wire to A or B adapts the inputs to that type. When comparing times of day, the calculation is a plain difference without wrapping around midnight, so 10:00 − 14:00 yields −4 hours.

    Pure node: evaluated whenever its output is pulled by another node.
node-time-difference-pin-A = A
node-time-difference-pin-B = B
node-time-difference-pin-Result = Result

# time/shift
node-time-shift-title = Shift Time
node-time-shift-description = Shifts a date-time or time of day by a duration
node-time-shift-help =
    Shifts Value forward or backward by Duration: Result = Value + Duration (or − Duration).

    Works with both date-times and times of day. Connecting a wire to Value adapts the input and output to that type. Shifting a time of day wraps around midnight, so 23:00 + 2 hours yields 01:00. Date-time arithmetic checks for overflow and reports an error if the result exceeds valid date boundaries.

    Pure node: evaluated whenever its output is pulled by another node.
node-time-shift-pin-Value = Value
node-time-shift-pin-Duration = Duration
node-time-shift-pin-Result = Result
node-time-shift-prop-operation-title = Operation
node-time-shift-prop-operation-description = Whether to add or subtract duration
node-time-shift-prop-operation-option-add = Add
node-time-shift-prop-operation-option-subtract = Subtract

# time/break
node-time-break-title = Break Date-Time
node-time-break-description = Extracts date and time components in the local time zone
node-time-break-help =
    Breaks a date-time into components in the server local time zone: Year, Month (1–12), Day (1–31), Weekday (1 = Monday … 7 = Sunday), Hour (0–23), Minute (0–59), Second (0–59), local Time of day, and Day of year (1–366).

    For example, 2026-03-15 14:30:00 yields Year = 2026, Month = 3, Day = 15, Weekday = 7 (Sunday), Hour = 14, Minute = 30, Second = 0, Time of day = 14:30:00, and Day of year = 74.

    Pure node: evaluated whenever any of its outputs are pulled by another node.
node-time-break-pin-Date-time = Date-time
node-time-break-pin-Year = Year
node-time-break-pin-Month = Month
node-time-break-pin-Day = Day
node-time-break-pin-Weekday = Weekday
node-time-break-pin-Hour = Hour
node-time-break-pin-Minute = Minute
node-time-break-pin-Second = Second
node-time-break-pin-time-of-day = Time of day
node-time-break-pin-day-of-year = Day of year

# time/start_of
node-time-start-of-title = Start of Period
node-time-start-of-description = Calculates the start of a period for a date-time
node-time-start-of-help =
    Calculates the start of the selected time period (minute, hour, day, week, month, or year) in the server local time zone, returning the result in UTC.

    Week starts on Monday at 00:00:00 local time. Day starts at 00:00:00, Month on the 1st day at 00:00:00, and Year on January 1st at 00:00:00. For ambiguous or nonexistent local times due to daylight saving time transitions, the earliest valid instant is chosen, stepping forward minute by minute if needed.

    Pure node: evaluated whenever its output is pulled by another node.
node-time-start-of-pin-Date-time = Date-time
node-time-start-of-pin-Result = Result
node-time-start-of-prop-unit-title = Unit
node-time-start-of-prop-unit-description = Time period to find the start of
node-time-start-of-prop-unit-option-minute = Minute
node-time-start-of-prop-unit-option-hour = Hour
node-time-start-of-prop-unit-option-day = Day
node-time-start-of-prop-unit-option-week = Week
node-time-start-of-prop-unit-option-month = Month
node-time-start-of-prop-unit-option-year = Year

# time/in_range
node-time-in-range-title = Time of Day in Range
node-time-in-range-description = Checks whether a time of day falls between From and To
node-time-in-range-help =
    Checks whether the local time of day falls within the range from From to To: From ≤ t < To.

    When From > To, the range crosses midnight (t ≥ From or t < To), for example 22:00 to 06:00 matches night hours. When From == To, the range is empty and always yields false. If Value is unconnected, the current time from the engine clock is used.

    Pure node: evaluated whenever its output is pulled by another node.
node-time-in-range-pin-Value = Value
node-time-in-range-pin-From = From
node-time-in-range-pin-To = To
node-time-in-range-pin-in-range = In range

error-time-shift-overflow = Date-time shift arithmetic overflowed

# Core media data types

type-media-photo-title = Photo
type-media-photo-description = Photographic image reference
type-media-photo-field-url = URL
type-media-photo-field-file_id = File ID
type-media-photo-field-mime_type = MIME type
type-media-photo-field-file_name = File name
type-media-photo-field-size = Size
type-media-photo-field-width = Width
type-media-photo-field-height = Height
type-media-photo-field-duration = Duration
type-media-photo-field-blob = Blob

type-media-audio-title = Audio
type-media-audio-description = Audio recording reference
type-media-audio-field-url = URL
type-media-audio-field-file_id = File ID
type-media-audio-field-mime_type = MIME type
type-media-audio-field-file_name = File name
type-media-audio-field-size = Size
type-media-audio-field-width = Width
type-media-audio-field-height = Height
type-media-audio-field-duration = Duration
type-media-audio-field-blob = Blob

type-media-video-title = Video
type-media-video-description = Video recording reference
type-media-video-field-url = URL
type-media-video-field-file_id = File ID
type-media-video-field-mime_type = MIME type
type-media-video-field-file_name = File name
type-media-video-field-size = Size
type-media-video-field-width = Width
type-media-video-field-height = Height
type-media-video-field-duration = Duration
type-media-video-field-blob = Blob

type-media-document-title = File
type-media-document-description = Document or generic file reference
type-media-document-field-url = URL
type-media-document-field-file_id = File ID
type-media-document-field-mime_type = MIME type
type-media-document-field-file_name = File name
type-media-document-field-size = Size
type-media-document-field-width = Width
type-media-document-field-height = Height
type-media-document-field-duration = Duration
type-media-document-field-blob = Blob

error-media-missing-source = Media object must have either url, file_id or blob set

