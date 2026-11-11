# Built-in core nodes

# const
node-const-title = Constant
node-const-description = Returns a constant value
node-const-help =
    Emits the value set directly on the node ("Value" row) or in the right panel on output Val. The value type (number, string, boolean, etc.) is chosen on the node, and the pin type follows it.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-const-pin-Val = Val

# math/add
node-math-add-title = Math Add
node-math-add-description = Arithmetic operation on two numbers
node-math-add-help =
    Res = A + B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-add-pin-A = A
node-math-add-pin-B = B
node-math-add-pin-Res = Res

# math/sub
node-math-sub-title = Math Subtract
node-math-sub-description = Arithmetic operation on two numbers
node-math-sub-help =
    Res = A − B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-sub-pin-A = A
node-math-sub-pin-B = B
node-math-sub-pin-Res = Res

# math/mul
node-math-mul-title = Math Multiply
node-math-mul-description = Arithmetic operation on two numbers
node-math-mul-help =
    Res = A × B.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-mul-pin-A = A
node-math-mul-pin-B = B
node-math-mul-pin-Res = Res

# math/div
node-math-div-title = Math Divide
node-math-div-description = Arithmetic operation on two numbers
node-math-div-help =
    Res = A ÷ B. Division by 0 yields infinity.

    Connecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.

    An unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-div-pin-A = A
node-math-div-pin-B = B
node-math-div-pin-Res = Res

# math/expression
node-math-expression-title = Expression
node-math-expression-description = Evaluates an arbitrary mathematical formula
node-math-expression-help =
    Evaluates the formula set directly on the node ("Formula" row), such as `(a + b) / 2` or `x * x - 1`.

    Each variable in the formula automatically becomes an input pin (in alphabetical order). An unconnected variable uses the value set next to the pin, or defaults to 0. If the formula is invalid, the result is 0.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-math-expression-pin-Res = Res

# logical/and
node-logical-and-title = Logical And
node-logical-and-description = Logical operation
node-logical-and-help =
    Res = true only if both A and B are true.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-and-pin-A = A
node-logical-and-pin-B = B
node-logical-and-pin-Res = Res

# logical/or
node-logical-or-title = Logical Or
node-logical-or-description = Logical operation
node-logical-or-help =
    Res = true if at least one of A or B is true.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-or-pin-A = A
node-logical-or-pin-B = B
node-logical-or-pin-Res = Res

# logical/not
node-logical-not-title = Logical Not
node-logical-not-description = Logical operation
node-logical-not-help =
    Res is the logical negation of A.

    An unconnected input uses the value set next to the pin on the node, or defaults to false.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-logical-not-pin-A = A
node-logical-not-pin-Res = Res

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
    On a signal at In, prints the value of Val to the editor console and server log. Without Val connected, prints "None".

    Has no output — this is a terminal node in a chain. To log a value in the middle of a chain, use Debug Log.
node-console-pin-In = In
node-console-pin-Val = Val

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
    On a signal at In, logs the "Prefix" together with the value of Val to the console and passes execution to Out.

    Can be inserted in the middle of a chain to inspect the value passing through it.
node-test-debug-pin-In = In
node-test-debug-pin-Val = Val
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
    1. A signal at In starts the loop with Index = 0 and fires LoopBody.
    2. When the loop body finishes, send a signal to Next — Index increments by 1 and LoopBody fires again.
    3. After Count iterations or a signal at Break, Completed fires.

    If Count ≤ 0, Completed fires immediately. Each step takes at least one tick, so the loop never blocks the engine.
node-for-loop-pin-In = In
node-for-loop-pin-Count = Count
node-for-loop-pin-Break = Break
node-for-loop-pin-Next = Next
node-for-loop-pin-LoopBody = LoopBody
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
    Outputs OutObj — a copy of object Obj where field Name has value Val. If the field did not exist, it is added; otherwise it is replaced.

    The type of Val is selected on the node. Until all inputs are provided, no output is produced.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-set-field-pin-Obj = Obj
node-object-set-field-pin-Name = Name
node-object-set-field-pin-Val = Val
node-object-set-field-pin-OutObj = OutObj

# object/get_field
node-object-get-field-title = Get Object Field
node-object-get-field-description = Gets the value of an object field
node-object-get-field-help =
    Looks up field Name in object Obj:
    • Val — the field value;
    • Found — whether the field exists.

    The type of Val is selected on the node.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-get-field-pin-Obj = Obj
node-object-get-field-pin-Name = Name
node-object-get-field-pin-Val = Val
node-object-get-field-pin-Found = Found

# object/unpack
node-object-unpack-title = Unpack Object
node-object-unpack-description = Unpacks fields of an object
node-object-unpack-help =
    Outputs several object fields at once. Fields are listed on the node or in the right panel separated by commas as "name:type", for example `temp:number, city:string` (without a type — string).

    Each field becomes a separate output pin. A field missing from the object has no value.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-object-unpack-pin-Obj = Obj

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


node-text-to-text-title = To text
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
