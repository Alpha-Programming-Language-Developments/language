use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use serde_json::{json, Value};
use crate::diagnostics::error::AlphaError;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::typechecker::TypeChecker;

pub struct LspServer {
    documents: HashMap<String, String>,
}

impl LspServer {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = io::BufReader::new(stdin.lock());
        let mut stdout = io::stdout().lock();

        loop {
            // Read headers
            let mut content_length: Option<usize> = None;
            loop {
                let mut line = String::new();
                let bytes_read = reader.read_line(&mut line)?;
                if bytes_read == 0 {
                    return Ok(()); // EOF
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    break; // Header section ended
                }
                if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                    if let Ok(len) = rest.trim().parse::<usize>() {
                        content_length = Some(len);
                    }
                }
            }

            let len = match content_length {
                Some(l) => l,
                None => continue,
            };

            let mut body_buf = vec![0u8; len];
            reader.read_exact(&mut body_buf)?;

            let msg_str = match String::from_utf8(body_buf) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let req: Value = match serde_json::from_str(&msg_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            self.handle_message(&req, &mut stdout)?;
        }
    }

    fn send_response<W: Write>(&self, out: &mut W, resp: Value) -> io::Result<()> {
        let payload = resp.to_string();
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        out.write_all(header.as_bytes())?;
        out.write_all(payload.as_bytes())?;
        out.flush()
    }

    fn handle_message<W: Write>(&mut self, msg: &Value, out: &mut W) -> io::Result<()> {
        let method = msg.get("method").and_then(|m| m.as_str());
        let id = msg.get("id");

        match method {
            Some("initialize") => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1, // Full sync
                            "completionProvider": {
                                "resolveProvider": false,
                                "triggerCharacters": [".", "@", ":"]
                            },
                            "hoverProvider": true
                        },
                        "serverInfo": {
                            "name": "alpha-lsp",
                            "version": "1.0.0"
                        }
                    }
                });
                self.send_response(out, resp)?;
            }

            Some("initialized") => {
                // Client confirmed initialization; notification, no response needed
            }

            Some("shutdown") => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                });
                self.send_response(out, resp)?;
            }

            Some("exit") => {
                std::process::exit(0);
            }

            Some("textDocument/didOpen") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    let text = params["textDocument"]["text"].as_str().unwrap_or_default().to_string();
                    self.documents.insert(uri.clone(), text);
                    self.publish_diagnostics(out, &uri)?;
                }
            }

            Some("textDocument/didChange") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            if let Some(text) = last["text"].as_str() {
                                self.documents.insert(uri.clone(), text.to_string());
                                self.publish_diagnostics(out, &uri)?;
                            }
                        }
                    }
                }
            }

            Some("textDocument/didSave") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    self.publish_diagnostics(out, &uri)?;
                }
            }

            Some("textDocument/didClose") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    self.documents.remove(&uri);
                    // Clear diagnostics
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": {
                            "uri": uri,
                            "diagnostics": []
                        }
                    });
                    self.send_response(out, notif)?;
                }
            }

            Some("textDocument/completion") => {
                let items = self.get_completions(msg);
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": items
                });
                self.send_response(out, resp)?;
            }

            Some("textDocument/hover") => {
                let hover_info = self.get_hover(msg);
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": hover_info
                });
                self.send_response(out, resp)?;
            }

            _ => {
                // If it's a request with an id, reply with method not found or null
                if let Some(req_id) = id {
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": null
                    });
                    self.send_response(out, resp)?;
                }
            }
        }

        Ok(())
    }

    fn publish_diagnostics<W: Write>(&self, out: &mut W, uri: &str) -> io::Result<()> {
        let text = match self.documents.get(uri) {
            Some(t) => t,
            None => return Ok(()),
        };

        let errors = validate_alpha_code(text);
        let mut diagnostics = Vec::new();

        for err in errors {
            let line = if err.span.line > 0 { err.span.line - 1 } else { 0 };
            let col = if err.span.col > 0 { err.span.col - 1 } else { 0 };
            let len = (err.span.end.saturating_sub(err.span.start)).max(1);

            let message = if let Some(hint) = &err.hint {
                format!("{}\nHint: {}", err.message, hint)
            } else {
                err.message.clone()
            };

            diagnostics.push(json!({
                "range": {
                    "start": { "line": line, "character": col },
                    "end": { "line": line, "character": col + len }
                },
                "severity": 1, // Error
                "source": "alpha",
                "message": message
            }));
        }

        let notif = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics
            }
        });

        self.send_response(out, notif)
    }

    fn get_completions(&self, _msg: &Value) -> Vec<Value> {
        let mut completions = Vec::new();

        // 1. Language Keywords (Kind = 14)
        let keywords = [
            ("function", "Function declaration: function name(arg: type) -> type { ... }"),
            ("var", "Mutable variable declaration: var x: int = 10"),
            ("const var", "Immutable constant declaration: const var PI: float = 3.14"),
            ("if", "Conditional statement: if cond { ... }"),
            ("else", "Else branch: else { ... }"),
            ("elseif", "Else-if branch: elseif cond { ... }"),
            ("for", "Loop statement: for 1...5 { ... } or for i in vect { ... }"),
            ("in", "Iteration target: for item in collection"),
            ("return", "Return value from function"),
            ("check", "Pattern matching: check value { pattern -> expr }"),
            ("make", "Instantiate structure: make Person { name: \"Alex\" }"),
            ("structure", "Structure declaration: structure Name { field: type }"),
            ("enum", "Enum declaration: enum Color { Red, Green, Blue }"),
            ("module", "Module declaration: module MyMod { ... }"),
            ("try", "Error handling: try { ... } catch err { ... }"),
            ("catch", "Catch block: catch err { ... }"),
            ("verify", "Assertion statement: verify cond, \"Error message\""),
            ("defer", "Deferred statement: defer Io.println(\"done\")"),
            ("import", "Import module or file: import \"math.alpha\""),
            ("system", "System directive block: !system { ... }"),
            ("@strict", "Strict typechecking directive. Enforces explicit typing, safety, and disallows force unwrap ('!?') or 'something'/'nothing'"),
            ("true", "Boolean literal true"),
            ("false", "Boolean literal false"),
            ("this", "Self-reference inside structure method"),
        ];

        for (kw, doc) in keywords {
            completions.push(json!({
                "label": kw,
                "kind": 14, // Keyword
                "detail": "Alpha Keyword",
                "documentation": {
                    "kind": "markdown",
                    "value": doc
                }
            }));
        }

        // 2. Primitive Types (Kind = 7)
        let types = [
            ("int", "64-bit signed integer"),
            ("unt", "64-bit unsigned integer"),
            ("float", "64-bit floating point number"),
            ("string", "UTF-8 string"),
            ("char", "Unicode character"),
            ("boolean", "Boolean flag (true/false)"),
            ("vect", "Dynamically sized vector: vect<T>"),
            ("map", "Associative key-value map: map<K, V>"),
            ("something", "Dynamic type representing any value (forbidden in @strict mode)"),
            ("nothing", "Absence of a value / void (forbidden in @strict mode)"),
        ];

        for (ty, doc) in types {
            completions.push(json!({
                "label": ty,
                "kind": 7, // Class / Type
                "detail": "Alpha Primitive Type",
                "documentation": {
                    "kind": "markdown",
                    "value": doc
                }
            }));
        }

        // 3. Standard Library Namespaces & Functions
        let stdlib = [
            // Time
            ("Time.benchmark", "Time.benchmark({ ... })\n\nMeasures the execution time of a code block in milliseconds.", "Time.benchmark({\n\t$0\n})"),
            ("Time.now", "Time.now() -> int\n\nReturns current timestamp in seconds.", "Time.now()"),
            ("Time.now_ms", "Time.now_ms() -> int\n\nReturns current timestamp in milliseconds.", "Time.now_ms()"),
            ("Time.sleep", "Time.sleep(ms: int)\n\nSleeps current thread for specified milliseconds.", "Time.sleep(${1:1000})"),
            ("Time.elapsed", "Time.elapsed(start_ms: int) -> int\n\nReturns elapsed milliseconds since start_ms.", "Time.elapsed(${1:start})"),
            // Io
            ("Io.println", "Io.println(val: something)\n\nPrints value followed by newline.", "Io.println(${1:value})"),
            ("Io.print", "Io.print(val: something)\n\nPrints value without trailing newline.", "Io.print(${1:value})"),
            ("Io.input", "Io.input(prompt: string) -> string\n\nReads line of input from stdin with prompt.", "Io.input(\"${1:prompt}: \")"),
            // Fs
            ("Fs.read", "Fs.read(path: string) -> string\n\nReads file contents as string.", "Fs.read(${1:\"path\"})"),
            ("Fs.write", "Fs.write(path: string, content: string)\n\nWrites string to file.", "Fs.write(${1:\"path\"}, ${2:content})"),
            ("Fs.append", "Fs.append(path: string, content: string)\n\nAppends string to file.", "Fs.append(${1:\"path\"}, ${2:content})"),
            ("Fs.exists", "Fs.exists(path: string) -> boolean\n\nChecks if path exists.", "Fs.exists(${1:\"path\"})"),
            // Math
            ("Math.abs", "Math.abs(x: int/float) -> int/float", "Math.abs(${1:x})"),
            ("Math.min", "Math.min(a, b)", "Math.min(${1:a}, ${2:b})"),
            ("Math.max", "Math.max(a, b)", "Math.max(${1:a}, ${2:b})"),
            ("Math.pow", "Math.pow(base, exp)", "Math.pow(${1:base}, ${2:exp})"),
            ("Math.sqrt", "Math.sqrt(x)", "Math.sqrt(${1:x})"),
            ("Math.floor", "Math.floor(x)", "Math.floor(${1:x})"),
            ("Math.ceil", "Math.ceil(x)", "Math.ceil(${1:x})"),
            ("Math.round", "Math.round(x)", "Math.round(${1:x})"),
        ];

        for (lbl, doc, snippet) in stdlib {
            completions.push(json!({
                "label": lbl,
                "kind": 3, // Function
                "detail": "Standard Library",
                "insertText": snippet,
                "insertTextFormat": 2, // Snippet
                "documentation": {
                    "kind": "markdown",
                    "value": doc
                }
            }));
        }

        completions
    }

    fn get_hover(&self, msg: &Value) -> Value {
        let params = match msg.get("params") {
            Some(p) => p,
            None => return json!(null),
        };
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return json!(null),
        };
        let pos = &params["position"];
        let target_line = pos["line"].as_u64().unwrap_or(0) as usize;
        let target_col = pos["character"].as_u64().unwrap_or(0) as usize;

        let text = match self.documents.get(uri) {
            Some(t) => t,
            None => return json!(null),
        };

        let lines: Vec<&str> = text.lines().collect();
        if target_line >= lines.len() {
            return json!(null);
        }
        let line = lines[target_line];
        if target_col >= line.len() {
            return json!(null);
        }

        // Extract identifier/token under cursor
        let chars: Vec<char> = line.chars().collect();
        let mut start = target_col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_' || chars[start - 1] == '@' || chars[start - 1] == '.') {
            start -= 1;
        }
        let mut end = target_col;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_' || chars[end] == '@' || chars[end] == '.') {
            end += 1;
        }

        let word: String = chars[start..end].iter().collect();

        let hover_text = match word.as_str() {
            "@strict" => "### `@strict` Directive\n\nEnforces professional, bullet-proof strict typing from this line onward:\n- Box / union types **must** be handled via `check` (pattern matching); force unwrap (`!?`) is strictly forbidden.\n- Every variable must have an explicit type annotation.\n- Types and values `something` and `nothing` are strictly forbidden.",
            "unt" => "### `unt` Type\n\n64-bit unsigned integer (`0` to `18_446_744_073_709_551_615`).",
            "int" => "### `int` Type\n\n64-bit signed integer (`-9_223_372_036_854_775_808` to `9_223_372_036_854_775_807`).",
            "float" => "### `float` Type\n\n64-bit IEEE-754 double precision floating point number.",
            "string" => "### `string` Type\n\nUTF-8 encoded dynamic string.",
            "Time.benchmark" | "benchmark" => "### `Time.benchmark({ ... })`\n\nMeasures the execution duration of a code block in milliseconds and returns the result as `int`.",
            "Time" => "### `Time` Module\n\nProvides standard functions for querying high-resolution timestamps, sleep, and benchmarking.",
            "Io" => "### `Io` Module\n\nStandard input/output functions (`print`, `println`, `input`, `read_file`, `write_file`).",
            "Fs" => "### `Fs` Module\n\nFilesystem operations (`read`, `write`, `append`, `exists`, `delete`, `copy`, `move`, `list`).",
            "Math" => "### `Math` Module\n\nMathematical utility functions and constants.",
            _ => return json!(null),
        };

        json!({
            "contents": {
                "kind": "markdown",
                "value": hover_text
            }
        })
    }
}

pub fn validate_alpha_code(source: &str) -> Vec<AlphaError> {
    let mut lexer = Lexer::new(source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(err) => return vec![err],
    };

    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(err) => return vec![err],
    };

    let mut typechecker = TypeChecker::new();
    if let Err(err) = typechecker.check_program(&program) {
        return vec![err];
    }

    Vec::new()
}
