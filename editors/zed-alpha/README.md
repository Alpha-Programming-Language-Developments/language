# Расширение Alpha для редактора Zed

Официальное расширение для редактора **Zed**, обеспечивающее полноценную поддержку и подсветку синтаксиса языка программирования **Alpha** (`.alpha`) на основе парсера **Tree-sitter**.

---

## 🌟 Возможности расширения

* **Полная подсветка синтаксиса (Syntax Highlighting)**:
  * Ключевые слова (`function`, `var`, `const`, `if`, `else`, `for`, `each`, `in`, `return`, `try`, `catch`, `import`, `public`, `here`).
  * Логические операторы-слова (`and`, `or`, `not`).
  * Встроенные и пользовательские типы (`int`, `float`, `string`, `char`, `boolean`, `something`, `nothing`, `vect<T>`, `map<K, V>`, `<int, nothing>`, `Io.File`, `Fs.Dir`).
  * Функции и методы, включая идентификаторы с суффиксом `?` (`is_prime?`, `nothing?`, `empty?`, `something?`).
  * Литералы: строки с escape-последовательностями, символы, числа, булевы значения, `nothing`.
  * Однострочные (`// ...`) и многострочные (`/* ... */`) комментарии.
* **Автозакрытие и подсветка парных скобок**: `{ }`, `[ ]`, `( )`, `" "`, `' '`.
* **Автоматические отступы (Smart Indents)** для блоков и списков параметров.
* **Символьная панель (Outline Panel)**: отображение объявленных функций в панели символов Zed (`Ctrl+Shift+O`).

---

## 🚀 Как загрузить и установить расширение в Zed

Zed поддерживает прямую установку разрабатываемых расширений (**Dev Extensions**) без необходимости публикации в маркетплейсе.

### Пошаговая инструкция:

1. **Запустите редактор Zed**.
2. Откройте **Командную палитру** (Command Palette):
   * На Windows / Linux: нажмите `Ctrl + Shift + P`
   * На macOS: нажмите `Cmd + Shift + P`
3. Начните вводить и выберите команду:
   ```text
   zed: install dev extension
   ```
4. В появившемся окне выбора папки выберите директорию:
   ```
   D:\RustProjects\alpha-lang\editors\zed-alpha
   ```
   *(папку, в которой находится файл `extension.toml`)*.
5. Нажмите **«Выбрать папку»** (Select Folder).

### Готово! 🎉
Zed автоматически:
1. Загрузит манифест `extension.toml`.
2. Скомпилирует грамматику Tree-sitter из вложенной папки `tree-sitter-alpha` в WebAssembly (Wasm).
3. Зарегистрирует язык **Alpha** для всех файлов с расширением `.alpha`.

---

## 🔍 Проверка работы

1. Откройте в Zed любой файл программы Alpha, например:
   * `tests/alpha_programs/01_math_and_primes.alpha`
   * `tests/alpha_programs/02_game_inventory.alpha`
   * `tests/alpha_programs/03_file_and_system.alpha`
2. В правом нижнем углу строки состояния Zed вы увидите язык **Alpha**.
3. Синтаксис будет красиво подсвечен в соответствии с вашей текущей цветовой темой Zed (One Dark, Gruvbox, Nord, Tokyo Night и т.д.).
4. Нажмите `Ctrl + Shift + O` — откроется список функций в файле (Outline).

---

## 🛠️ Настройка и пересборка грамматики (для разработчиков)

Если вы решите изменить правила грамматики в `tree-sitter-alpha/grammar.js`:

1. Убедитесь, что у вас установлен Node.js (`node -v`).
2. Перейдите в папку грамматики:
   ```bash
   cd editors/zed-alpha/tree-sitter-alpha
   ```
3. Сгенерируйте обновленный парсер:
   ```bash
   npx tree-sitter-cli generate
   ```
4. Зафиксируйте изменения в git-репозитории грамматики:
   ```bash
   git add .
   git commit -m "Update grammar rules"
   ```
5. В Zed перезагрузите расширения через командную палитру (`Ctrl+Shift+P`):
   ```text
   zed: reload extensions
   ```

---

## 📂 Структура расширения

```
editors/zed-alpha/
├── extension.toml                     # Манифест расширения для Zed
├── README.md                          # Данное руководство
│
├── languages/
│   └── alpha/
│       ├── config.toml                # Конфигурация языка (.alpha, комментарии, скобки)
│       ├── highlights.scm             # Запросы Tree-sitter для подсветки синтаксиса
│       ├── brackets.scm               # Правила парных скобок
│       ├── indents.scm                # Правила автоматических отступов
│       └── outline.scm                # Правила символов для Outline панели
│
└── tree-sitter-alpha/                 # Репозиторий грамматики Tree-sitter
    ├── grammar.js                     # Определение синтаксиса языка Alpha
    ├── package.json                   # Метаданные пакета Tree-sitter
    └── src/
        ├── parser.c                   # Сгенерированный компилируемый C-парсер
        ├── grammar.json
        ├── node-types.json
        └── tree_sitter/               # Заголовочные файлы Tree-sitter C runtime
```
