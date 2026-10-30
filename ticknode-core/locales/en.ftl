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
node-variable-get-title = Get my_var
node-variable-get-description = Returns the value of a variable by name
node-variable-get-help =
    Reads the global variable named in the "Variable" property and outputs it on Val. Variables are shared across the entire graph and written by the Set node. Until the variable is written, outputs 0.

    Pure node: has no exec pins and is evaluated whenever its output is needed by another node.
node-variable-get-pin-Val = Val

# variable/set
node-variable-set-title = Set my_var
node-variable-set-description = Writes a value to a variable
node-variable-set-help =
    On a signal at In, writes the value from Val to the global variable (named in the "Variable" property) and passes execution to Out.

    The variable is visible to all Get nodes with the same name across the entire graph.
node-variable-set-pin-In = In
node-variable-set-pin-Out = Out
node-variable-set-pin-Val = Val

# match
node-match-title = Match
node-match-description = Selects an output by conditions, like a decider combinator
node-match-help =
    On a signal at In, checks cases from top to bottom and passes execution to the output of the first matching case. If none match — to Else. With "All matches" enabled, all matching cases fire.

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
node-sleep-description = Delays execution by the specified number of milliseconds or ticks
node-sleep-help =
    A signal at In starts a delay, and when it elapses, emits a signal on Out.

    • Delay — duration in milliseconds (rounded up to whole ticks).
    • UseTicks = true — Delay is counted in ticks.
    • TickDuration — tick duration in ms for calculation (defaults to the server tick).

    Each signal at In starts its own independent delay, like Queue Delay.
node-sleep-pin-In = In
node-sleep-pin-Delay = Delay
node-sleep-pin-UseTicks = UseTicks
node-sleep-pin-TickDuration = TickDuration
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
