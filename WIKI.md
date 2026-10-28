# Alpha Language Official Wiki & Documentation

Welcome to the comprehensive reference manual and documentation for the **Alpha Programming Language**.

- [🇬🇧 English Documentation](#-english-documentation)
- [🇷🇺 Русская Документация (Russian Documentation)](#-русская-документация-russian-documentation)

---

# 🇬🇧 English Documentation

## 1. Syntax Fundamentals
- **Statically Typed**: All variables and functions have explicit or inferred types verified at compile-time.
- **Optional Semicolons**: Semicolons (`;`) are completely optional at the end of statements.
- **Code Blocks**: Blocks are delimited by braces `{ ... }`.
- **Comments**: Single-line comments begin with `//`.

---

## 2. Type System

### 2.1 Primitive Types
- `int`: 64-bit signed integer (`10`, `-42`).
- `unt`: 64-bit unsigned integer (`0` to `18_446_744_073_709_551_615`). Safe unsigned arithmetic, division, modulo, and comparisons.
- `float`: 64-bit IEEE-754 floating-point number (`3.14`, `-0.005`).
- `string`: UTF-8 encoded text (`"Alpha Language"`).
- `char`: Unicode scalar character (`'A'`, `'λ'`).
- `boolean`: Logical `true` or `false`.

### 2.2 Special Types
- `something`: Dynamic / universal container type that can hold any value (similar to `any`).
- `nothing`: Represents the complete absence of a value (similar to `nil` or `void`).
- `hexcolor`: Dedicated 24-bit color literal type declared with `#(...)`, e.g. `#(FFF00)` or `#(FF00AA)`.

### 2.3 Compound Types
- **Vector (`vect<T>`)**: Dynamic growable list of type `T`.
- **Map (`map<K, V>`)**: Key-value hash map associating keys of type `K` with values of type `V`.
- **Tuples (`(T1, T2, ...)` )**: Fixed-size ordered heterogeneous sequence of types.

### 2.4 Union / Multi-variant Types
Return types can express several acceptable variants, e.g., `<int, nothing>`:
```alpha
function find_id(name: string) [public, <int, nothing>] {
    if name == "admin" {
        return 1
    }
    return nothing
}
```

### 2.5 Strict Typing Mode (`@strict`)
Adding `@strict` at the top of a file (or at any point) activates enterprise-grade strict typechecking from that line forward:
1. **Explicit Typing Required**: Every variable must have an explicit type annotation (`var x: int = 10`). Untyped declarations like `var x = 10` are rejected.
2. **Safe Box / Union Handling**: Values wrapped in union/box types must be unpacked with `check` pattern matching. Force unwrap operators (`!?`, `!`) are strictly prohibited!
3. **No `something` or `nothing`**: Dynamic `something` and `nothing` types and literals are forbidden.
4. **Zero Tolerance**: Any violation immediately halts compilation with a `[Strict Mode]` error.

```alpha
@strict

function safe_div(a: int, b: int) [public, <int, string>] {
    if b == 0 {
        return "Division by zero"
    }
    return a / b
}

function main() [public, int] {
    var result: <int, string> = safe_div(10, 2)
    
    // In @strict mode, must handle with check:
    var final_val: int = check result {
        (5) => 5,
        else => 0
    }
    return final_val
}
```

---

## 3. 1-Based Indexing & Slicing

In Alpha, all indexable collections (**vect**, **tuple**, **string**, and numeric-keyed **map**) start counting from **1**.

```alpha
var v: vect<string> = vect.new()
v.push("first")
v.push("second")
v.push("third")

var item1: string = v[1]       // "first"
var item2: string = v[2]       // "second"
v[2] = "updated_second"         // Mutation at 1-based index

var t: (int, string) = (42, "hello")
var num: int = (t[1] -> int)   // 42
var msg: string = (t[2] -> string) // "hello"
```

### Slicing
Slices support both exclusive `..` and inclusive `...` ranges:
```alpha
var text: string = "hello world"
var s1: string = text[1..6]     // "hello" (characters from 1 up to 5)
var s2: string = text[7...11]   // "world" (characters from 7 to 11 inclusive)
var s3: string = text[7..]      // "world" (from 7 to end)
var s4: string = text[..6]      // "hello" (from start up to 6 exclusive)
```

---

## 4. Variables & Constants

- `var`: Mutable variable.
- `const var`: Immutable constant value.

```alpha
var score: int = 100
score = 150 // Allowed

const var MAX_LIVES: int = 3
// MAX_LIVES = 4 // Error: cannot reassign constant
```

---

## 5. Operators & Expressions

- **Arithmetic**: `+`, `-`, `*`, `/`, `%`
- **Compound Assignment**: `+=`, `-=`, `*=`, `/=`, `%=`
- **Relational**: `==`, `!=`, `<`, `<=`, `>`, `>=`
- **Logical**: `&&` (and), `||` (or), `!` (not)
- **Ternary Operator**: `condition ? then_value : else_value`
- **Elvis Operator (Default fallback)**: `val ?: fallback_value`
- **Force Unwrap**: `val!` (throws a runtime error if `val` is `nothing`)
- **Explicit Cast**: `(expression -> target_type)`
- **String Interpolation**: `"Welcome, \{username}! Your score is \{score}."`
- **Multiline Strings**: `""" ... """`

---

## 6. Functions

### Syntax & Metadata
Function attributes are placed in metadata brackets `[visibility, return_type]`:
```alpha
function add(a: int, b: int) [public, int] {
    return a + b
}
```

### Visibility
- `public`: Accessible from any imported module.
- `here`: Private to the defining file.

### Question Marks in Identifiers
Alpha allows `?` anywhere inside function names and identifiers:
```alpha
function is_ready?() [public, boolean] {
    return true
}

function get?user?name() [public, string] {
    return "Alex"
}
```

---

## 7. Control Flow

### If / Else
```alpha
var score: int = 95
if score > 90 {
    print("Grade: A")
} else if score > 75 {
    print("Grade: B")
} else {
    print("Grade: C")
}
```

### Loops
```alpha
// 1. Range loop (1-based)
for i = 1, i <= 10, i++ {
    printf("i = {}\n", i)
}

// 2. Repeat range loop (repeats N times without loop variable)
for 1...5 {
    print("Pinging server...")
}

// 3. Collection iteration (for each)
var names: vect<string> = vect.new()
names.push("Alice")
names.push("Bob")

for each name in names {
    printf("Hello, {}!\n", name)
}

// 4. Map key-value iteration
var scores: map<string, int> = map.new()
scores.insert("Alice", 100)
scores.insert("Bob", 85)

for each user, points in scores {
    printf("{}: {} pts\n", user, points)
}

// 5. Infinite loop with break and continue
var step: int = 1
for {
    if step > 5 { break }
    step++
}
```

---

## 8. Pattern Matching (`check`)

The `check` expression matches values against patterns:
```alpha
var status_code: int = 404

var result: string = check status_code {
    200 => "OK",
    404 => "Not Found",
    500 => "Internal Server Error",
    _   => "Unknown Code"
}
```

Blocks are supported inside branches:
```alpha
function handle_command(command: string) [public, boolean] {
    check command {
        "start" => {
            return true
        },
        "stop" => {
            return false
        },
        _ => {
            return false
        }
    }
}
```

---

## 9. Error Handling (`try ... catch`)

Catch runtime errors safely without crashing the process:
```alpha
try {
    var file: IoFile = Io.open("non_existent.txt")
    print(file.content())
} catch err {
    printf("Handled error: {}\n", err)
}

// Throwing explicit errors
function validate_age(age: int) [public, nothing] {
    if age < 0 {
        error("Age cannot be negative: {}", age)
    }
}
```

---

## 10. Deferred Execution (`make schedule` / Defer)

Guarantees execution of cleanup code when exiting the current scope. Schedules run in **LIFO** (Last In, First Out) order:

```alpha
function process_data(path: string) [public, nothing] {
    make schedule {
        print("1. Releasing lock on file: " + path)
    }
    make schedule (print("2. Closing network stream"))

    // Will execute #2 first, then #1 when leaving this function!
}
```

---

## 11. Assertions (`verify`)

Validates assumptions and invariants. Halts with an error if the condition is false:
```alpha
var divider: int = 5
var buffer: string = "data"

verify divider != 0, "Divider cannot be zero"
verify buffer.len() > 0, "Buffer is empty"
```

---

## 12. Conditional Platform Compilation

Code blocks can be conditionally executed based on the host OS:
```alpha
!system win {
    print("Running on Windows: initializing Win32 APIs")
}

!system posix {
    print("Running on Linux / macOS: initializing POSIX APIs")
}
```

---

## 13. Structures & Object-Oriented Methods

```alpha
structure Player {
    name: string,
    health: int,
    is_alive: boolean,

    function take_damage(dmg: int) [public, nothing] {
        this.health = this.health - dmg
        if this.health <= 0 {
            this.health = 0
            this.is_alive = false
        }
    }

    function create(name: string) [public, static, Player] {
        return make Player {
            name: name,
            health: 100,
            is_alive: true
        }
    }
}

// Instantiation:
var p: Player = Player.create("Hero")
p.take_damage(30)
printf("Health: {}\n", p.health) // 70
```

---

## 14. Enums

Type-safe enumerated values:
```alpha
enum ServerState {
    Starting,
    Running,
    Stopped,
    Crashed
}

var state: ServerState = ServerState.Running

if state == ServerState.Running {
    print("Server is healthy.")
}
```

---

## 15. Modules & Namespaces

Organize code cleanly into logical namespaces:
```alpha
module NetworkTools {
    function ping(host: string) [public, boolean] {
        return true
    }

    function format_url(domain: string, port: int) [public, string] {
        return format("http://{}:{}", domain, port)
    }
}

// Calling module functions:
var ok: boolean = NetworkTools.ping("localhost")
var url: string = NetworkTools.format_url("127.0.0.1", 8080)
```

---

## 16. Package Management (`packages.alpaks`)

All remote web dependencies are declared in `packages.alpaks` at the root of your project:
```text
# packages.alpaks
"something" = import web::"https://pastebin.com/raw/dC4eYYwH"
"uuid" = import web::"https://example.com/alpha-pkgs/uuid.alpha"
```

In your `.alpha` code, import them directly by their alias:
```alpha
import something
import uuid

function main() [public, nothing] {
    something()
}
```
*(Direct `import web::"..."` inside source files is strictly prohibited to keep builds reproducible and maintainable).*

---

## 17. Standard Core Library Reference

### `Core/Basic.alpha`
- `print(value)`: Prints value followed by a newline.
- `printf(format, ...)`: Prints formatted text (`{}` placeholders).
- `format(format, ...)`: Returns formatted string.
- `error(format, ...)`: Aborts execution with an error diagnostic.

### `Core/Cli.alpha`
- `cli.arguments()`: Returns `vect<string>` of CLI arguments passed to the application (1-based index).
- `paint(text, hexcolor)` / `cli.paint(text, hexcolor)`: Wraps text in ANSI 24-bit color codes.

### `Core/Io.alpha`
- `Io.read(path)`: Reads entire file as string.
- `Io.write(path, text)`: Overwrites file with text.
- `Io.append(path, text)`: Appends text to file.
- `Io.exists?(path)`: Checks if file exists.
- `Io.file?(path)`: Checks if path is a regular file.
- `Io.dir?(path)`: Checks if path is a directory.
- `Io.delete(path)`: Deletes file.
- `Io.open(path)`: Opens an `IoFile` handle with methods `.content()`, `.write()`, `.append()`, `.linelen()`, `.bytelen()`, `.symlen()`, `.empty?()`.

### `Core/Fs.alpha`
- `Fs.Dir(path)`: Opens a directory handle.
- `dir.exists?()`: Checks if directory exists on disk.
- `dir.create()`: Creates directory.
- `dir.delete()`: Deletes directory recursively.
- `dir.get_children()`: Returns `vect<string>` of directory entry names.
- `dir.file.create(name, content)`: Creates a file inside directory.
- `Fs.walk(path)` / `dir.walk()`: Returns full paths of all files recursively (can be used directly with `for each file in Fs.walk(dir)`).

### `Core/Env.alpha`
- `env.current`: String containing current working directory path.
- `env.exec_dir`: String containing directory of the running executable.

### `Core/Time.alpha`
- `time.now()`: Current UNIX timestamp in seconds (`int`).
- `time.now_ms()`: Current timestamp in milliseconds (`int`).
- `time.sleep(ms)`: Pauses thread execution for given milliseconds.
- `time.elapsed(start_ms)`: Calculates elapsed milliseconds since `start_ms`.
- `time.benchmark({ ... })`: Measures and returns execution duration of the block in milliseconds (`int`).

### `Core/Math.alpha`
- `math.abs(x)` / `abs(x)`: Absolute value.
- `math.min(a, b)` / `min(a, b)`: Minimum of two numbers.
- `math.max(a, b)` / `max(a, b)`: Maximum of two numbers.
- `math.clamp(val, min, max)` / `clamp(...)`: Clamps value within bounds.
- `math.pow(base, exp)` / `pow(...)`: Power exponentiation.
- `math.sqr(n)` / `sqr(n)`: Square of `n` (returns `<int, nothing>`).

### `Core/Random.alpha`
- `random()` / `random.float()`: Random float in `[0.0, 1.0)`.
- `random(max)` / `random.int(max)`: Random integer in `[0, max]`.
- `random(min, max)` / `random.int(min, max)`: Random integer in `[min, max]`.
- `random.choice(vect)`: Picks a random element from a vector.
- `random.string(len)`: Generates random alphanumeric string.
- `random.seed(val)`: Sets pseudo-random generator seed.

### `Core/Http.alpha`
- `http.get(url)`: Performs HTTP GET request and returns body as string.
- `http.post(url, body)`: Performs HTTP POST request.
- `http.download(url, filepath)`: Downloads remote URL directly to disk.

### `Core/Json.alpha`
- `json.parse(str)`: Parses JSON string into nested Alpha maps and vectors.
- `json.stringify(value)`: Serializes Alpha values into JSON string.

### `Core/Machine.alpha`
- `console.out(command)`: Executes system command in the terminal and returns its stdout string (`string`).

---

## 18. CLI Tooling Reference

```bash
# Initialize a new project
alpha here <project_name>

# Run a project or file
alpha run
alpha run src/main.alpha --debug 8080

# Check types and syntax without executing
alpha check
alpha check src/main.alpha

# Compile to a native binary in build/
alpha build

# Version and help
alpha -v
alpha --version
alpha -h
alpha --help
```

---
---

# 🇷🇺 Русская Документация (Russian Documentation)

## 1. Основы синтаксиса
- **Строгая статическая типизация**: Все переменные и функции имеют типы, проверяемые на этапе компиляции.
- **Необязательные точки с запятой**: Точки с запятой (`;`) ставить не требуется.
- **Блоки кода**: Выделяются фигурными скобками `{ ... }`.
- **Комментарии**: Однострочные комментарии начинаются с `//`.

---

## 2. Система типов

### 2.1 Примитивные типы
- `int`: 64-битное целое со знаком (`42`, `-10`).
- `unt`: 64-битное целое без знака (`0` до `18_446_744_073_709_551_615`). Поддерживает беззнаковую арифметику, деление, остаток `%` и сравнения. Присвоение отрицательных чисел пресекается компилятором и рантаймом.
- `float`: 64-битное число с плавающей точкой (`3.14`, `-0.5`).
- `string`: Строка в кодировке UTF-8 (`"Привет, Alpha!"`).
- `char`: Символ Unicode (`'A'`, `'я'`).
- `boolean`: Логическое `true` или `false`.

### 2.2 Специальные типы
- `something`: Универсальный динамический тип (может содержать значение любого типа).
- `nothing`: Отсутствие значения (`nil` / `null` / `void`).
- `hexcolor`: Отдельный тип цвета, задаваемый как `#(...)`, например `#(FFF00)` или `#(00AAFF)`.

### 2.3 Составные типы
- **Вектор (`vect<T>`)**: Динамический расширяемый массив элементов типа `T`.
- **Хэш-таблица (`map<K, V>`)**: Ассоциативная таблица пар ключ-значение.
- **Кортеж (`(T1, T2, ...)` )**: Неизменяемая упорядоченная последовательность значений разных типов.

### 2.4 Union / Мульти-вариантные типы
Функции могут возвращать несколько альтернативных типов:
```alpha
function get_data(id: int) [public, <string, nothing>] {
    if id == 1 {
        return "Успешно"
    }
    return nothing
}
```

### 2.5 Режим строгой типизации (`@strict`)
Добавление директивы `@strict` в файл активирует бескомпромиссную строгую проверку типов для всех последующих строк:
1. **Обязательная явная типизация**: Каждая переменная обязана иметь явно указанный тип (`var x: int = 10`). Объявления вида `var x = 10` запрещены.
2. **Безопасная распаковка Box / Union типов**: Значения составных union/box типов обязаны обрабатываться через безопасное сопоставление `check`. Использование операторов форсированного развертывания (`!?` и `!`) строго запрещено!
3. **Полный запрет `something` и `nothing`**: Запрещено объявлять типы `something` / `nothing` или использовать литерал `nothing`.
4. **Строгий контроль**: При малейшем нарушении компилятор останавливает сборку с ошибкой `[Strict Mode]`.

```alpha
@strict

function calculate(a: int, b: int) [public, <int, string>] {
    if b == 0 {
        return "Деление на ноль"
    }
    return a / b
}

function main() [public, int] {
    var res: <int, string> = calculate(10, 2)
    
    // В режиме @strict обязательна безопасная обработка:
    var final_val: int = check res {
        (5) => 5,
        else => 0
    }
    return final_val
}
```

---

## 3. Индексация с единицы (1-based Indexing)

В Alpha счёт элементов во всех индексируемых коллекциях (**vect**, **tuple**, **string**, а также позиционный доступ в **map**) начинается строго с **единицы (`1`)**.

```alpha
var v: vect<string> = vect.new()
v.push("первый")
v.push("второй")
v.push("третий")

var first: string = v[1]       // "первый"
var second: string = v[2]      // "второй"
v[2] = "новый_второй"          // Изменение элемента по 1-based индексу

var t: (int, string) = (42, "ответ")
var n: int = (t[1] -> int)     // 42
var s: string = (t[2] -> string) // "ответ"
```

### Срезы (Slicing)
Срезы поддерживают нестрогий диапазон `..` (до указанного) и строгий `...` (включительно):
```alpha
var text: string = "hello world"
var s1: string = text[1..6]     // "hello" (символы с 1 по 5 включительно)
var s2: string = text[7...11]   // "world" (символы с 7 по 11 включительно)
var s3: string = text[7..]      // "world" (от 7 символа до конца)
var s4: string = text[..6]      // "hello" (от начала до 6 не включительно)
```

---

## 4. Переменные и константы

- `var`: Изменяемая переменная.
- `const var`: Неизменяемая константа.

```alpha
var counter: int = 1
counter = 2 // Корректно

const var PI: float = 3.14159265
// PI = 3.0 // Ошибка компиляции: нельзя изменять константу
```

---

## 5. Операторы и выражения

- **Арифметика**: `+`, `-`, `*`, `/`, `%`
- **Составное присваивание**: `+=`, `-=`, `*=`, `/=`, `%=`
- **Сравнения**: `==`, `!=`, `<`, `<=`, `>`, `>=`
- **Логические**: `&&` (И), `||` (ИЛИ), `!` (НЕ)
- **Тернарный оператор**: `условие ? значение_если_истина : значение_если_ложь`
- **Оператор Элвиса (дефолтное значение)**: `значение ?: дефолт`
- **Принудительное разворачивание**: `значение!` (ошибка в рантайме, если там `nothing`)
- **Явное приведение типов**: `(выражение -> целевой_тип)`
- **Интерполяция строк**: `"Пользователь: \{name}, Баланс: \{balance} руб."`
- **Многострочные строки**: `""" ... """`

---

## 6. Функции

### Синтаксис и метаданные
Свойства функции (видимость и тип возвращаемого значения) указываются в квадратных скобках `[видимость, тип]`:
```alpha
function add(a: int, b: int) [public, int] {
    return a + b
}
```

### Видимость
- `public`: Функция доступна из других файлов и модулей.
- `here`: Приватная функция (доступна только внутри текущего файла).

### Знак вопроса в именах
В Alpha знак `?` разрешён в любой позиции имени функций и переменных:
```alpha
function exists?() [public, boolean] {
    return true
}

function user?has?access() [public, boolean] {
    return false
}
```

---

## 7. Управляющие конструкции

### Условия If / Else
```alpha
var age: int = 20
if age >= 18 {
    print("Доступ разрешён")
} else {
    print("Доступ ограничен")
}
```

### Циклы
```alpha
// 1. Диапазонный цикл (1-based)
for i = 1, i <= 10, i++ {
    printf("Шаг: {}\n", i)
}

// 2. Цикл повторений без переменной-счётчика (N раз)
for 1...3 {
    print("Уведомление!")
}

// 3. Итерация по элементам вектора (for each)
var items: vect<string> = vect.new()
items.push("яблоко")
items.push("банан")

for each fruit in items {
    printf("Фрукт: {}\n", fruit)
}

// 4. Итерация по парам хэш-таблицы
var prices: map<string, int> = map.new()
prices.insert("яблоко", 50)
prices.insert("банан", 80)

for each key, value in prices {
    printf("{}: {} руб.\n", key, value)
}

// 5. Бесконечный цикл с break и continue
var x: int = 1
for {
    if x > 100 { break }
    x += 10
}
```

---

## 8. Сопоставление с образцом (`check`)

Выражение `check` сопоставляет значение с шаблонами:
```alpha
var status: int = 200

var text: string = check status {
    200 => "Успешно",
    404 => "Не найдено",
    500 => "Ошибка сервера",
    _   => "Неизвестный статус"
}
```

Поддерживаются блоки кода:
```alpha
function handle_command(command: string) [public, boolean] {
    check command {
        "start" => {
            return true
        },
        "stop" => {
            return false
        },
        _ => {
            return false
        }
    }
}
```

---

## 9. Обработка ошибок (`try ... catch`)

Перехватывайте ошибки времени выполнения без падения процесса:
```alpha
try {
    var data: string = Io.read("config.json")
    print(data)
} catch err {
    printf("Не удалось прочитать файл: {}\n", err)
}

// Генерация пользовательской ошибки
function check_not_empty(str: string) [public, nothing] {
    if str.empty?() {
        error("Строка не может быть пустой!")
    }
}
```

---

## 10. Отложенное выполнение (`make schedule` / Defer)

Гарантирует выполнение блока очистки ресурсов при выходе из области видимости. Выполняется по принципу **LIFO** (последним запланирован — первым выполнен):

```alpha
function send_message(dest: string) [public, nothing] {
    make schedule {
        print("1. Разрыв соединения с " + dest)
    }
    make schedule (print("2. Сброс буфера передачи"))

    // При выходе из функции сначала напечатается #2, а затем #1!
}
```

---

## 11. Утверждения (`verify`)

Проверка инвариантов и предусловий:
```alpha
var divider: int = 5
var name: string = "Alpha"

verify divider != 0, "Делитель не может быть равен нулю!"
verify name.len() > 0, "Имя не может быть пустым"
```

---

## 12. Условная компиляция под ОС

Блоки кода, выполняемые только на указанной платформе:
```alpha
!system win {
    print("Платформа: Windows")
}

!system posix {
    print("Платформа: Linux / macOS")
}
```

---

## 13. Структуры и ООП

```alpha
structure User {
    username: string,
    balance: int,

    function add_funds(amount: int) [public, nothing] {
        this.balance += amount
    }

    function create(name: string) [public, static, User] {
        return make User {
            username: name,
            balance: 0
        }
    }
}

// Создание и вызов методов:
var u: User = User.create("Max")
u.add_funds(500)
printf("Баланс: {} руб.\n", u.balance) // 500
```

---

## 14. Перечисления (`enum`)

```alpha
enum Status {
    Pending,
    Active,
    Done
}

var s: Status = Status.Active

if s == Status.Active {
    print("Задача выполняется.")
}
```

---

## 15. Пространства имён (`module`)

Группировка функций и утилит в модули:
```alpha
module StringUtils {
    function capitalize(s: string) [public, string] {
        return s // логика
    }

    function is_blank?(s: string) [public, boolean] {
        return s.empty?()
    }
}

// Вызов:
var res: boolean = StringUtils.is_blank?("hello")
```

---

## 16. Управление веб-пакетами (`packages.alpaks`)

Все внешние веб-библиотеки объявляются исключительно в корневом файле **`packages.alpaks`**:
```text
# packages.alpaks
"something" = import web::"https://pastebin.com/raw/dC4eYYwH"
"http_utils" = import web::"https://example.com/alpha-pkgs/http.alpha"
```

В исходном коде они импортируются по заданному алиасу:
```alpha
import something
import http_utils

function main() [public, nothing] {
    something()
}
```
*(Прямой импорт `import web::"..."` внутри исходных файлов запрещён на уровне компилятора).*

---

## 17. Справочник стандартной библиотеки (`Core/`)

### `Core/Basic.alpha`
- `print(value)`: Печатает значение с переносом строки.
- `printf(format, ...)`: Форматированный вывод в консоль.
- `format(format, ...)`: Возвращает отформатированную строку.
- `error(format, ...)`: Прерывает выполнение с диагностической ошибкой.

### `Core/Cli.alpha`
- `cli.arguments()`: Возвращает `vect<string>` переданных аргументов командной строки (1-based индексация).
- `paint(text, hexcolor)` / `cli.paint(text, hexcolor)`: Окрашивает текст в 24-битный ANSI-цвет.

### `Core/Io.alpha`
- `Io.read(path)`: Читает файл целиком как строку.
- `Io.write(path, text)`: Перезаписывает файл строкой.
- `Io.append(path, text)`: Дописывает строку в файл.
- `Io.exists?(path)`: Проверяет существование файла.
- `Io.file?(path)`: Проверяет, является ли путь файлом.
- `Io.dir?(path)`: Проверяет, является ли путь каталогом.
- `Io.delete(path)`: Удаляет файл.
- `Io.open(path)`: Открывает дескриптор `IoFile` (`.content()`, `.write()`, `.append()`, `.linelen()`, `.bytelen()`, `.symlen()`, `.empty?()`).

### `Core/Fs.alpha`
- `Fs.Dir(path)`: Открывает дескриптор каталога.
- `dir.exists?()`: Проверяет существование каталога.
- `dir.create()`: Создает каталог.
- `dir.delete()`: Рекурсивно удаляет каталог.
- `dir.get_children()`: Возвращает `vect<string>` содержимого.
- `dir.file.create(name, text)`: Создает файл внутри папки.
- `Fs.walk(path)` / `dir.walk()`: Рекурсивно возвращает пути всех вложенных файлов (удобно для `for each file in Fs.walk("src")`).

### `Core/Env.alpha`
- `env.current`: Текущая рабочая директория (строка).
- `env.exec_dir`: Директория исполняемого файла (строка).

### `Core/Time.alpha`
- `time.now()`: UNIX-время в секундах (`int`).
- `time.now_ms()`: UNIX-время в миллисекундах (`int`).
- `time.sleep(ms)`: Задержка выполнения потока в миллисекундах.
- `time.elapsed(start_ms)`: Количество миллисекунд, прошедших с `start_ms`.
- `time.benchmark({ ... })`: Замеряет и возвращает время выполнения блока кода в миллисекундах (`int`).

### `Core/Math.alpha`
- `math.abs(x)` / `abs(x)`: Модуль числа.
- `math.min(a, b)` / `min(a, b)`: Минимум из двух чисел.
- `math.max(a, b)` / `max(a, b)`: Максимум из двух чисел.
- `math.clamp(val, min, max)` / `clamp(...)`: Ограничение диапазона.
- `math.pow(base, exp)` / `pow(...)`: Возведение в степень.
- `math.sqr(n)` / `sqr(n)`: Квадрат числа (возвращает `<int, nothing>`).

### `Core/Random.alpha`
- `random()` / `random.float()`: Случайный float в диапазоне `[0.0, 1.0)`.
- `random(max)` / `random.int(max)`: Случайное целое в диапазоне `[0, max]`.
- `random(min, max)` / `random.int(min, max)`: Случайное целое в диапазоне `[min, max]`.
- `random.choice(vect)`: Случайный выбор элемента из вектора.
- `random.string(len)`: Генерация случайной буквенно-цифровой строки.
- `random.seed(val)`: Установка сида генератора.

### `Core/Http.alpha`
- `http.get(url)`: GET-запрос (возвращает тело ответа).
- `http.post(url, body)`: POST-запрос с телом.
- `http.download(url, filepath)`: Скачивание файла по сети на диск.

### `Core/Json.alpha`
- `json.parse(str)`: Парсинг JSON строки в структуры Alpha (`map`, `vect`).
- `json.stringify(val)`: Сериализация значения в строку JSON.

### `Core/Machine.alpha`
- `console.out(command)`: Выполняет терминальную команду операционной системы и возвращает её вывод (`string`).

---

## 18. Команды утилиты CLI (`alpha`)

```bash
# Инициализация нового проекта
alpha here <имя_проекта>

# Запуск проекта или отдельного скрипта
alpha run
alpha run src/main.alpha --port 8080

# Проверка синтаксиса и типов без исполнения
alpha check
alpha check src/main.alpha

# Сборка в нативный исполняемый файл (.exe) в папке build/
alpha build

# Версия языка и справка
alpha -v
alpha --version
alpha -h
alpha --help
```

---

## 15. Language Server (`alpha-lsp`) & Editor Integration

The Alpha language includes an official Language Server Protocol implementation (`alpha-lsp`) providing IDE features for editors like **Zed**, VS Code, and Neovim.

### Features
- **Real-time Diagnostics**: Immediate feedback on syntax errors, type-check errors, and `@strict` violations as you type.
- **Smart Completions**: Auto-completes keywords, primitive types (`unt`, `int`), and standard library modules (`Io`, `Fs`, `Time`, `Math`, `Env`).
- **Hover Documentation**: Detailed documentation for keywords, types, and functions.

### Running the Language Server
```bash
# Direct binary
alpha-lsp

# Or via alpha CLI subcommand
alpha lsp
```

### Zed Editor Extension (`editors/zed-alpha`)
The `editors/zed-alpha` folder contains a complete Zed extension:
1. In Zed, open Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`).
2. Run `zed: install dev extension`.
3. Select `editors/zed-alpha`.
4. Zed automatically links Tree-sitter syntax highlighting, symbol outline, and connects to `alpha-lsp`.

---

## 15. Language Server (`alpha-lsp`) и интеграция с редакторами

В состав языка Alpha входит официальный сервер языка по протоколу LSP (`alpha-lsp`), обеспечивающий поддержку для редакторов **Zed**, VS Code, Neovim и других.

### Возможности
- **Диагностика в реальном времени**: Мгновенная подсветка синтаксических ошибок, несоответствия типов и нарушений директивы `@strict` при вводе.
- **Автодополнение (Completions)**: Подсказки по ключевым словам, примитивным типам (`unt`, `int`, etc.), функциям и модулям стандартной библиотеки (`Io`, `Fs`, `Time`, `Math`, `Env`).
- **Всплывающая документация (Hover)**: Подробные описания и примеры при наведении курсора на ключевые слова, директивы и типы.

### Запуск Language Server
```bash
# Отдельный исполняемый файл
alpha-lsp

# Либо через CLI команду
alpha lsp
```

### Расширение для редактора Zed (`editors/zed-alpha`)
В папке `editors/zed-alpha` находится готовое расширение для Zed:
1. В Zed откройте командную палитру (`Ctrl+Shift+P` / `Cmd+Shift+P`).
2. Выберите команду `zed: install dev extension`.
3. Укажите папку `editors/zed-alpha`.
4. Zed активирует подсветку Tree-sitter, дерево символов и подключит `alpha-lsp` для валидации и подсказок.
