# Alpha Programming Language

A modern, statically-typed programming language implemented in Rust, adhering strictly to the [syntax.md](syntax.md) specification.

---

## Architecture Overview

Alpha is structured into clean, decoupled, and modular compiler and runtime packages:

```
src/
├── main.rs                   # CLI executable entry point
├── lib.rs                    # Root library and public re-exports
│
├── diagnostics/              # Error handling and source diagnostics
│   ├── mod.rs
│   └── error.rs              # Spans, diagnostic formatting, and caret highlighting
│
├── frontend/                 # Lexing, parsing, and AST
│   ├── mod.rs
│   ├── token.rs              # Tokens, keywords, and punctuation
│   ├── lexer.rs              # Tokenizer with line/col tracking
│   ├── ast.rs                # Abstract Syntax Tree nodes
│   └── parser.rs             # Pratt & recursive-descent parser
│
├── semantics/                # Static analysis and type checking
│   ├── mod.rs
│   ├── types.rs              # Type system representations and union compatibility
│   └── typechecker.rs        # Static type checker and scope verification
│
├── runtime/                  # Runtime execution engine and stdlib
│   ├── mod.rs
│   ├── value.rs              # Dynamic values (Vect, Map, Io.File, Fs.Dir)
│   ├── env.rs                # Scoped variable environments
│   ├── stdlib.rs             # Core library bindings (Basic, Io, Fs, Env)
│   └── interpreter.rs        # Tree-walking interpreter for `alpha run`
│
├── backend/                  # Code generation and native compilation
│   ├── mod.rs
│   └── compiler.rs           # Native executable compiler invoking `rustc` (`alpha build`)
│
├── driver/                   # Project manager and CLI interface
│   ├── mod.rs
│   ├── project.rs            # `alpha-project.toml` manager (`alpha here`)
│   └── cli.rs                # CLI command dispatcher (`here`, `build`, `run`, `check`)
│
└── Core/                     # Standard Core library alpha scripts
    ├── Basic.alpha
    ├── Io.alpha
    ├── Fs.alpha
    └── Env.alpha
```

---

## Language Features

- **Data Types**: `int`, `float`, `string`, `char`, `boolean`, `something`, `nothing`, `vect<T>`, `map<K, V>`.
- **Union Types**: Return types can specify multiple variants, such as `<int, nothing>` or `<string, int>`.
- **Variables & Constants**: `var x: int = 10` (mutable) and `const var max: int = 100` (immutable).
- **Functions**: Visibility and return types are specified in metadata brackets:
  ```alpha
  function is_valid?() [public, boolean] {
      return true
  }

  function get_score(name: string) [public, <int, nothing>] {
      if scores.contains(name) {
          return scores.get(name)
      } else {
          return nothing
      }
  }
  ```
- **Optional Semicolons**: Semicolons `;` at the end of statements are optional.
- **Loops**:
  - Infinite: `for { ... }`
  - Counter: `for i = 0, i < 5, i++ { ... }`
  - Collection iteration: `for each player in players { ... }`
  - Map iteration: `for each key, value in scores { ... }`
- **Nothing Checks**: Universal `.nothing?()` method available on any type.
- **Error Handling**:
  - Compiler and runtime errors format with clear diagnostics, line snippets, caret markers, and suggestions:
    ```
    Compile Error: Expected ':', found 'int'
      --> src/main.alpha:1:9
       |
       1 | var age int = 25
         |         ^^^
       = Hint: Types must be preceded by a colon, e.g. 'var x: int = 10'
    ```
  - Catch runtime errors via `try { ... } catch err { ... }` blocks.
  - Trigger halts via `error("message: {}", val)`.
- **Core Standard Library**:
  - `Core/Basic.alpha`: `print()`, `printf()`, `format("{2}:{2}", a, b)`, `error()`.
  - `Core/Io.alpha`: `Io.open(path)`, `.content()`, `.write()`, `.reset()`, `.something?()`, `.empty?()`, `.linelen()`, `.symlen()`, `.bytelen()`.
  - `Core/Fs.alpha`: `dir.open(path)`, `dir.create()`, `dir.delete()`, `dir.reset()`, `dir.file.create()`, `dir.get_children()`, `dir.nothing?()`, `dir.empty?()`.
  - `Core/Env.alpha`: `env.current`, `env.exec_dir`.
  - `Core/Math.alpha`: `abs(x)`, `max(a, b)`, `min(a, b)`, `clamp(val, min, max)`, `pow(base, exp)`.

---

## Creating Custom Core Modules in Alpha

You can create your own custom modules written in Alpha under the `Core/` folder (either `src/Core/` or `Core/` in your project root). Any functions marked `public` become accessible when imported:

```alpha
// src/Core/Crypto.alpha
function hash_simple(text: string) [public, int] {
    var h: int = 0
    for i = 0, i < text.len(), i++ {
        h = h * 31 + text.get(i)
    }
    return h
}

function internal_helper() [here, nothing] {
    // 'here' marks this function private to this file
}
```

Then simply import and use it anywhere in your Alpha code:

```alpha
// src/main.alpha
import Core/Crypto.alpha
import Core/Basic.alpha

function main() [public, nothing] {
    var code: int = hash_simple("Alpha")
    printf("Hash: {}\n", code)
}
```

---

## Showcase Programs (`tests/alpha_programs/`)

A collection of interesting programs written in Alpha demonstrating various language capabilities:

1. [01_math_and_primes.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/01_math_and_primes.alpha):
   - Fibonacci sequence generation.
   - Prime number checker with `is_prime?()` syntax.
   - Core Math functions (`abs`, `max`, `pow`).

2. [02_game_inventory.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/02_game_inventory.alpha):
   - RPG inventory and merchant system.
   - Generic vectors `vect<string>` and maps `map<string, int>`.
   - Functions returning union types `<int, nothing>` with `.nothing?()` validations.

3. [03_file_and_system.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/03_file_and_system.alpha):
   - File system handling via `Fs.Dir` and `Io.File`.
   - Timestamp zero-padded formatting (`format("[{2}:{2}:{2}] ...")`).
   - Line, character, and byte counting, directory traversal and cleanup.

4. [04_error_resilience.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/04_error_resilience.alpha):
   - Error management using `try { ... } catch err { ... }`.
   - Safe division and parameter boundary validation with custom halt reporting.

5. [05_task_manager.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/05_task_manager.alpha):
   - Formatted ASCII table dashboard.
   - Multi-map task tracking and collection operations.

6. [06_matrix_and_algorithms.alpha](file:///D:/RustProjects/alpha-lang/tests/alpha_programs/06_matrix_and_algorithms.alpha):
   - In-place Bubble Sort algorithm.
   - Vector indexing and mutation (`arr.set(j, val)`).

Run any showcase program with:
```bash
cargo run -- tests/alpha_programs/01_math_and_primes.alpha
```

---

## Building and Testing

```bash
# Build the compiler in release mode
cargo build --release

# Run the complete test suite (15 unit & integration tests)
cargo test
```

### 1. Initialize a Project (`alpha here`)
Initializes an Alpha project with `alpha-project.toml` and `src/main.alpha`:
```bash
alpha here my_project
```

### 2. Run Directly (`alpha run`)
Executes an Alpha project or script using the built-in runtime engine:
```bash
alpha run src/main.alpha
# or run in project root
alpha run
```

### 3. Native Compilation (`alpha build`)
Alpha compiles the project into a native executable in the `build/` directory:
```bash
alpha build
# Produces: build/my_project.exe (on Windows) or build/my_project (on Linux/macOS)
```

### 4. Static Verification (`alpha check`)
Verifies syntax and runs static type checking without executing:
```bash
alpha check src/main.alpha
```

---

## Editor Support (Zed)

An extension for the [Zed](https://zed.dev) editor is available in [`editors/zed-alpha/`](file:///D:/RustProjects/alpha-lang/editors/zed-alpha/README.md).
It provides:
- Full syntax highlighting powered by Tree-sitter.
- Automatic bracket pairing, quotes, and smart indentation.
- Outline panel symbol navigation (`Ctrl+Shift+O`).

To install it in Zed:
1. Open Zed and press `Ctrl+Shift+P` (or `Cmd+Shift+P` on macOS).
2. Run `zed: install dev extension`.
3. Select the folder `editors/zed-alpha`.

---

## Building and Testing

```bash
# Build the compiler in debug or release mode
cargo build --release

# Run the complete test suite (all 15 unit & integration tests)
cargo test
```
