## 1. Основные правила

- Типизированный язык программирования.
- Точки с запятой в конце инструкций необязательны.
- Блоки кода обозначаются "{}".
- Однострочные комментарии начинаются с "//".

## 2. Типы данных

Поддерживаемые типы:

- "int" — целое число.
- "float" — число с плавающей точкой.
- "string" — строка.
- "char" — один символ.
- "boolean" — логическое значение.
- "something" — универсальный или неопределённый тип.
- "nothing" — отсутствие значения.
- "vect<T>" — динамический массив элементов типа "T".
- "map<K, V>" — хэш-таблица с ключами типа "K" и значениями типа "V".

Примеры:

var age: int = 18
var name: string = "Alpha"
var numbers: vect<int> = vect.new()
var scores: map<string, int> = map.new()

Типы "vect<T>" и "map<K, V>" должны учитывать типы своих элементов, ключей и значений.

Вектор "vect<T>"

var numbers: vect<int> = vect.new()

numbers.push(10)
numbers.push(20)

- "vect.new()" — создание вектора.
- "push(value)" — добавление элемента в конец.
- Все элементы должны соответствовать типу "T".

Хэш-таблица "map<K, V>"

var scores: map<string, int> = map.new()

scores.insert("Player1", 100)
scores.insert("Player2", 250)

- "map.new()" — создание хэш-таблицы.
- "insert(key, value)" — добавление пары ключ-значение или обновление значения существующего ключа.
- "contains(key)" — проверка существования ключа; возвращает "boolean".
- "get(key)" — получение значения по ключу; если ключ отсутствует, возвращает "nothing".
- Ключи должны соответствовать типу "K", значения — типу "V".

3. Переменные и константы

Переменная:

var name: string = "Alpha"

Константа:

const var max_speed: int = 100

"var" объявляет изменяемую переменную. "const var" объявляет значение, которое нельзя изменять после инициализации.

## 4. Функции

Общий синтаксис:

function name(arguments) [visibility, return_type] {
// Function body
}

Аргументы объявляются с указанием типов:

function add(a: int, b: int) [public, int] {
return a + b
}

Если аргумент должен быть чем угодно в случае с print то это тип something

Видимость

- "here" — функция доступна только внутри текущего файла.
- "public" — функция доступна из других файлов и модулей в соответствии с правилами импорта.

Примеры:

```
function calculate() [here, int] {
  return 10
}
```
```
function greet() [public, nothing] {
  print("Hello")
}
```

### Возвращаемые типы

В метаданных указывается ожидаемый возвращаемый тип:

```
function get_number() [here, int] {
  return 42
}
```

"nothing" означает, что функция не возвращает значение.

"something" используется, когда результат определённо будет но любого типа кроме nothing

Запись "<string, int>" означает, что функция может вернуть либо "string", либо "int".

```
function get_value() [here, <string, int>] {
  return "Hello"
}
```

Функция должна соблюдать объявленный возвращаемый тип. Если указано несколько типов, результат должен соответствовать одному из них.

Символ `?` разрешён в любом месте имени функции и идентификаторов (в начале, середине, конце и в любом количестве):
```
function is_valid?() [public, boolean] {
  return true
}

function is?prime(n: int) [public, boolean] {
  return n > 1
}

function ?starts_with_question?(x: int) [public, int] {
  return x * 2
}

function check???(msg: string) [public, string] {
  return msg + "!"
}
```
## 5. Импорты

Локальный файл

`import (file_name).alpha`

Также поддерживаются пути с папками:
`import tools/wifi.alpha`

Встроенный модуль

`import Core/Basic.alpha`

Импорт через веб

`import web::"file, repo url"`

Веб-импорт может указывать на удалённый файл или репозиторий.

## 6. Условия

Используются "if", "elseif" и "else".

```
if condition {
// First branch
} elseif another_condition {
// Alternative branch
} else {
// Default branch
}
```
также присутствует отрицание:
```
if not condition {}
```
## 7. Циклы

Бесконечный цикл
```
for {
// Repeated indefinitely
}
```
Цикл со счётчиком
```
for i = 0, i < 5, i++ {
  print(i)
}
```

Формат: начальное значение, условие продолжения, шаг.

Перебор коллекции

```
for each entry in variable {
  print(entry)
}
```
для map вот так:
```
for each key, value in variable {
  print(entry)
}
```

## 8. Проверка "nothing"

Для проверки отсутствия значения используется ".nothing?()":
```
if variable.nothing?() {
  print("Value is nothing")
}
```

9. Пример программы
```
import Core/Basic.alpha

var players: vect<string> = vect.new()
var scores: map<string, int> = map.new()

function add_player(name: string) [public, nothing] {
  players.push(name)
  scores.insert(name, 0)
}

function get_score(name: string) [public, <int, nothing>] {
  if scores.contains(name) {
    return scores.get(name)
  } else {
    return nothing
  }
}

function main() [public, nothing] {
  add_player("Player1")
  add_player("Player2")

  for each player in players {
    print(player)
  }
}
```

## 10. Core

Если что Core это папка в src языка а basic.alpha это просто файл который находится в инструментарии то есть по факту Core это папка с core инструментами написаными на самом языке Alpha которая доступная всегда и везде

## 11. Модуль Basic.alpha

Этот модуль хранит в себе такие функции:
`print()` // принт любого типа кроме nothing
`printf()` // принт с форматированием, пример: `printf("text {} other {}", var1, var1 + 2)`
`format()` // форматирует что угодно в указанный формат, возвращает string
`error("error: {}", err)` // выкидывает ошибку и останавливает программу

пример format:
```
format("{2}:{2}:{2}", hours, minutes, seconds)
```
Результат: 02:30:10

{2} означает что должно быть 2 гарантированно символа, типо не 2 а 02 а если числа в скобках нет то будет просто число из аргумента просто 2 например

## 11. Проекты

В языке alpha недостаточно просто создать файл main.alpha, надо создать проект используя команду (исполняемый файл языка, alpha) here тогда в текущей директории инициализируется проект на языке alpha:

В директории появляются обьекты:

файл alpha-project.toml с содержанием:
```
[project]
name = "project_name"
version = "0.0.1"
```
И папка src/ с файлом main.alpha в котором написано:
```
function main() [here, nothing] {

}
```
Объяснение: файл alpha-project.toml обозначает для компилятора что это проект на языке alpha и у него есть 1 единый главный файл src/main.alpha

Alpha это компилируемый язык программирования и для того чтобы его собрать в исполняемый файл нужно написать (исполняемый файл языка, alpha) build и тогда в папке build появится исполняемый файл

## 12. Работа с файлами (Модуль Core/Io.alpha):

Для работы с файлами надо импортировать модуль `import Core/Io.alpha`. Все файловые операции (чтение, запись, копирование, перемещение, удаление файлов) сосредоточены в модуле `Io`.

### Статические функции `Io`:
```alpha
import Core/Io.alpha

function main() [public, nothing] {
    // 1. Быстрая запись и перезапись
    Io.write("data.txt", "Alpha Language File I/O")

    // 2. Быстрое чтение всего файла в строку
    var text: string = Io.read("data.txt")

    // 3. Проверка существования и типа
    var exists: boolean = Io.exists?("data.txt")
    var is_file: boolean = Io.file?("data.txt")

    // 4. Размер файла в байтах
    var sz: int = Io.size("data.txt")

    // 5. Копирование и перемещение файлов
    Io.copy("data.txt", "backup.txt")
    Io.move("backup.txt", "archive.txt")

    // 6. Удаление файла
    Io.delete("archive.txt")
}
```

### Дескриптор файла `Io.File`:
Для того чтобы открыть файл и работать через дескриптор:
```alpha
var file: Io.File = Io.open("path")
var content: string = file.content()
```

Чтобы полностью очистить файл:
```alpha
var file: Io.File = Io.open("path")
file.reset() // очистить содержимое файла
file.write("some\nthing")
```

Методы `Io.File`:
- `file.content()` // чтение всего содержимого файла как string
- `file.write("text")` // дописать текст в файл
- `file.reset()` // очистить файл
- `file.delete()` // удалить файл
- `file.exists?()` // существует ли такой файл? (также `file.something?()`)
- `file.empty?()` // пустой ли файл?
- `file.size()` // размер файла в байтах (также `file.bytelen()`)
- `file.linelen()` // сколько строк в файле
- `file.symlen()` // сколько символов в файле

## 13. Работа с директориями и путями (Модуль Core/Fs.alpha):

Для работы с директориями и путями импортируется модуль `import Core/Fs.alpha`. Все файловые операции перенесены в `Io`, а `Fs` отвечает за пути и папки в стиле Alpha с вопросительными знаками `?`.

### Статические функции `Fs`:
- `Fs.exists?(path)` — существует ли путь (папка или файл)?
- `Fs.file?(path)` — является ли путь файлом?
- `Fs.dir?(path)` — является ли путь директорией?
- `Fs.join("a", "b", "c")` — объединение путей
- `Fs.extension(path)` — расширение файла (например `"txt"`)
- `Fs.filename(path)` — имя файла с расширением (например `"hello.txt"`)
- `Fs.stem(path)` — имя файла без расширения (например `"hello"`)
- `Fs.parent(path)` — родительская директория
- `Fs.abs(path)` — абсолютный путь на диске

### Работа с директориями (`Fs.Dir` и `dir`):
```alpha
var my_dir: Fs.Dir = Fs.open("dir_path") // или dir.open("dir_path")
```

Методы директории:
- `dir.exists?(name)` — существует ли дочерний элемент с таким именем?
- `dir.files()` — возвращает `vect<string>` со списком файлов в директории
- `dir.dirs()` — возвращает `vect<string>` со списком поддиректорий
- `dir.walk()` — рекурсивный обход всех вложенных файлов и папок (возвращает `vect<string>`)
- `dir.create("subfolder")` — создать новую подпапку
- `dir.delete(".")` — удалить директорию или указанную папку
- `dir.rename("new_name", ".")` — переименовать
- `dir.reset()` — удалить все вложенные элементы в папке
- `dir.empty?()` — пуста ли папка?
- `dir.nothing?()` — не существует ли папка?

#### Итерация по директории с `dir.walk()`:
Функция `dir.walk()` возвращает вектор строк всех рекурсивно найденных элементов, что идеально для цикла `for ... in`:
```alpha
var project_dir: Fs.Dir = Fs.open(".")
for file in project_dir.walk() {
    print(file)
}
```

## 14. Новые возможности языка (Alpha 0.2+)

### Структуры и методы (Классы)
В языке Alpha классы реализуются как структуры (`structure`), к которым прикрепляются методы в блоке `attach`:
```alpha
structure Player {
    name: string,
    health: int,
    inventory: vect<string>,
attach {
    function new(name: string) [static, public, Player] {
        return make this{name: name, health: 100, inventory: vect.new()}
    }

    function take_damage(dmg: int) [nonstatic, public, nothing] {
        this.health = this.health - dmg
    }

    function is_alive?() [nonstatic, public, boolean] {
        return this.health > 0
    }
}
}
```
- Метаданные методов: `[(static/nonstatic), (public/here), (return_type)]`.
- `static` — статический метод, вызывается до создания объекта через имя структуры, например: `Player.new("Alice")`.
- `nonstatic` — метод экземпляра, вызывается на объекте, например: `player.take_damage(20)`.
- Для обращения к полям и методам объекта внутри `nonstatic` методов используется ключевое слово `this`.
- Конструирование объекта: `make this{field: value, ...}` или `make StructureName{field: value, ...}`.

### Pattern Matching (`check`)
Сопоставление с образцом поддерживает как инструкции (statement), так и выражения (expression) с распаковкой:

**Как инструкция:**
```alpha
check something {
    (something) => { print("OK") },
    else => { error("err") }
}
```

**Как выражение с распаковкой значения:**
```alpha
var str: string = check function_with_return_value() {
    something => received, // если вернулось значение, оно распаковывается в переменную received
    nothing => { error("err") } // если nothing, выполняется ветка
}
```

### Быстрая распаковка (`!?`)
Постфиксный оператор `!?` принудительно распаковывает значение из объединения `<T, nothing>`. Если значение равно `nothing`, выполнение завершается ошибкой:
```alpha
var box: <int, nothing> = scores.get("Player1")
var value: something = box!?
```

### Элвис-оператор (`or`)
Позволяет задать запасное значение по умолчанию, если левый операнд равен `nothing`:
```alpha
var score: int = scores.get("Player1") or 10
```

### Диапазонный цикл `for`
Поддерживаются включительный (`...`) и исключительный (`..`) диапазоны:
```alpha
for n in 1...3 {
    // n принимает значения 1, 2, 3
}

for n in 1..3 {
    // n принимает значения 1, 2
}
```

### Логические операторы
Поддерживаются символьные и словесные операторы:
- `&&` и `and` — логическое И
- `||` и `or` — логическое ИЛИ
- `!` и `not` — логическое НЕ

Пример:
```alpha
if a == b && b == a || !(c == a) {
    print("Condition met")
}
```

### Приведение типов (`->`)
Синтаксис: `((data) -> (type))` или `(expr -> type)`:
```alpha
var result: string = func()
var a: int = (result -> int)
var s: string = (a -> string)
```

### Тернарный оператор (строчный if)
Синтаксис: `if cond: then_expr else else_expr`:
```alpha
var a: int = if b > 2: 2 else 3
var title: string = if b > 2: "Yes" else "No"
```

### Конкатенация строк через `+=`
Строки можно объединять оператором `+=`:
```alpha
var msg: string = "Hello"
msg += " World!"
```

### Группировка скобками в арифметике
Поддерживается произвольная вложенность скобок:
```alpha
var x: int = (2 + 3) * 4 // 20
```

### Методы строк
- `.len()` — возвращает длину строки в символах (`int`).
- `.empty?()` — возвращает `true`, если строка пустая (`boolean`).

Пример:
```alpha
var s: string = "Alpha"
var length: int = s.len()
var empty: boolean = s.empty?()
```

### Интерполяция строк и многострочные строки
Поддерживается интерполяция `$""` с выражениями внутри `{}`:
```alpha
var name: string = "Alice"
var greeting: string = $"Hello {name}, welcome to Alpha!"
```

Многострочные строки `"""..."""` и интерполированные многострочные строки `$"""..."""`:
```alpha
var text: string = """
Line 1
Line 2
"""

var dynamic_text: string = $"""
User: {name}
Status: OK
"""
```

### Модуль `Core/Machine.alpha` и терминал `console.out()`
Модуль для низкоуровневого взаимодействия с машиной и вызова терминальных команд:
```alpha
import Core/Machine.alpha

function main() [public, nothing] {
    // Выполняет терминальную команду кроссплатформенно и возвращает её вывод как string
    var output: string = console.out("echo Hello from Machine!")
    print(output)

    var current_dir: string = console.out("whoami")
    print(current_dir)
}
```
- На **Windows** команда выполняется через `cmd.exe /D /C ...` с автоматической поддержкой кодировок UTF-8 и OEM/CP866.
- На **Linux / macOS** команда выполняется через `sh -c ...`.
- Функция возвращает `string` с результатом вывода команды (stdout/stderr).

### Условные блоки платформы `!system`
Позволяет выполнять код или объявлять функции/структуры/импорты только для определенной операционной системы (`win` / `posix` / `linux` / `macos`):

```alpha
!system win {
    function get_os() [public, string] {
        return "windows"
    }
}

!system posix {
    function get_os() [public, string] {
        return "posix"
    }
}
```

Также блоки `!system` можно использовать внутри функций:
```alpha
function get_slash() [public, string] {
    var slash: string = ""
    !system win {
        slash = "\"
    }
    !system posix {
        slash = "/"
    }
    return slash
}
```
Поддерживаемые платформы:
- `win`, `windows` — Windows
- `posix`, `unix` — любые POSIX-совместимые ОС (Linux, macOS, BSD)
- `linux` — Linux
- `macos`, `darwin`, `osx` — macOS

### Модуль `Core/Random.alpha` и генератор `random`
В Alpha встроен быстрый и качественный генератор псевдослучайных чисел (на базе SplitMix64). Доступен глобально под именем `random`, а также через модуль `import Core/Random.alpha`:

```alpha
import Core/Random.alpha

function main() [public, nothing] {
    // 1. Случайные целые числа
    var dice: int = random.int(1, 6)          // от 1 до 6 включительно
    var up_to_ten: int = random.int(10)       // от 0 до 10 включительно

    // 2. Случайные числа с плавающей точкой
    var chance: float = random.float()        // от 0.0 до 1.0
    var temp: float = random.float(15.5, 30.0)// от 15.5 до 30.0

    // 3. Случайный булевый флаг
    var coin: boolean = random.bool()         // true или false (также random.boolean())

    // 4. Случайный выбор элемента из вектора
    var options: vect<string> = vect.new()
    options.push("Лево")
    options.push("Право")
    options.push("Прямо")
    var turn: string = (random.choice(options) -> string)

    // 5. Случайная строка / токен
    var token: string = random.string(16)     // 16 случайных символов A-Z, a-z, 0-9

    // 6. Установка seed для воспроизводимости
    random.seed(42)

    // 7. Также можно вызывать как функцию:
    var r1: int = random(1, 100)
    var r2: float = random()
}
```


### Кортежи (Tuples), множественный возврат и распаковка
Кортежи позволяют группировать несколько значений разных типов в одно целое:

```alpha
// Функция, возвращающая кортеж из двух значений
function get_player() [public, (int, string)] {
    return (101, "AlphaDev")
}

function main() [public, nothing] {
    // 1. Создание кортежа и индексация:
    var data: (int, string, boolean) = (1, "ok", true)
    var id: int = (data[0] -> int)
    var status: string = (data[1] -> string)

    // 2. Распаковка результата функции без указания типов:
    var (uid, name) = get_player()

    // 3. Распаковка с явным указанием типов элементов:
    var (x: int, y: string) = (50, "coords")

    // 4. Множественное присваивание через кортеж:
    (uid, name) = (202, "Hero")
}
```

### Срезы строк и векторов (Slicing)
Поддерживаются срезы с полуинтервалами (`..` исключительный) и отрезками (`...` включительный), а также открытые срезы:

```alpha
function main() [public, nothing] {
    // Срезы строк:
    var s: string = "hello world"
    var sub1: string = s[0..5]      // "hello" (символы с 0 по 4)
    var sub2: string = s[6...10]    // "world" (символы с 6 по 10 включительно)
    var sub3: string = s[6..]       // "world" (от 6 до конца строки)
    var sub4: string = s[..5]       // "hello" (от начала до 5)

    // Срезы векторов:
    var v: vect<int> = vect.new()
    v.push(10)
    v.push(20)
    v.push(30)
    v.push(40)

    var v1: vect<int> = v[1..3]     // [20, 30]
    var v2: vect<int> = v[2...]     // [30, 40]
    var v3: vect<int> = v[..2]      // [10, 20]
}
```

### Модуль `Core/Time.alpha` и работа со временем `time`
Доступен как глобально через `time`, так и через явный импорт `import Core/Time.alpha`:

```alpha
import Core/Time.alpha

function main() [public, nothing] {
    // 1. Текущее время Unix Epoch в секундах
    var sec: int = time.now()

    // 2. Текущее время Unix Epoch в миллисекундах
    var start: int = time.now_ms()

    // 3. Задержка / сон потока на указанное число миллисекунд
    time.sleep(100)

    // 4. Прошедшее время в миллисекундах от заданной отметки
    var diff: int = time.elapsed(start)
    print($"Time elapsed: {diff} ms")
}
```

### Модуль `Core/Http.alpha` и сетевые запросы `http`
Кроссплатформенный HTTP-клиент (на базе безопасных вызовов curl с поддержкой SSL/TLS и редиректов). Доступен глобально `http` или через `import Core/Http.alpha`:

```alpha
import Core/Http.alpha

function main() [public, nothing] {
    // 1. HTTP GET запрос
    var response: string = http.get("https://api.github.com/zen")
    print(response)

    // 2. HTTP POST запрос с телом (application/json)
    var post_res: string = http.post("https://httpbin.org/post", "{\"message\": \"Hello from Alpha\"}")

    // 3. Скачивание файла по URL
    var success: boolean = http.download("https://example.com/index.html", "downloaded.html")
}
```

### Модуль `Core/Json.alpha` и сериализация `json`
Модуль для парсинга и сериализации JSON. Преобразует JSON в нативные структуры данных Alpha (`map`, `vect`, `string`, `int`, `float`, `boolean`, `nothing`). Доступен глобально `json` или через `import Core/Json.alpha`:

```alpha
import Core/Json.alpha

function main() [public, nothing] {
    var raw: string = "{\"user\": \"Alex\", \"score\": 250, \"tags\": [\"alpha\", \"fast\"]}"

    // 1. Парсинг JSON строки в объект Alpha:
    var doc: something = json.parse(raw)
    var user: string = (doc.get("user") -> string)
    var score: int = (doc.get("score") -> int)
    var tags: vect<string> = (doc.get("tags") -> vect<string>)

    // 2. Сериализация структур данных Alpha обратно в JSON строку:
    var json_str: string = json.stringify(doc)
    print(json_str)
}
```

### Мощный модуль директорий и путей `Core/Fs.alpha`
Включает функции для работы с путями и папками на уровне пространства имен `Fs` в стиле Alpha с `?`, а также методы директорий через объект `dir` или `Fs.open(path)`:

```alpha
import Core/Fs.alpha

function main() [public, nothing] {
    // 1. Работа с путями:
    var full_path: string = Fs.join("logs", "app", "output.txt")
    var ext: string = Fs.extension(full_path)   // "txt"
    var fname: string = Fs.filename(full_path) // "output.txt"
    var stem: string = Fs.stem(full_path)       // "output"
    var parent: string = Fs.parent(full_path)   // "logs/app"
    var abs: string = Fs.abs(full_path)         // абсолютный путь на диске

    // 2. Проверка состояния в стиле Alpha:
    var exists: boolean = Fs.exists?(full_path)
    var is_file: boolean = Fs.file?(full_path)
    var is_dir: boolean = Fs.dir?("logs")

    // 3. Работа с директориями (через dir или Fs.open):
    var my_dir: Fs.Dir = Fs.open("logs")
    var only_files: vect<string> = (my_dir.files() -> vect<string>)
    var only_dirs: vect<string> = (my_dir.dirs() -> vect<string>)
    var has_log: boolean = my_dir.exists?("output.txt")

    // 4. Рекурсивный обход для for ... in:
    for item in my_dir.walk() {
        print(item)
    }
}
```

### Отложенное выполнение `make schedule` (defer)
Конструкция `make schedule` позволяет отложить выполнение функции или блока кода до выхода из текущей области видимости (блока `{ ... }` или функции). Отложенные инструкции выполняются в порядке **LIFO** (Last-In, First-Out — последняя объявленная инструкция выполняется первой) при любом выходе: при нормальном завершении, досрочном `return`, `break`, `continue` или при возникновении ошибки.

Поддерживается как откладывание функции `make schedule (any function)`, так и блок `make schedule { ... }` (также поддерживается ключевое слово `defer`):
```alpha
import Core/Io.alpha

function cleanup(path: string) [public, nothing] {
    print("Cleaning up: " + path)
}

function process_file(path: string) [public, nothing] {
    var file: Io.File = Io.open(path)
    
    // 1. Отложить вызов функции:
    make schedule (file.reset())

    // 2. Отложить блок кода:
    make schedule {
        print("Cleaning up resources for: " + path)
    }

    if !file.exists?() {
        return // make schedule гарантированно сработает перед выходом!
    }

    file.write("Processing data...")
}

function lifo_demo() [public, nothing] {
    make schedule (print("1"))
    make schedule (print("2"))
    make schedule (print("3"))
    // При выходе напечатает в порядке LIFO:
    // 3
    // 2
    // 1
}
```

### Проверка утверждений `verify` (Assert)
Оператор `verify` проверяет истинность логического условия. Если условие ложно, выполнение прерывается с ошибкой рантайма и указанным сообщением (которое также можно перехватить через `try ... catch`):

```alpha
verify condition, "error_message"
```

Пример:
```alpha
function divide(a: int, b: int) [public, int] {
    verify b != 0, "Делитель не может быть равен нулю!"
    return a / b
}

function check_player(name: string, score: int) [public, nothing] {
    verify !name.empty?(), "Имя игрока не может быть пустым"
    verify score >= 0, "Счёт игрока должен быть неотрицательным"
}
```
Если сообщение об ошибке опущено (`verify condition`), используется стандартное сообщение об ошибке утверждения.

---

## 18. Новые возможности языка (Enums, HexColor, Core/Cli, Модули, 1-based Indexing, Packages.alpaks)

### 18.1 Индексация с единицы (1-based Indexing)
Счёт элементов во всех коллекциях (**vect**, **tuple**, **string**, а также позиционный/целочисленный доступ в **map**) начинается с **единицы** (`1`):

```alpha
var v: vect<string> = vect.new()
v.push("first")
v.push("second")
v.push("third")

print(v[1]) // "first"
print(v[2]) // "second"
print(v[3]) // "third"

v[2] = "updated" // Запись по 1-based индексу
```

Срезы также отталкиваются от 1-based позиций:
```alpha
var s: string = "hello world"
var part: string = s[1..6]    // "hello" (символы с 1 по 5 включительно)
var tail: string = s[7...]    // "world" (символы с 7 до конца)
```

### 18.2 Перечисления (`enum`)
Синтаксис объявления `enum`:
```alpha
enum Status {
    Pending,
    Active,
    Done
}

enum Color {
    Red,
    Green,
    Blue
}
```

Использование вариантов enum:
```alpha
var s: Status = Status.Active

if s == Status.Active {
    print("Статус активен!")
}
```

### 18.3 Тип `hexcolor` и окрашивание вывода
Alpha имеет встроенный тип данных `hexcolor`, представляющий HEX-код цвета в формате `#(RRGGBB)` или `#(RGB)`:

```alpha
var color: hexcolor = #(FFF00)
var red: hexcolor = #(FF0000)
var cyan: hexcolor = #(00FFFF)
```

Функция `paint(text, hexcolor)` или метод `cli.paint(text, hexcolor)` окрашивают текст с использованием 24-битных ANSI escape-кодов для терминала:
```alpha
var title: string = paint("Alpha Engine", #(00FF66))
print(title)
```

### 18.4 Модуль `Core/Cli.alpha` и аргументы командной строки
Модуль `Core/Cli.alpha` предоставляет средства для работы с терминалом и CLI аргументами:

```alpha
import Core/Cli.alpha

function main() [public, nothing] {
    // Получение аргументов запуска (индексация с 1)
    var arguments: vect<string> = cli.arguments()

    if arguments.len() > 0 {
        printf("Первый аргумент: {}\n", arguments[1])
    }

    // Кастомный вывод в цвет терминала
    var banner: string = cli.paint(">>> SUCCESS <<<", #(00FF00))
    print(banner)
}
```

### 18.5 Пространства имён (`module`)
Синтаксис объявления модулей/неймспейсов:
```alpha
module MathTools {
    function add(a: int, b: int) [public, int] {
        return a + b
    }

    function square(n: int) [public, int] {
        return n * n
    }
}
```

Вызов функций модуля происходит через точку:
```alpha
var result: int = MathTools.add(10, 20)
var squared: int = MathTools.square(5)
```

### 18.6 Управление веб-библиотеками (`packages.alpaks`)
Начиная с этой версии, прямые импорты по URL (`import web::"..."`) в исходных файлах `.alpha` запрещены.
Все веб-библиотеки подключаются исключительно в корневом файле проекта **`packages.alpaks`**:

Синтаксис `packages.alpaks`:
```text
"alias" = import web::"https://pastebin.com/raw/..."
"toml" = import web::"https://example.com/alpha-toml/module.alpha"
"something" = import web::"https://pastebin.com/raw/dC4eYYwH"
```

После этого в любом `.alpha` файле проекта библиотека импортируется по присвоенному алиасу:
```alpha
import something
// или:
import toml
```
При создании нового проекта (`alpha here`) файл `packages.alpaks` генерируется автоматически.

