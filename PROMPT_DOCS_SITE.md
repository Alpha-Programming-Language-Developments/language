# Prompt: Generate the Complete Official Alpha Language Documentation Site

You are a Senior Technical Writer and Principal Systems Engineer. Your task is to build a complete, production-grade, bilingual (English & Russian) documentation site for the **Alpha Programming Language**.

The site must be generated inside the `docs-site/` directory as a standalone single-page application (SPA) with no build tools or package managers required. It must open and function perfectly just by double-clicking `index.html` in any modern web browser.

---

## 1. Design & UI Requirements (Strict)

1. **Monochrome Minimalist Style (GitBook 2026 Black & White)**:
   - Base Palette: `#000000` (pure black background), `#ffffff` (text), `#141414` (sidebar & card backgrounds), `#222222` (subtle borders), `#888888` (muted labels/secondary text).
   - **Colors to avoid**: Do NOT use orange, purple, green, or rainbow brand accents. The entire UI must be strictly clean monochrome.
2. **Typography**:
   - Modern clean sans-serif (`Inter`, `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `system-ui`, `sans-serif`).
   - Monospace for code: `JetBrains Mono`, `Consolas`, `ui-monospace`, `monospace`.
3. **Zero Border Artifacts Around Syntax Tokens**:
   - Code blocks must have clean dark backgrounds with syntax highlighting.
   - Every `<span>` token inside code blocks MUST have `border: none !important; outline: none !important; background: transparent !important`.
4. **No Emojis — Vector Icons Only**:
   - Zero emojis anywhere in the navigation, badges, callouts, or controls.
   - Use crisp, inline SVGs for all icons (search magnifying glass, language globe, arrows, callout icons, copy button).
5. **Branding**:
   - Logo: `https://avatars.githubusercontent.com/u/349414014`
   - Header title: **ALPHA** with a small monospace badge `v0.9.0`.
   - Never use "Powered by Alpha Engine" or third-party platform badges.
6. **Bilingual Support (EN / RU)**:
   - Instant language switcher in the top bar (EN / RU) that preserves current scroll or chapter.
   - Full, thorough, and authentic content in both languages (not placeholder summaries).
7. **Interactive Features**:
   - Instant client-side search dialog triggered by `Ctrl+K` or `/`.
   - Right-side "On This Page" table of contents (TOC) that syncs with page headings.
   - Copy-to-clipboard button on every code block with a confirmation state.
   - Previous / Next Chapter navigation buttons at the bottom of every page.
   - Clean feedback buttons ("Was this helpful? Yes / No").

---

## 2. Core Language Truths & Rules (Do Not Violate)

1. **1-Based Indexing**: Alpha is strictly 1-based (`arr[1]` is the first element; `len(arr)` is the last valid index).
2. **No Semicolons**: Statements end at newlines.
3. **Explicit Typing**: `var x: int = 10`, `const var PI: float = 3.14159`.
4. **Function Syntax**: `function name(arg: type) [visibility, return_type] { ... }`.
5. **Visibility Modifiers**: `[public, ...]`, `[here, ...]` (private/module-local).
6. **Conditionals**: Use `if`, `elseif` (strictly one word, NOT `else if`), and `else`. Ternary expressions require a colon: `if (cond): then_expr else else_expr`.
7. **No Garbage Collector & Memory**: Alpha has NO GC. Scriptor/arena allocations use `Core/ArenaAllocator.alpha` (`mem.arena.allocate(capacity)`, `sector.insert()`, `sector.reset()`). There is NO `!system` block and NO `mem.arena(...) {}` block syntax.
8. **Strict Mode**: `@strict` enables compile-time safety (disallows `something`, unhandled returns, unsafe casts).
9. **NO Core/Net**: There is NO network/socket module (`Core/Net`) in the standard library.
10. **NO Multithreading / Concurrency**: Alpha does NOT have `thread.spawn`, `channel.new()`, or built-in concurrency primitives.
11. **WebAlph is an External Package**: WebAlph is NOT a built-in compiler library! It is an external ecosystem package installed into `packages.alpaks` via `alpha pkg https://github.com/.../webalph`.
12. **Native Webview**: `Webview` in standard library uses the OS-native webview (WebView2 on Windows, WebKit on Linux/macOS) communicating through JSON IPC.
13. **Unified Visibility Model (Private to 1 File by Default)**: Everything in Alpha is private to 1 file by default. Any structure, enum, variable, constant, or custom type works strictly in its defining file unless explicitly marked with `public`:
    - `public structure Name { ... }` (vs private `structure Name { ... }`)
    - `public enum Name { ... }` (vs private `enum Name { ... }`)
    - `public var name: type = val` (vs private `var name: type = val`)
    - `public const var NAME: type = val` (vs private `const var NAME: type = val`)
    - `public var MyType: type = ...` (vs private `var MyType: type = ...`)
    - Without `public`, any reference from another file triggers a compile-time visibility error: `... is private ('here')`.
14. **Custom Type Aliases (`var mytype: type = ...`)**: `type` is a first-class language meta-type. Users can define custom type aliases using standard variable syntax: e.g. `var mytype: type = vect<map<int, string>>` (file-private) or `public var mytype: type = vect<map<int, string>>` (exported across files upon import).

---

## 3. Verified Standard Library (Core Modules)

Only document the verified modules shipped with Alpha:
1. `Core/Basic.alpha`: `print`, `println`, `printf`, `format`, `error`.
2. `Core/Io.alpha`: `Io.open`, `Io.read`, `Io.write`, `Io.copy`, `Io.remove`, `Io.getline(path, line_number)`, `file.getline`.
3. `Core/Fs.alpha`: `Fs.exists?(path)`, `Fs.file?(path)`, `Fs.dir?(path)`, `Fs.mkdir(path)`, `Fs.remove_dir(path)`, `Fs.list_dir(path)`.
4. `Core/Cli.alpha`: `cli.arguments()`, `cli.paint(text, color)`, `paint(text, #(hexcolor))`.
5. `Core/Env.alpha`: `env.current` (working dir), `env.exec_dir` (executable dir).
6. `Core/Time.alpha`: `time.now()`, `time.now_ms()`, `time.sleep(ms)`, `time.elapsed(start_ms)`.
7. `Core/Math.alpha`: `Math.min`, `Math.max`, `Math.abs`, `Math.clamp`, `Math.gcd`, `Math.factorial`.
8. `Core/Machine.alpha`: `console.out(command)` (terminal shell execution).
9. `Core/ArenaAllocator.alpha`: `mem.arena.allocate(cap)`, `sector.insert(obj)`, `sector.reset()`, `sector.count()`, `sector.allocated()`.
10. `Core/Webview.alpha`: `Webview.open`, `Webview.on_message`, `Webview.eval`, `Webview.run`.

---

## 4. Chapters Structure (15 Chapters Total)

Create exactly 15 chapters in both `chapters-en.js` and `chapters-ru.js`:
1. `intro`: Introduction & Philosophy
2. `getting-started`: Installation & Quickstart
3. `syntax-variables`: Syntax Basics, Variables & 1-Based Indexing
4. `types-primitives`: Type System & Primitive Types
5. `collections`: Collections: Vectors, Maps & Tuples
6. `control-flow`: Control Flow: If, Elseif, Check & Loops
7. `functions`: Functions & Procedures
8. `structures-oop`: Structures & Object-Oriented Programming
9. `enums-matching`: Enums & Pattern Matching
10. `modules-packages`: Modules, Imports & Package Manager
11. `visibility`: Visibility & Encapsulation System (Private by default, public modifiers)
12. `strict-errors`: Strict Mode (@strict) & Error Handling
13. `memory-system`: Arena Memory Allocator (Core/ArenaAllocator)
14. `stdlib-cores`: Standard Library (Core Modules)
15. `webalph-tooling`: External WebAlph GUI Package & Testing
