<div align="center">

<img src="https://avatars.githubusercontent.com/u/349414014" width="130" height="130" alt="Alpha Language Logo" style="border-radius: 50%; box-shadow: 0 4px 20px rgba(0,0,0,0.15);" />

# The Alpha Programming Language

**A modern, statically-typed, expressive language with intuitive 1-based indexing, powerful package management, and first-class developer ergonomics.**

[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg?style=for-the-badge)](Cargo.toml)
[![Build & Tests](https://img.shields.io/badge/tests-59%20passed-success.svg?style=for-the-badge)](tests/alpha_tests.rs)
[![License](https://img.shields.io/badge/license-MIT-purple.svg?style=for-the-badge)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20POSIX-orange.svg?style=for-the-badge)](#)

[Features](#-key-features) • [Installation](#-installation) • [Quick Start](#-quick-start) • [CLI Usage](#-cli-commands) • [Documentation](WIKI.md)

</div>

---

## 🌟 Key Features

- 🎯 **Intuitive 1-Based Indexing**: Natural element counting for vectors, tuples, strings, and maps (`v[1]` is the first item).
- 🧠 **Region-Based Arena Memory (`Core/ArenaAllocator`)**: Fast contiguous allocation with $O(1)$ batch reset and generational Use-After-Free safety.
- 🖼️ **Native Desktop GUI & Webview (`Core/Webview`)**: Modern Wry/Tao native desktop applications with HTML/JS/CSS interfaces and bidirectional IPC.
- 🔒 **Enterprise Strict Mode & `everything` Type**: `@strict` validation with universal non-nothing `everything` type checked at compile-time.
- 🎨 **First-Class HexColor & Terminal Painting**: Built-in `hexcolor` data type (`#(RRGGBB)`), `paint()` and `cli.paint()`.
- 📦 **Declarative Web Package Manager**: Remote dependencies are declared in `packages.alpaks` with clean aliases.
- 🧱 **Object-Oriented Structures & Enums**: Clear `structure` definitions with methods, constructors (`make`), and type-safe `enum` variants.
- 🗂️ **Modules & Namespaces**: Clean code organization with `module Name { ... }`.
- ⏳ **Guaranteed Deferred Execution**: LIFO resource cleanups using `make schedule { ... }`.
- 🛡️ **Robust Error Handling**: Expressive `try ... catch` blocks, `verify` assertions, and actionable diagnostic formatting.
- ⚡ **Batteries-Included Standard Library**: Core modules for File System (`Fs`), I/O (`Io`), Environment (`Env`), HTTP, JSON, Randomness, Math, and Time.
- 🖥️ **Conditional Target Compilation**: Cross-platform blocks with `!system win { ... }` and `!system posix { ... }`.

---

## 🚀 Quick Taste of Alpha

```alpha
import Core/Basic.alpha
import Core/Cli.alpha

enum Priority {
    Low,
    Medium,
    High
}

module GreetingService {
    function greet(name: string, p: Priority) [public, string] {
        var badge: string = (p == Priority.High) ? paint("[URGENT]", #(FF0000)) : paint("[NORMAL]", #(00FF66))
        return format("{} Hello, {}! Welcome to Alpha.", badge, name)
    }
}

function main() [public, nothing] {
    // 1-based CLI arguments
    var args: vect<string> = cli.arguments()
    var user: string = (args.len() >= 1) ? args[1] : "Developer"

    // Guaranteed scope cleanup
    make schedule {
        print(cli.paint("Execution completed successfully.", #(00FFFF)))
    }

    var message: string = GreetingService.greet(user, Priority.High)
    print(message)
}
```

---

## 📥 Installation

Alpha requires the **Rust** toolchain (Rust 2024 edition / 1.85+).

### 1. Clone the repository
```bash
git clone https://github.com/your-username/alpha-lang.git
cd alpha-lang
```

### 2. Build & Install CLI
Install the compiler binary directly into your Cargo bin path (`~/.cargo/bin`):
```bash
cargo install --path . --force
```

Verify that the CLI is accessible:
```bash
alpha -v
# Output: Alpha version 0.1.0
```

### 3. Run the Test Suite
```bash
cargo test
# Runs all 52 automated integration tests across the parser, typechecker, and runtime.
```

---

## 🛠️ Quick Start

### 1. Creating a New Project
Initialize a fresh project directory using `alpha here`:
```bash
mkdir my_project
cd my_project
alpha here my_project
```
This generates the standard Alpha project layout:
```text
my_project/
├── alpha-project.toml    # Project metadata (name, version)
├── packages.alpaks       # Declarative remote web packages
└── src/
    └── main.alpha        # Application entry point
```

### 2. Managing Dependencies (`packages.alpaks`)
Remote web libraries cannot be scattered across source files. Instead, declare them in `packages.alpaks` with user-friendly aliases:
```text
# packages.alpaks
"something" = import web::"https://pastebin.com/raw/dC4eYYwH"
"json_extra" = import web::"https://example.com/alpha-lib/json.alpha"
```

Then simply import the alias in your source code:
```alpha
// src/main.alpha
import Core/Basic.alpha
import something

function main() [public, nothing] {
    print(something())
}
```

### 3. Running Your Project
Run the project in interpreted mode with immediate execution:
```bash
alpha run
```
You can also run any standalone `.alpha` file and pass CLI arguments:
```bash
alpha run src/main.alpha --verbose config.json
# or simply:
alpha src/main.alpha
```

### 4. Checking Types and Syntax
Verify syntax and static types without running the program:
```bash
alpha check
# or
alpha check src/main.alpha
```

### 5. Compiling to Native Executable
Compile your project into an optimized native executable (`.exe` on Windows):
```bash
alpha build
```
The compiled binary will be placed in the `build/` folder:
```bash
./build/my_project.exe
```

---

## 💻 CLI Commands

| Command | Description |
| :--- | :--- |
| `alpha here [name]` | Initialize a new Alpha project in the current directory |
| `alpha run [file] [args...]` | Execute an Alpha project or script with arguments |
| `alpha check [file]` | Validate syntax and run static typechecker |
| `alpha build [file]` | Compile file or project into native binary in `build/` |
| `alpha -v`, `--version` | Display Alpha compiler and runtime version |
| `alpha -h`, `--help` | Display CLI help and usage manual |

---

## 📚 Complete Language Manual

For an exhaustive guide covering types, control structures, pattern matching, I/O, file systems, scheduled deferred execution (`make schedule`), networking, and all standard library modules, read our **[Full WIKI Documentation (English & Russian)](WIKI.md)**.

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
