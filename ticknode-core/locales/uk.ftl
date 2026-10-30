# Built-in core nodes (texts from NodeDefinition in ticknode-core/src/lib.rs)

# const
node-const-title = Constant
node-const-description = Повертає константне значення
node-const-help =
    Видає на вихід Val значення, задане прямо на ноді (рядок «Значення») або в правій панелі. Тип значення (число, рядок, логічне) обирається в правій панелі, і від нього залежить тип піна.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-const-pin-Val = Val

# math/add
node-math-add-title = Math Add
node-math-add-description = Математична операція над двома числами
node-math-add-help =
    Res = A + B.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем. Ціле число лишається цілим, якщо результат цілий.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-math-add-pin-A = A
node-math-add-pin-B = B
node-math-add-pin-Res = Res

# math/sub
node-math-sub-title = Math Subtract
node-math-sub-description = Математична операція над двома числами
node-math-sub-help =
    Res = A − B.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем. Ціле число лишається цілим, якщо результат цілий.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-math-sub-pin-A = A
node-math-sub-pin-B = B
node-math-sub-pin-Res = Res

# math/mul
node-math-mul-title = Math Multiply
node-math-mul-description = Математична операція над двома числами
node-math-mul-help =
    Res = A × B.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем. Ціле число лишається цілим, якщо результат цілий.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-math-mul-pin-A = A
node-math-mul-pin-B = B
node-math-mul-pin-Res = Res

# math/div
node-math-div-title = Math Divide
node-math-div-description = Математична операція над двома числами
node-math-div-help =
    Res = A ÷ B. Ділення на 0 дає нескінченність.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем. Ціле число лишається цілим, якщо результат цілий.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-math-div-pin-A = A
node-math-div-pin-B = B
node-math-div-pin-Res = Res

# math/expression
node-math-expression-title = Expression
node-math-expression-description = Обчислює довільну математичну формулу
node-math-expression-help =
    Обчислює формулу, задану прямо на ноді (рядок «Формула»), наприклад `(a + b) / 2` або `x * x - 1`.

    Кожна змінна у формулі автоматично стає вхідним піном (в абетковому порядку). Непід'єднана змінна бере значення, задане біля піна, а без нього дорівнює 0. Якщо формула некоректна — результат 0.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-math-expression-pin-Res = Res

# logical/and
node-logical-and-title = Logical And
node-logical-and-description = Логічна операція
node-logical-and-help =
    Res = true, лише якщо і A, і B істинні.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається false.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-logical-and-pin-A = A
node-logical-and-pin-B = B
node-logical-and-pin-Res = Res

# logical/or
node-logical-or-title = Logical Or
node-logical-or-description = Логічна операція
node-logical-or-help =
    Res = true, якщо істинний хоча б один із A, B.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається false.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-logical-or-pin-A = A
node-logical-or-pin-B = B
node-logical-or-pin-Res = Res

# logical/not
node-logical-not-title = Logical Not
node-logical-not-description = Логічна операція
node-logical-not-help =
    Res — протилежне значення до A.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається false.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-logical-not-pin-A = A
node-logical-not-pin-Res = Res

# variable/get
node-variable-get-title = Get my_var
node-variable-get-description = Повертає значення змінної за її назвою
node-variable-get-help =
    Читає глобальну змінну з назвою з властивості «Змінна» і видає її на Val. Змінні спільні для всього графа, записує їх нода Set. Поки змінну не записано — видає 0.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-variable-get-pin-Val = Val

# variable/set
node-variable-set-title = Set my_var
node-variable-set-description = Записує значення у змінну
node-variable-set-help =
    За сигналом на In записує значення з Val у глобальну змінну (назва — у властивості «Змінна») і передає виконання на Out.

    Змінну бачать усі ноди Get з тією ж назвою в усьому графі.
node-variable-set-pin-In = In
node-variable-set-pin-Out = Out
node-variable-set-pin-Val = Val

# match
node-match-title = Match
node-match-description = Обирає вихід за умовами, як комбінатор-вирішувач
node-match-help =
    За сигналом на In перевіряє випадки згори вниз і передає виконання на вихід першого, що збігся. Якщо не збігся жоден — на Else. З прапорцем «Усі збіги» спрацьовують усі випадки, що збіглись.

    Входи: «+ вхід» додає вхідний пін. Ім'я входу можна змінити (умови оновляться), тип — обрати; «auto» приймає будь-що, і тоді оператори підбираються за типом під'єднаного джерела. Непід'єднаному входу можна задати значення прямо на ноді.

    Умова — «вхід  оператор  операнд». Оператори залежать від типу входу:
    • числа, час, тривалість — =, ≠, <, ≤, >, ≥ (для часу — раніше / пізніше);
    • рядки — дорівнює, містить, починається з, закінчується на, порожній, довжина;
    • логічні — = true / = false;
    • списки — містить, порожній, довжина;
    • об'єкти — має поле;
    • будь-який — є значення / немає значення.
    Праворуч — значення або інший вхід (так порівнюються дві змінні). Значення приводиться до типу входу: «07:30» — час, «1h 30m» або 90 — тривалість, «#ff0000» — колір.

    Умови з'єднуються «і» / «або»; «і» зв'язує сильніше: «a > 1 і b > 1 або c» = «(a > 1 і b > 1) або c». Випадок без умов спрацьовує завжди. Вхід без значення умову не виконує (крім «немає значення»).
node-match-pin-In = In
node-match-pin-Else = Else

# clock
node-clock-title = Clock
node-clock-description = Генерує імпульс виконання кожні interval_ticks тактів
node-clock-help =
    Запускає виконання сама: кожні N тактів (властивість «Інтервал») видає сигнал на Out.

    Такт сервера — 100 мс, тож 10 тактів ≈ 1 секунда. Це основне джерело подій: з нього починаються ланцюжки, що мають працювати постійно.
node-clock-pin-Out = Out

# console
node-console-title = Console Log
node-console-description = Виводить значення в консоль при отриманні сигналу виконання
node-console-help =
    За сигналом на In виводить значення Val у консоль редактора й у лог сервера. Без під'єднаного Val виводить «None».

    Виходу немає — це кінцева нода ланцюжка. Щоб вивести значення посеред ланцюжка, використайте Debug Log.
node-console-pin-In = In
node-console-pin-Val = Val

# test/click_button
node-test-click-button-title = Click Button
node-test-click-button-description = Генерує exec сигнал на наступному тіку після того як користувач клікне на кнопку
node-test-click-button-help =
    Видає сигнал на Out, коли ви натискаєте «Запустити» в правій панелі.

    Зручно, щоб вручну запустити ланцюжок під час налагодження.
node-test-click-button-pin-Out = Out

# test/debug
node-test-debug-title = Debug Log
node-test-debug-description = Приймає будь-які дані та виводить їх у лог з текстовим описом
node-test-debug-help =
    За сигналом на In виводить у консоль «Префікс» (з правої панелі) разом зі значенням Val і передає виконання далі на Out.

    Її можна вставити посеред ланцюжка, щоб подивитися, яке значення через нього проходить.
node-test-debug-pin-In = In
node-test-debug-pin-Val = Val
node-test-debug-pin-Out = Out

# compare
node-compare-title = Compare
node-compare-description = Порівнює два числа
node-compare-help =
    Порівнює числа A і B:
    • Eq — A дорівнює B (з точністю до 1e-9);
    • Gt — A більше за B;
    • Lt — A менше за B.

    Непід'єднаний вхід бере значення, задане біля піна на ноді, а без нього вважається нулем.

    Для розгалуження виконання за умовою зручніша нода Match.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-compare-pin-A = A
node-compare-pin-B = B
node-compare-pin-Eq = Eq
node-compare-pin-Gt = Gt
node-compare-pin-Lt = Lt

# for_loop
node-for-loop-title = For Loop
node-for-loop-description = Асинхронний цикл від 0 до Count-1 з кроком 1
node-for-loop-help =
    Покроковий цикл:
    1. Сигнал на In починає цикл з Index = 0 і видає сигнал на LoopBody.
    2. Коли тіло циклу відпрацювало, подайте сигнал на Next — Index збільшиться на 1, і LoopBody спрацює знову.
    3. Після Count ітерацій або сигналу на Break спрацьовує Completed.

    Якщо Count ≤ 0, одразу спрацьовує Completed. Кожен крок займає щонайменше такт, тож цикл не блокує рушій.
node-for-loop-pin-In = In
node-for-loop-pin-Count = Count
node-for-loop-pin-Break = Break
node-for-loop-pin-Next = Next
node-for-loop-pin-LoopBody = LoopBody
node-for-loop-pin-Index = Index
node-for-loop-pin-Completed = Completed

# sleep
node-sleep-title = Sleep
node-sleep-description = Затримує виконання на вказану кількість мілісекунд або тактів
node-sleep-help =
    Сигнал на In запускає затримку, а коли вона мине — видає сигнал на Out.

    • Delay — тривалість у мілісекундах (округлюється вгору до цілих тактів).
    • UseTicks = true — Delay рахується в тактах.
    • TickDuration — тривалість такту в мс для розрахунку (типово — такт сервера).

    Кожен сигнал на In запускає власну затримку, як у Queue Delay.
node-sleep-pin-In = In
node-sleep-pin-Delay = Delay
node-sleep-pin-UseTicks = UseTicks
node-sleep-pin-TickDuration = TickDuration
node-sleep-pin-Out = Out

# list/create
node-list-create-title = Create List
node-list-create-description = Створює порожній список
node-list-create-help =
    Видає порожній список. Тип елементів обирається в правій панелі.

    Далі список наповнюють нодою List Add.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-list-create-pin-List = List

# list/add
node-list-add-title = List Add
node-list-add-description = Додає елемент до списку
node-list-add-help =
    Видає OutList — копію списку List з елементом Item у кінці. Вхідний список не змінюється.

    Тип елементів обирається в правій панелі й має збігатися з типом списку.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-list-add-pin-List = List
node-list-add-pin-Item = Item
node-list-add-pin-OutList = OutList

# list/remove
node-list-remove-title = List Remove
node-list-remove-description = Вилучає елемент зі списку за індексом
node-list-remove-help =
    Видає OutList — копію списку без елемента з номером Index (рахунок від 0). Якщо такого номера немає, список повертається без змін.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-list-remove-pin-List = List
node-list-remove-pin-Index = Index
node-list-remove-pin-OutList = OutList

# list/get
node-list-get-title = List Get
node-list-get-description = Отримує елемент зі списку за індексом
node-list-get-help =
    Видає Item — елемент списку з номером Index (рахунок від 0). Якщо такого номера немає, вихід не має значення.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-list-get-pin-List = List
node-list-get-pin-Index = Index
node-list-get-pin-Item = Item

# list/length
node-list-length-title = List Length
node-list-length-description = Повертає довжину списку
node-list-length-help =
    Видає Len — кількість елементів у списку. Без під'єднаного списку — 0.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-list-length-pin-List = List
node-list-length-pin-Len = Len

# object/create
node-object-create-title = Create Object
node-object-create-description = Створює порожній об'єкт
node-object-create-help =
    Видає порожній об'єкт — набір іменованих полів.

    Поля додають нодою Set Object Field.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-object-create-pin-Obj = Obj

# object/set_field
node-object-set-field-title = Set Object Field
node-object-set-field-description = Встановлює значення поля об'єкта
node-object-set-field-help =
    Видає OutObj — копію об'єкта Obj, у якій поле Name має значення Val. Якщо поля не було, воно додається, інакше — замінюється.

    Тип Val обирається в правій панелі. Поки не під'єднано всі входи, результату немає.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-object-set-field-pin-Obj = Obj
node-object-set-field-pin-Name = Name
node-object-set-field-pin-Val = Val
node-object-set-field-pin-OutObj = OutObj

# object/get_field
node-object-get-field-title = Get Object Field
node-object-get-field-description = Отримує значення поля об'єкта
node-object-get-field-help =
    Шукає в об'єкті Obj поле Name:
    • Val — значення поля;
    • Found — чи знайдено поле.

    Тип Val обирається в правій панелі.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-object-get-field-pin-Obj = Obj
node-object-get-field-pin-Name = Name
node-object-get-field-pin-Val = Val
node-object-get-field-pin-Found = Found

# object/unpack
node-object-unpack-title = Unpack Object
node-object-unpack-description = Розпаковує поля об'єкта
node-object-unpack-help =
    Видає кілька полів об'єкта одразу. Поля перелічують на ноді або в правій панелі через кому у вигляді «назва:тип», наприклад `temp:number, city:string` (без типу — string).

    Кожне поле стає окремим вихідним піном. Поле, якого немає в об'єкті, не має значення.

    Чиста нода: не має exec-пінів і обчислюється щоразу, коли її результат потрібен іншій ноді.
node-object-unpack-pin-Obj = Obj
