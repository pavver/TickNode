# Ukrainian translations of the string plugin.

node-string-concat-title = Об'єднання рядків
node-string-concat-description = Об'єднує два текстові рядки
node-string-concat-help =
    Result — рядок A, одразу за яким іде B.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-concat-pin-In = In
node-string-concat-pin-A = A
node-string-concat-pin-B = B
node-string-concat-pin-Out = Out
node-string-concat-pin-Result = Результат

node-string-length-title = Довжина рядка
node-string-length-description = Повертає довжину рядка
node-string-length-help =
    Result — кількість символів (не байтів) у рядку Value.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-length-pin-In = In
node-string-length-pin-Value = Значення
node-string-length-pin-Out = Out
node-string-length-pin-Result = Результат

node-string-substring-title = Підрядок
node-string-substring-description = Виділяє підрядок із рядка
node-string-substring-help =
    Result — Len символів рядка Value, починаючи з позиції Start (рахунок від 0). Що виходить за межі рядка, відкидається.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-substring-pin-In = In
node-string-substring-pin-Value = Значення
node-string-substring-pin-Start = Початок
node-string-substring-pin-Len = Кількість
node-string-substring-pin-Out = Out
node-string-substring-pin-Result = Результат

node-string-contains-title = Пошук підрядка
node-string-contains-description = Перевіряє, чи містить рядок інший підрядок
node-string-contains-help =
    Result = true, якщо Value містить підрядок Sub. Порожній Sub міститься в будь-якому рядку.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-contains-pin-In = In
node-string-contains-pin-Value = Значення
node-string-contains-pin-Sub = Підрядок
node-string-contains-pin-Out = Out
node-string-contains-pin-Result = Результат

node-string-split-title = Розбиття рядка
node-string-split-description = Розбиває рядок за роздільником
node-string-split-help =
    Result — список частин рядка Value, розбитого за роздільником Pat. Порожній Pat розбиває рядок на окремі символи.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-split-pin-In = In
node-string-split-pin-Value = Значення
node-string-split-pin-Pat = Шаблон
node-string-split-pin-Out = Out
node-string-split-pin-Result = Результат

node-string-regex-title = Регулярний вираз
node-string-regex-description = Перевіряє регулярний вираз та повертає збіг
node-string-regex-help =
    Шукає в Value регулярний вираз Pat (синтаксис Rust regex):
    • Matched — чи знайдено збіг;
    • Capture — перша група в дужках, а якщо груп немає — увесь збіг.

    Некоректний вираз дає Matched = false.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-regex-pin-In = In
node-string-regex-pin-Value = Значення
node-string-regex-pin-Pat = Шаблон
node-string-regex-pin-Out = Out
node-string-regex-pin-Matched = Збіг
node-string-regex-pin-Capture = Захоплення

node-string-aggregate-title = Збирання рядків
node-string-aggregate-description = Об'єднує список рядків за роздільником
node-string-aggregate-help =
    Result — елементи списку List, з'єднані в один рядок через Sep. Нерядкові елементи пропускаються.

    Непід'єднаний рядковий вхід вважається порожнім рядком.

    Виконується за сигналом на In, після чого передає виконання на Out — результат доступний наступним нодам.
node-string-aggregate-pin-In = In
node-string-aggregate-pin-List = Список
node-string-aggregate-pin-Sep = Роздільник
node-string-aggregate-pin-Out = Out
node-string-aggregate-pin-Result = Результат
