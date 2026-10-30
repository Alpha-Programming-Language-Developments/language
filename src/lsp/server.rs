use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use serde_json::{json, Value};
use crate::diagnostics::error::{AlphaError, Span};
use crate::frontend::ast::{AttachedMethod, Expr, FunctionDecl, ImportTarget, LiteralValue, Program, Statement};
use crate::semantics::types::Type;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::typechecker::TypeChecker;

pub struct LspServer {
    documents: HashMap<String, String>,
}

fn normalize_uri(uri: &str) -> String {
    let unescaped = uri.replace("%3A", ":").replace("%3a", ":");
    unescaped.to_lowercase()
}

impl LspServer {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn add_document(&mut self, uri: &str, text: &str) {
        self.documents.insert(normalize_uri(uri), text.to_string());
    }

    pub fn run(&mut self) -> io::Result<()> {
        eprintln!("[alpha-lsp] Server starting up on stdio...");
        let stdin = io::stdin();
        let mut reader = io::BufReader::new(stdin.lock());
        let mut stdout = io::stdout().lock();
        eprintln!("[alpha-lsp] Ready for LSP messages");

        loop {
            // Read HTTP-like LSP headers
            let mut content_length: Option<usize> = None;
            loop {
                let mut line = String::new();
                let bytes_read = reader.read_line(&mut line)?;
                if bytes_read == 0 {
                    eprintln!("[alpha-lsp] Stdin closed (EOF), exiting");
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
                Err(e) => {
                    eprintln!("[alpha-lsp] Invalid UTF-8 in message body: {}", e);
                    continue;
                }
            };

            let req: Value = match serde_json::from_str(&msg_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[alpha-lsp] JSON parse error: {}", e);
                    continue;
                }
            };

            // Catch any potential panic to keep server permanently stable
            let handle_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.handle_message(&req, &mut stdout)
            }));

            match handle_res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    eprintln!("[alpha-lsp] I/O error during handle_message: {}", e);
                    return Err(e);
                }
                Err(panic_payload) => {
                    eprintln!("[alpha-lsp] Recovered from panic in message handler: {:?}", panic_payload);
                }
            }
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

        if let Some(m) = method {
            eprintln!("[alpha-lsp] Request: method='{}', id={:?}", m, id);
        }

        match method {
            Some("initialize") => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": {
                                "openClose": true,
                                "change": 1
                            },
                            "completionProvider": {
                                "resolveProvider": false,
                                "triggerCharacters": [".", "@", ":", "/", "<"]
                            },
                            "hoverProvider": true,
                            "definitionProvider": true,
                            "documentSymbolProvider": true,
                            "signatureHelpProvider": {
                                "triggerCharacters": ["(", ","]
                            }
                        },
                        "serverInfo": {
                            "name": "alpha-lsp",
                            "version": "1.0.0"
                        }
                    }
                });
                self.send_response(out, resp)?;
                eprintln!("[alpha-lsp] Initialized successfully");
            }

            Some("initialized") => {
                eprintln!("[alpha-lsp] Client confirmed initialized");
            }

            Some("shutdown") => {
                eprintln!("[alpha-lsp] Shutdown requested");
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                });
                self.send_response(out, resp)?;
            }

            Some("exit") => {
                eprintln!("[alpha-lsp] Exiting process");
                std::process::exit(0);
            }

            Some("textDocument/didOpen") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    let text = params["textDocument"]["text"].as_str().unwrap_or_default().to_string();
                    self.documents.insert(normalize_uri(&uri), text);
                    self.publish_diagnostics(out, &uri)?;
                }
            }

            Some("textDocument/didChange") => {
                if let Some(params) = msg.get("params") {
                    let uri = params["textDocument"]["uri"].as_str().unwrap_or_default().to_string();
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            if let Some(text) = last["text"].as_str() {
                                self.documents.insert(normalize_uri(&uri), text.to_string());
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
                    self.documents.remove(&normalize_uri(&uri));
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
                    "result": {
                        "isIncomplete": false,
                        "items": items
                    }
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

            Some("textDocument/definition") => {
                let def = self.get_definition(msg);
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": def
                });
                self.send_response(out, resp)?;
            }

            Some("textDocument/documentSymbol") => {
                let symbols = self.get_document_symbols(msg);
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": symbols
                });
                self.send_response(out, resp)?;
            }

            Some("textDocument/signatureHelp") => {
                let sig = self.get_signature_help(msg);
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": sig
                });
                self.send_response(out, resp)?;
            }

            _ => {
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
        let text = match self.documents.get(&normalize_uri(uri)) {
            Some(t) => t,
            None => return Ok(()),
        };

        let file_path = uri_to_path(uri);
        let errors = validate_alpha_code_with_file(text, file_path.as_deref());
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
                "severity": 1, // 1 = Error
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

    // -------------------------------------------------------------------------
    // Document Symbols (Outline)
    // -------------------------------------------------------------------------
    pub fn get_document_symbols(&self, msg: &Value) -> Value {
        let params = match msg.get("params") {
            Some(p) => p,
            None => return json!([]),
        };
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return json!([]),
        };
        let text = match self.documents.get(&normalize_uri(uri)) {
            Some(t) => t,
            None => return json!([]),
        };

        let program = match parse_program_safely(text) {
            Some(p) => p,
            None => return json!([]),
        };

        let mut symbols = Vec::new();

        // 1. Functions (Kind = 12: Function)
        for func in &program.functions {
            let (range, sel_range) = span_to_lsp_range(&func.span, &func.name);
            symbols.push(json!({
                "name": format!("{}(...)", func.name),
                "detail": format!("{}", func.return_type),
                "kind": 12,
                "range": range,
                "selectionRange": sel_range
            }));
        }

        // 2. Structures (Kind = 23: Struct)
        for st in &program.structures {
            let (range, sel_range) = span_to_lsp_range(&st.span, &st.name);
            let mut children = Vec::new();
            for (f_name, f_ty) in &st.fields {
                let (f_range, f_sel) = span_to_lsp_range(&st.span, f_name);
                children.push(json!({
                    "name": f_name,
                    "detail": format!("{}", f_ty),
                    "kind": 8, // Field
                    "range": f_range,
                    "selectionRange": f_sel
                }));
            }
            for m in &st.methods {
                let (m_range, m_sel) = span_to_lsp_range(&m.span, &m.name);
                children.push(json!({
                    "name": format!("{}(...)", m.name),
                    "detail": format!("{}", m.return_type),
                    "kind": 6, // Method
                    "range": m_range,
                    "selectionRange": m_sel
                }));
            }
            symbols.push(json!({
                "name": st.name,
                "kind": 23,
                "range": range,
                "selectionRange": sel_range,
                "children": children
            }));
        }

        // 3. Enums (Kind = 10: Enum)
        for en in &program.enums {
            let (range, sel_range) = span_to_lsp_range(&en.span, &en.name);
            let mut children = Vec::new();
            for v in &en.variants {
                children.push(json!({
                    "name": v,
                    "kind": 22, // EnumMember
                    "range": range.clone(),
                    "selectionRange": range.clone()
                }));
            }
            symbols.push(json!({
                "name": en.name,
                "kind": 10,
                "range": range,
                "selectionRange": sel_range,
                "children": children
            }));
        }

        // 4. Modules (Kind = 2: Module)
        for m in &program.modules {
            let (range, sel_range) = span_to_lsp_range(&m.span, &m.name);
            let mut children = Vec::new();
            for func in &m.functions {
                let (f_range, f_sel) = span_to_lsp_range(&func.span, &func.name);
                children.push(json!({
                    "name": format!("{}(...)", func.name),
                    "detail": format!("{}", func.return_type),
                    "kind": 12,
                    "range": f_range,
                    "selectionRange": f_sel
                }));
            }
            symbols.push(json!({
                "name": m.name,
                "kind": 2,
                "range": range,
                "selectionRange": sel_range,
                "children": children
            }));
        }

        // 5. Global variables (Kind = 13: Variable)
        for stmt in &program.statements {
            if let Statement::Var(v) = stmt {
                let (range, sel_range) = span_to_lsp_range(&v.span, &v.name);
                symbols.push(json!({
                    "name": v.name,
                    "detail": format!("{}", v.ty),
                    "kind": 13,
                    "range": range,
                    "selectionRange": sel_range
                }));
            }
        }

        json!(symbols)
    }

    // -------------------------------------------------------------------------
    // Go To Definition
    // -------------------------------------------------------------------------
    pub fn get_definition(&self, msg: &Value) -> Value {
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

        let text = match self.documents.get(&normalize_uri(uri)) {
            Some(t) => t,
            None => return json!(null),
        };

        let word = match extract_word_at_pos(text, target_line, target_col) {
            Some(w) if !w.is_empty() => w,
            _ => return json!(null),
        };

        let program = match parse_program_safely(text) {
            Some(p) => p,
            None => return json!(null),
        };

        // Check functions
        for func in &program.functions {
            if func.name == word {
                let (range, _) = span_to_lsp_range(&func.span, &func.name);
                return json!({ "uri": uri, "range": range });
            }
        }

        // Check structures
        for st in &program.structures {
            if st.name == word {
                let (range, _) = span_to_lsp_range(&st.span, &st.name);
                return json!({ "uri": uri, "range": range });
            }
            for m in &st.methods {
                if m.name == word {
                    let (range, _) = span_to_lsp_range(&m.span, &m.name);
                    return json!({ "uri": uri, "range": range });
                }
            }
            for (f_name, _) in &st.fields {
                if f_name == &word {
                    let (range, _) = span_to_lsp_range(&st.span, f_name);
                    return json!({ "uri": uri, "range": range });
                }
            }
        }

        // Check enums
        for en in &program.enums {
            if en.name == word {
                let (range, _) = span_to_lsp_range(&en.span, &en.name);
                return json!({ "uri": uri, "range": range });
            }
            for v in &en.variants {
                if *v == word {
                    let (range, _) = span_to_lsp_range(&en.span, v);
                    return json!({ "uri": uri, "range": range });
                }
            }
        }

        // Check modules
        for m in &program.modules {
            if m.name == word {
                let (range, _) = span_to_lsp_range(&m.span, &m.name);
                return json!({ "uri": uri, "range": range });
            }
            for func in &m.functions {
                if func.name == word {
                    let (range, _) = span_to_lsp_range(&func.span, &func.name);
                    return json!({ "uri": uri, "range": range });
                }
            }
        }

        // Check variables across all scopes (functions, parameters, blocks, top-level)
        for (v_name, _, v_span) in collect_all_variables(&program) {
            if v_name == word {
                let (range, _) = span_to_lsp_range(&v_span, &v_name);
                return json!({ "uri": uri, "range": range });
            }
        }

        json!(null)
    }

    // -------------------------------------------------------------------------
    // Signature Help
    // -------------------------------------------------------------------------
    pub fn get_signature_help(&self, msg: &Value) -> Value {
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

        let text = match self.documents.get(&normalize_uri(uri)) {
            Some(t) => t,
            None => return json!(null),
        };

        let lines: Vec<&str> = text.lines().collect();
        if target_line >= lines.len() {
            return json!(null);
        }
        let line = lines[target_line];
        let before_cursor: String = line.chars().take(target_col).collect();

        // Find last unclosed '('
        let open_paren = match before_cursor.rfind('(') {
            Some(idx) => idx,
            None => return json!(null),
        };

        // Callee name before '('
        let before_paren = before_cursor[..open_paren].trim_end();
        let callee = before_paren.rsplit(|c: char| !c.is_alphanumeric() && c != '_' && c != '?' && c != '.').next().unwrap_or_default();
        if callee.is_empty() {
            return json!(null);
        }

        // Active param by counting commas after '('
        let inside_args = &before_cursor[open_paren + 1..];
        let active_param = inside_args.chars().filter(|&c| c == ',').count();

        // Standard library / builtin signatures
        let stdlib_sigs: HashMap<&str, (&str, &str, Vec<&str>)> = HashMap::from([
            ("mem.arena.allocate", ("mem.arena.allocate([capacity: int]) -> Mem.ArenaSector", "Allocates a new memory arena sector", vec!["capacity: int"])),
            ("Mem.arena.allocate", ("Mem.arena.allocate([capacity: int]) -> Mem.ArenaSector", "Allocates a new memory arena sector", vec!["capacity: int"])),
            ("sector.insert", ("sector.insert(object: everything) -> nothing", "Moves an object or variable into the arena sector", vec!["object: everything"])),
            ("sector.reset", ("sector.reset() -> nothing", "Frees and deletes all objects residing in this arena sector", vec![])),
            ("print", ("print(value: something) -> nothing", "Outputs value to stdout", vec!["value: something"])),
            ("printf", ("printf(fmt: string, ...args) -> nothing", "Outputs formatted string with {} placeholders", vec!["fmt: string", "...args"])),
            ("format", ("format(fmt: string, ...args) -> string", "Formats string with placeholders and padding", vec!["fmt: string", "...args"])),
            ("error", ("error(message: string) -> nothing", "Halts execution with a formatted error", vec!["message: string"])),
            ("paint", ("paint(text: string, color: hexcolor) -> string", "Applies ANSI hex color to string", vec!["text: string", "color: hexcolor"])),
            ("Time.benchmark", ("Time.benchmark({ ... }) -> int", "Measures execution duration in milliseconds", vec!["block: code block"])),
            ("time.benchmark", ("time.benchmark({ ... }) -> int", "Measures execution duration in milliseconds", vec!["block: code block"])),
            ("Time.sleep", ("Time.sleep(ms: int) -> nothing", "Suspends thread execution for ms milliseconds", vec!["ms: int"])),
            ("time.sleep", ("time.sleep(ms: int) -> nothing", "Suspends thread execution for ms milliseconds", vec!["ms: int"])),
            ("Time.elapsed", ("Time.elapsed(start_ms: int) -> int", "Returns milliseconds elapsed since start_ms", vec!["start_ms: int"])),
            ("Io.open", ("Io.open(path: string) -> Io.File", "Opens file handle for reading and writing", vec!["path: string"])),
            ("Io.read", ("Io.read(path: string) -> string", "Reads entire file into a string", vec!["path: string"])),
            ("Io.write", ("Io.write(path: string, content: string) -> nothing", "Writes text content into a file", vec!["path: string", "content: string"])),
            ("Fs.exists?", ("Fs.exists?(path: string) -> boolean", "Checks if file or folder exists", vec!["path: string"])),
            ("Fs.open", ("Fs.open(path: string) -> Fs.Dir", "Opens directory handle", vec!["path: string"])),
            ("Math.abs", ("Math.abs(n: int) -> int", "Absolute value of an integer", vec!["n: int"])),
            ("Math.min", ("Math.min(a: int, b: int) -> int", "Minimum of two integers", vec!["a: int", "b: int"])),
            ("Math.max", ("Math.max(a: int, b: int) -> int", "Maximum of two integers", vec!["a: int", "b: int"])),
            ("Math.pow", ("Math.pow(base: int, exp: int) -> int", "Computes base raised to the power of exp", vec!["base: int", "exp: int"])),
            ("content", ("file.content() -> string", "Reads the entire file content as string", vec![])),
            ("file.getline", ("file.getline(line_number: int) -> string", "Reads a specific line from file by 1-based index", vec!["line_number: int"])),
            ("getline", ("file.getline(line_number: int) -> string", "Reads a specific line from file by 1-based index", vec!["line_number: int"])),
            ("chars", ("str.chars() -> vect<char>", "Returns a vector of characters in the string", vec![])),
            ("split", ("str.split(sep: string) -> vect<string>", "Splits string by separator into a vector of strings", vec!["sep: string"])),
            ("trim", ("str.trim() -> string", "Trims leading and trailing whitespace", vec![])),
            ("trim_start", ("str.trim_start() -> string", "Trims leading whitespace", vec![])),
            ("trim_end", ("str.trim_end() -> string", "Trims trailing whitespace", vec![])),
            ("replace", ("str.replace(from: string, to: string) -> string", "Replaces occurrences of from with to", vec!["from: string", "to: string"])),
            ("contains", ("str.contains(sub: string) -> boolean", "Checks if string contains substring", vec!["sub: string"])),
            ("starts_with?", ("str.starts_with?(prefix: string) -> boolean", "Checks if string starts with prefix", vec!["prefix: string"])),
            ("ends_with?", ("str.ends_with?(suffix: string) -> boolean", "Checks if string ends with suffix", vec!["suffix: string"])),
            ("to_lower", ("str.to_lower() -> string", "Converts string to lowercase", vec![])),
            ("to_upper", ("str.to_upper() -> string", "Converts string to uppercase", vec![])),
            ("to_int", ("str.to_int() -> int", "Parses string into integer", vec![])),
            ("to_float", ("str.to_float() -> float", "Parses string into float", vec![])),
            ("cli.arguments", ("cli.arguments() -> vect<string>", "Returns list of command-line arguments", vec![])),
            ("arguments", ("cli.arguments() -> vect<string>", "Returns list of command-line arguments", vec![])),
            ("cli.paint", ("cli.paint(text: string, color: hexcolor) -> string", "Applies ANSI hex color to string", vec!["text: string", "color: hexcolor"])),
        ]);

        if let Some((label, doc, params)) = stdlib_sigs.get(callee) {
            let param_items: Vec<Value> = params.iter().map(|p| json!({ "label": p })).collect();
            return json!({
                "signatures": [{
                    "label": label,
                    "documentation": { "kind": "markdown", "value": doc },
                    "parameters": param_items
                }],
                "activeSignature": 0,
                "activeParameter": active_param.min(params.len().saturating_sub(1))
            });
        }

        // Check user functions
        if let Some(program) = parse_program_safely(text) {
            for func in &program.functions {
                if func.name == callee {
                    let param_labels: Vec<String> = func.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
                    let sig_label = format!("function {}({}) -> {}", func.name, param_labels.join(", "), func.return_type);
                    let param_items: Vec<Value> = param_labels.iter().map(|p| json!({ "label": p })).collect();
                    return json!({
                        "signatures": [{
                            "label": sig_label,
                            "documentation": { "kind": "markdown", "value": format!("Function `{}` defined in this file", func.name) },
                            "parameters": param_items
                        }],
                        "activeSignature": 0,
                        "activeParameter": active_param.min(param_labels.len().saturating_sub(1))
                    });
                }
            }
        }

        json!(null)
    }

    fn get_imported_programs(&self, current_uri: &str, program: &Program) -> Vec<Program> {
        let mut imported = Vec::new();
        let current_path = uri_to_path(current_uri);
        let parent_dir = current_path.as_ref().and_then(|p| p.parent());

        for imp in &program.imports {
            match &imp.target {
                ImportTarget::Local(path_str) => {
                    let mut text_opt = None;

                    if let Some(parent) = parent_dir {
                        let candidate1 = parent.join(path_str);
                        let uri1 = format!("file:///{}", candidate1.to_string_lossy().replace('\\', "/"));
                        if let Some(doc) = self.documents.get(&normalize_uri(&uri1)) {
                            text_opt = Some(doc.clone());
                        } else if candidate1.is_file() {
                            if let Ok(c) = fs::read_to_string(&candidate1) {
                                text_opt = Some(c);
                            }
                        }

                        if text_opt.is_none() && !path_str.ends_with(".alpha") {
                            let candidate2 = parent.join(format!("{}.alpha", path_str));
                            let uri2 = format!("file:///{}", candidate2.to_string_lossy().replace('\\', "/"));
                            if let Some(doc) = self.documents.get(&normalize_uri(&uri2)) {
                                text_opt = Some(doc.clone());
                            } else if candidate2.is_file() {
                                if let Ok(c) = fs::read_to_string(&candidate2) {
                                    text_opt = Some(c);
                                }
                            }
                        }
                    }

                    if let Some(src) = text_opt {
                        if let Some(p) = parse_program_safely(&src) {
                            imported.push(p);
                        }
                    }
                }
                ImportTarget::Core(core_path) => {
                    let mut text_opt = None;
                    if let Some(parent) = parent_dir {
                        let candidates = [
                            parent.join(core_path),
                            parent.join("src").join(core_path),
                            parent.join("..").join("src").join(core_path),
                            parent.join("..").join("..").join("src").join(core_path),
                        ];
                        for cand in &candidates {
                            if cand.is_file() {
                                if let Ok(c) = fs::read_to_string(cand) {
                                    text_opt = Some(c);
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(src) = text_opt {
                        if let Some(p) = parse_program_safely(&src) {
                            imported.push(p);
                        }
                    }
                }
                _ => {}
            }
        }

        imported
    }

    // -------------------------------------------------------------------------
    // Completions
    // -------------------------------------------------------------------------
    pub fn get_completions(&self, msg: &Value) -> Vec<Value> {
        let params = match msg.get("params") {
            Some(p) => p,
            None => return vec![],
        };
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        let target_line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
        let target_col = params["position"]["character"].as_u64().unwrap_or(0) as usize;

        let mut line_prefix = String::new();
        let mut full_text = String::new();
        if let Some(text) = self.documents.get(&normalize_uri(uri)) {
            full_text = text.clone();
            let lines: Vec<&str> = text.lines().collect();
            if target_line < lines.len() {
                let line = lines[target_line];
                line_prefix = line.chars().take(target_col).collect::<String>().trim_end().to_string();
            }
        }

        // 1. If line starts with "import ": suggest standard Core modules and local .alpha files
        if line_prefix.starts_with("import ") || line_prefix.starts_with("import") {
            let mut import_items = Vec::new();
            let core_modules = [
                "Core/Basic.alpha",
                "Core/Time.alpha",
                "Core/Io.alpha",
                "Core/Fs.alpha",
                "Core/Math.alpha",
                "Core/Env.alpha",
                "Core/Cli.alpha",
                "Core/Machine.alpha",
                "Core/Random.alpha",
                "Core/Http.alpha",
                "Core/Json.alpha",
                "Core/Webview.alpha",
            ];
            for m in core_modules {
                import_items.push(json!({
                    "label": m,
                    "kind": 9, // Module
                    "detail": "Alpha Core Module",
                    "insertText": m
                }));
            }

            // Also scan current directory for local .alpha files
            if let Some(p) = uri_to_path(uri) {
                if let Some(parent) = p.parent() {
                    if let Ok(entries) = fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("alpha") {
                                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                                    if fname != p.file_name().and_then(|s| s.to_str()).unwrap_or_default() {
                                        import_items.push(json!({
                                            "label": format!("./{}", fname),
                                            "kind": 9,
                                            "detail": "Local Alpha Module",
                                            "insertText": format!("./{}", fname)
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            return import_items;
        }

        // 2. Parse current program and imported files dynamically
        let current_program = parse_program_safely(&full_text).or_else(|| {
            let lines: Vec<&str> = full_text.lines().collect();
            if target_line < lines.len() {
                let mut sanitized: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
                sanitized[target_line] = format!("// {}", sanitized[target_line]);
                parse_program_safely(&sanitized.join("\n"))
            } else {
                None
            }
        });
        let imported_programs = if let Some(ref prog) = current_program {
            self.get_imported_programs(uri, prog)
        } else {
            Vec::new()
        };

        // 3. Member Access after '.'
        if let Some(prefix) = extract_prefix_before_dot(&line_prefix) {
            let members = get_member_completions(&prefix, current_program.as_ref(), &imported_programs, &full_text);
            if !members.is_empty() {
                return members;
            }
        }

        // 4. General Scope Completions
        let mut completions = Vec::new();
        let mut seen_labels = HashSet::new();

        let mut push_item = |item: Value| {
            if let Some(label) = item["label"].as_str() {
                if seen_labels.insert(label.to_string()) {
                    completions.push(item);
                }
            }
        };

        // A. Dynamic User Functions & Symbols from Current Document
        if let Some(ref program) = current_program {
            for func in &program.functions {
                push_item(func_to_completion(func, None));
            }
            for m in &program.modules {
                for func in &m.functions {
                    push_item(func_to_completion(func, Some(&m.name)));
                    push_item(func_to_completion(func, None));
                }
                push_item(json!({
                    "label": m.name,
                    "kind": 9, // Module
                    "detail": format!("module {}", m.name)
                }));
            }
            for st in &program.structures {
                for m in &st.methods {
                    push_item(method_to_completion(m, &st.name));
                }
                push_item(json!({
                    "label": st.name,
                    "kind": 22, // Struct
                    "detail": format!("structure {}", st.name)
                }));
            }
            for en in &program.enums {
                push_item(json!({
                    "label": en.name,
                    "kind": 10, // Enum
                    "detail": format!("enum {}", en.name)
                }));
                for v in &en.variants {
                    push_item(json!({
                        "label": format!("{}.{}", en.name, v),
                        "kind": 20, // EnumMember
                        "detail": format!("{}.{}", en.name, v)
                    }));
                }
            }
            for (v_name, v_ty, _) in collect_all_variables(program) {
                push_item(json!({
                    "label": v_name,
                    "kind": 6, // Variable
                    "detail": format!("var {}: {}", v_name, v_ty)
                }));
            }
        }

        // B. Dynamic Functions & Symbols from Imported Files
        for imported_prog in &imported_programs {
            for func in &imported_prog.functions {
                push_item(func_to_completion(func, None));
            }
            for m in &imported_prog.modules {
                for func in &m.functions {
                    push_item(func_to_completion(func, Some(&m.name)));
                    push_item(func_to_completion(func, None));
                }
                push_item(json!({
                    "label": m.name,
                    "kind": 9,
                    "detail": format!("module {}", m.name)
                }));
            }
            for st in &imported_prog.structures {
                for m in &st.methods {
                    push_item(method_to_completion(m, &st.name));
                }
                push_item(json!({
                    "label": st.name,
                    "kind": 22,
                    "detail": format!("structure {}", st.name)
                }));
            }
            for en in &imported_prog.enums {
                push_item(json!({
                    "label": en.name,
                    "kind": 10,
                    "detail": format!("enum {}", en.name)
                }));
            }
            for (v_name, v_ty, _) in collect_all_variables(imported_prog) {
                push_item(json!({
                    "label": v_name,
                    "kind": 6,
                    "detail": format!("var {}: {}", v_name, v_ty)
                }));
            }
        }

        // C. Standard Core Library Functions
        for (name, detail) in &[
            ("print", "function print(value: something) -> nothing"),
            ("printf", "function printf(fmt: string, ...args) -> nothing"),
            ("format", "function format(fmt: string, ...args) -> string"),
            ("error", "function error(message: string) -> nothing"),
            ("paint", "function paint(text: string, color: hexcolor) -> string"),
            ("Time.now", "function Time.now() -> int"),
            ("Time.sleep", "function Time.sleep(ms: int) -> nothing"),
            ("Time.benchmark", "function Time.benchmark(block) -> int"),
            ("Io.open", "function Io.open(path: string) -> Io.File"),
            ("Io.read", "function Io.read(path: string) -> string"),
            ("Io.write", "function Io.write(path: string, content: string) -> nothing"),
            ("Fs.open", "function Fs.open(path: string) -> Fs.Dir"),
            ("Fs.exists?", "function Fs.exists?(path: string) -> boolean"),
            ("cli.arguments", "function cli.arguments() -> vect<string>"),
            ("cli.paint", "function cli.paint(text: string, color: hexcolor) -> string"),
            ("Math.abs", "function Math.abs(n: int) -> int"),
            ("Math.min", "function Math.min(a: int, b: int) -> int"),
            ("Math.max", "function Math.max(a: int, b: int) -> int"),
            ("Mem.arena.allocate", "function Mem.arena.allocate([capacity: int]) -> Mem.ArenaSector"),
        ] {
            push_item(json!({
                "label": *name,
                "kind": 3, // Function
                "detail": *detail
            }));
        }

        // D. Core Namespaces
        for m in &["Time", "Io", "Fs", "Math", "Env", "Cli", "Machine", "Random", "Http", "Json", "Webview"] {
            push_item(json!({
                "label": *m,
                "kind": 9, // Module
                "detail": "module"
            }));
        }

        // E. Language Keywords & Snippets
        let snippets = [
            ("function", "function ${1:name}(${2:params}) [${3:public}, ${4:return_type}] {\n\t$0\n}"),
            ("var", "var ${1:name}: ${2:type} = ${3:value}"),
            ("const var", "const var ${1:NAME}: ${2:type} = ${3:value}"),
            ("if", "if ${1:condition} {\n\t$0\n}"),
            ("elseif", "elseif ${1:condition} {\n\t$0\n}"),
            ("else", "else {\n\t$0\n}"),
            ("for in", "for each ${1:item} in ${2:collection} {\n\t$0\n}"),
            ("for range", "for ${1:1}...${2:10} {\n\t$0\n}"),
            ("make schedule", "make schedule {\n\t$0\n}"),
            ("check", "check ${1:val} {\n\t${2:pattern} => ${3:expr},\n}"),
            ("make", "make ${1:StructName}{${2:field}: ${3:val}}"),
            ("structure", "structure ${1:Name} {\n\t${2:field}: ${3:type},\nattach {\n\t$0\n}\n}"),
            ("enum", "enum ${1:Name} {\n\t${2:Variant1},\n\t${3:Variant2},\n}"),
            ("module", "module ${1:Name} {\n\t$0\n}"),
            ("try catch", "try {\n\t$1\n} catch ${2:err} {\n\t$0\n}"),
            ("verify", "verify ${1:condition}, \"${2:error message}\""),
            ("@strict", "@strict\n"),
            ("#typechecked", "#typechecked "),
            ("#test", "#test ${1:test_name}() {\n\t$0\n}"),
        ];
        for (lbl, snip) in snippets {
            push_item(json!({
                "label": lbl,
                "kind": 15, // Snippet
                "detail": "keyword / snippet",
                "insertText": snip,
                "insertTextFormat": 2
            }));
        }

        // F. Primitive Types
        let types = [
            "int", "unt", "float", "string", "char", "boolean",
            "hexcolor", "vect", "map", "something", "everything", "nothing",
        ];
        for ty in types {
            push_item(json!({
                "label": ty,
                "kind": 7, // Type
                "detail": "type"
            }));
        }

        completions
    }

    // -------------------------------------------------------------------------
    // Hover Documentation
    // -------------------------------------------------------------------------
    pub fn get_hover(&self, msg: &Value) -> Value {
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

        let text = match self.documents.get(&normalize_uri(uri)) {
            Some(t) => t,
            None => return json!(null),
        };

        let word = match extract_word_at_pos(text, target_line, target_col) {
            Some(w) if !w.is_empty() => w,
            _ => return json!(null),
        };

        // 1. Check user-defined items in this file
        if let Some(program) = parse_program_safely(text) {
            for func in &program.functions {
                if func.name == word {
                    let params_str: Vec<String> = func.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
                    let hover_doc = format!(
                        "```alpha\nfunction {}({}) [{:?}, {}]\n```\n\n*User function defined in this file*",
                        func.name,
                        params_str.join(", "),
                        func.visibility,
                        func.return_type
                    );
                    return json!({ "contents": { "kind": "markdown", "value": hover_doc } });
                }
            }
            for st in &program.structures {
                if st.name == word {
                    let fields_str: Vec<String> = st.fields.iter().map(|(f_name, f_ty)| format!("    {}: {}", f_name, f_ty)).collect();
                    let hover_doc = format!("```alpha\nstructure {} {{\n{}\n}}\n```", st.name, fields_str.join(",\n"));
                    return json!({ "contents": { "kind": "markdown", "value": hover_doc } });
                }
            }
            for en in &program.enums {
                if en.name == word {
                    let hover_doc = format!("```alpha\nenum {} {{\n    {}\n}}\n```", en.name, en.variants.join(", "));
                    return json!({ "contents": { "kind": "markdown", "value": hover_doc } });
                }
            }
            for stmt in &program.statements {
                if let Statement::Var(v) = stmt {
                    if v.name == word {
                        let kind_str = if v.is_const { "const var" } else { "var" };
                        let hover_doc = format!("```alpha\n{} {}: {}\n```", kind_str, v.name, v.ty);
                        return json!({ "contents": { "kind": "markdown", "value": hover_doc } });
                    }
                }
            }
        }

        // 2. Builtin Language Keywords, Directives, and Core Modules
        let hover_text = match word.as_str() {
            "unt" => "64-bit unsigned integer (`unt`)",
            "int" => "64-bit signed integer (`int`)",
            "float" => "64-bit float (`float`)",
            "string" => "string",
            "char" => "char",
            "boolean" => "boolean",
            "hexcolor" => "hexcolor (`#(RRGGBB)`)",
            "@strict" => "@strict",
            "#typechecked" | "typechecked" => "directive #typechecked (marks function as pre-checked, enforces strict mode rules, accelerates compilation)",
            "#test" => "directive #test (marks function as an automated test for 'alpha test')",
            "make schedule" | "schedule" => "make schedule { ... }",
            "check" => "check pattern matching",
            "Time" | "time" => "module Time",
            "Io" | "io" => "module Io",
            "Fs" | "fs" => "module Fs",
            "Math" | "math" => "module Math",
            "Env" | "env" => "module Env",
            "Cli" | "cli" => "module Cli",
            "Machine" | "console" => "module Machine",
            "Random" | "random" => "module Random",
            "Http" | "http" => "module Http",
            "Json" | "json" => "module Json",
            "Webview" | "webview" => "module Webview",
            "Mem" | "mem" => "module Mem (ArenaAllocator)",
            "Mem.ArenaSector" | "ArenaSector" => "type Mem.ArenaSector",
            "everything" => "type everything (accepts any value except nothing; allowed in strict mode)",
            "print" => "function print(value: something) -> nothing",
            "printf" => "function printf(fmt: string, ...args) -> nothing",
            "format" => "function format(fmt: string, ...args) -> string",
            "error" => "function error(message: string) -> nothing",
            "chars" => "method chars() -> vect<char>\n\nReturns a vector of individual characters in the string.",
            "split" => "method split(sep: string) -> vect<string>\n\nSplits string by separator into a vector of substrings.",
            "trim" => "method trim() -> string\n\nTrims leading and trailing whitespace characters.",
            "trim_start" => "method trim_start() -> string\n\nTrims leading whitespace characters.",
            "trim_end" => "method trim_end() -> string\n\nTrims trailing whitespace characters.",
            "replace" => "method replace(from: string, to: string) -> string\n\nReplaces all occurrences of from with to.",
            "starts_with?" | "starts_with" => "method starts_with?(prefix: string) -> boolean\n\nReturns true if string starts with prefix.",
            "ends_with?" | "ends_with" => "method ends_with?(suffix: string) -> boolean\n\nReturns true if string ends with suffix.",
            "to_lower" => "method to_lower() -> string\n\nConverts string to lowercase.",
            "to_upper" => "method to_upper() -> string\n\nConverts string to uppercase.",
            "to_int" => "method to_int() -> int\n\nParses string into a signed 64-bit integer.",
            "to_float" => "method to_float() -> float\n\nParses string into a 64-bit float.",
            "content" => "method content() -> string\n\nReads the entire file content into a string.",
            "getline" => "method getline(line_number: int) -> string\n\nReads a single line from the file using 1-based indexing (returns nothing if out of bounds).",
            "arguments" => "function cli.arguments() -> vect<string>\n\nReturns command-line arguments passed to the program.",
            "push" => "method push(item) -> nothing\n\nAppends an element to the vector.",
            "pop" => "method pop() -> element\n\nRemoves and returns the last element of the vector.",
            "insert" => "method insert(...) -> nothing\n\nInserts into map or arena sector.",
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

// -----------------------------------------------------------------------------
// Helper functions
// -----------------------------------------------------------------------------

pub fn validate_alpha_code(source: &str) -> Vec<AlphaError> {
    validate_alpha_code_with_file(source, None)
}

pub fn validate_alpha_code_with_file(source: &str, file_name: Option<&Path>) -> Vec<AlphaError> {
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
    if let Some(f) = file_name {
        let work_dir = if let Some(parent) = f.parent() {
            crate::driver::project::find_project_root(parent).unwrap_or_else(|| {
                if parent.file_name().and_then(|n| n.to_str()) == Some("src") {
                    parent.parent().unwrap_or(parent).to_path_buf()
                } else {
                    parent.to_path_buf()
                }
            })
        } else {
            std::path::PathBuf::from(".")
        };
        typechecker = typechecker.with_working_dir(work_dir);
        typechecker = typechecker.with_file(f.to_string_lossy().to_string());
    }

    if let Err(err) = typechecker.check_program(&program) {
        return vec![err];
    }

    Vec::new()
}

fn parse_program_safely(source: &str) -> Option<Program> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().ok()?;
    let mut parser = Parser::new(tokens);
    parser.parse_program().ok()
}

fn extract_word_at_pos(text: &str, line_idx: usize, char_col: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_idx >= lines.len() {
        return None;
    }
    let chars: Vec<char> = lines[line_idx].chars().collect();
    if chars.is_empty() {
        return None;
    }

    let col = char_col.min(chars.len().saturating_sub(1));
    let mut start = col;
    while start > 0 && is_alpha_ident_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < chars.len() && is_alpha_ident_char(chars[end]) {
        end += 1;
    }

    if start >= end {
        None
    } else {
        Some(chars[start..end].iter().collect())
    }
}

fn is_alpha_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '?' || c == '@' || c == '.'
}

fn span_to_lsp_range(span: &Span, name: &str) -> (Value, Value) {
    let line = if span.line > 0 { span.line - 1 } else { 0 };
    let col = if span.col > 0 { span.col - 1 } else { 0 };
    let len = (span.end.saturating_sub(span.start)).max(1);

    let full_range = json!({
        "start": { "line": line, "character": col },
        "end": { "line": line, "character": col + len }
    });

    let sel_range = json!({
        "start": { "line": line, "character": col },
        "end": { "line": line, "character": col + name.len() }
    });

    (full_range, sel_range)
}

fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    if let Some(stripped) = uri.strip_prefix("file:///") {
        #[cfg(windows)]
        {
            Some(std::path::PathBuf::from(stripped.replace('/', "\\")))
        }
        #[cfg(not(windows))]
        {
            Some(std::path::PathBuf::from(format!("/{}", stripped)))
        }
    } else if let Some(stripped) = uri.strip_prefix("file://") {
        Some(std::path::PathBuf::from(stripped))
    } else {
        None
    }
}

fn extract_prefix_before_dot(line_prefix: &str) -> Option<String> {
    if !line_prefix.ends_with('.') {
        return None;
    }
    let trimmed = line_prefix[..line_prefix.len() - 1].trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut i = chars.len();
    let mut paren_depth = 0;
    let mut bracket_depth = 0;

    while i > 0 {
        let c = chars[i - 1];
        if c == ')' {
            paren_depth += 1;
            i -= 1;
        } else if c == '(' {
            if paren_depth > 0 {
                paren_depth -= 1;
                i -= 1;
            } else {
                break;
            }
        } else if c == ']' {
            bracket_depth += 1;
            i -= 1;
        } else if c == '[' {
            if bracket_depth > 0 {
                bracket_depth -= 1;
                i -= 1;
            } else {
                break;
            }
        } else if paren_depth > 0 || bracket_depth > 0 {
            i -= 1;
        } else if c.is_alphanumeric() || c == '_' || c == '?' || c == '@' || c == '.' {
            i -= 1;
        } else {
            break;
        }
    }

    if i == chars.len() {
        None
    } else {
        Some(chars[i..].iter().collect())
    }
}

fn func_to_completion(func: &FunctionDecl, prefix: Option<&str>) -> Value {
    let name = match prefix {
        Some(p) => format!("{}.{}", p, func.name),
        None => func.name.clone(),
    };
    let params_sig: Vec<String> = func.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
    let detail = format!("{}({}) -> {}", name, params_sig.join(", "), func.return_type);
    let insert_text = if func.params.is_empty() {
        format!("{}()", name)
    } else {
        let snippet_args: Vec<String> = func.params.iter().enumerate().map(|(i, p)| {
            format!("${{{}:{}}}", i + 1, p.name)
        }).collect();
        format!("{}({})", name, snippet_args.join(", "))
    };

    json!({
        "label": name,
        "kind": 3, // Function
        "detail": detail,
        "insertText": insert_text,
        "insertTextFormat": 2
    })
}

fn method_to_completion(method: &AttachedMethod, struct_name: &str) -> Value {
    let params_sig: Vec<String> = method.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
    let detail = format!("{}.{}({}) -> {}", struct_name, method.name, params_sig.join(", "), method.return_type);
    let insert_text = if method.params.is_empty() {
        format!("{}()", method.name)
    } else {
        let snippet_args: Vec<String> = method.params.iter().enumerate().map(|(i, p)| {
            format!("${{{}:{}}}", i + 1, p.name)
        }).collect();
        format!("{}({})", method.name, snippet_args.join(", "))
    };

    json!({
        "label": method.name,
        "kind": 2, // Method
        "detail": detail,
        "insertText": insert_text,
        "insertTextFormat": 2
    })
}

pub fn collect_variables_from_statement(stmt: &Statement, vars: &mut Vec<(String, Type, Span)>) {
    match stmt {
        Statement::Var(v) => {
            vars.push((v.name.clone(), v.ty.clone(), v.span));
        }
        Statement::VarTuple { names, types, span, .. } => {
            for (i, name) in names.iter().enumerate() {
                let ty = types.get(i).cloned().flatten().unwrap_or(Type::Something);
                vars.push((name.clone(), ty, *span));
            }
        }
        Statement::If { then_branch, elseif_branches, else_branch, .. } => {
            for s in &then_branch.statements {
                collect_variables_from_statement(s, vars);
            }
            for (_, elif_block) in elseif_branches {
                for s in &elif_block.statements {
                    collect_variables_from_statement(s, vars);
                }
            }
            if let Some(eb) = else_branch {
                for s in &eb.statements {
                    collect_variables_from_statement(s, vars);
                }
            }
        }
        Statement::Check { arms, .. } => {
            for arm in arms {
                if let Some(b) = &arm.binding {
                    vars.push((b.clone(), Type::Something, arm.span));
                }
                if let crate::frontend::ast::CheckBody::Block(b) = &arm.body {
                    for s in &b.statements {
                        collect_variables_from_statement(s, vars);
                    }
                }
            }
        }
        Statement::ForInfinite { body, .. } => {
            for s in &body.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::ForCount { init, body, .. } => {
            if let Some(i) = init {
                collect_variables_from_statement(i, vars);
            }
            for s in &body.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::ForRange { var_name, body, span, .. } => {
            if let Some(name) = var_name {
                vars.push((name.clone(), Type::Int, *span));
            }
            for s in &body.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::ForEach { item_var, key_var, collection, body, span } => {
            let item_ty = match collection {
                Expr::MethodCall { method, .. } if method == "chars" => Type::Char,
                Expr::Literal(LiteralValue::String(_), _) => Type::Char,
                _ => Type::Something,
            };
            vars.push((item_var.clone(), item_ty, *span));
            if let Some(k) = key_var {
                vars.push((k.clone(), Type::Int, *span));
            }
            for s in &body.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::TryCatch { err_var, try_block, catch_block, span } => {
            vars.push((err_var.clone(), Type::String, *span));
            for s in &try_block.statements {
                collect_variables_from_statement(s, vars);
            }
            for s in &catch_block.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::Block(b) => {
            for s in &b.statements {
                collect_variables_from_statement(s, vars);
            }
        }
        Statement::Schedule { statement, .. } => {
            collect_variables_from_statement(statement, vars);
        }
        _ => {}
    }
}

pub fn collect_all_variables(program: &Program) -> Vec<(String, Type, Span)> {
    let mut vars = Vec::new();
    for stmt in &program.statements {
        collect_variables_from_statement(stmt, &mut vars);
    }
    for func in &program.functions {
        for p in &func.params {
            vars.push((p.name.clone(), p.ty.clone(), p.span));
        }
        for stmt in &func.body.statements {
            collect_variables_from_statement(stmt, &mut vars);
        }
    }
    for st in &program.structures {
        for m in &st.methods {
            for p in &m.params {
                vars.push((p.name.clone(), p.ty.clone(), p.span));
            }
            for stmt in &m.body.statements {
                collect_variables_from_statement(stmt, &mut vars);
            }
        }
    }
    for m in &program.modules {
        for v in &m.variables {
            vars.push((v.name.clone(), v.ty.clone(), v.span));
        }
        for func in &m.functions {
            for p in &func.params {
                vars.push((p.name.clone(), p.ty.clone(), p.span));
            }
            for stmt in &func.body.statements {
                collect_variables_from_statement(stmt, &mut vars);
            }
        }
    }
    vars
}


fn scan_variables_from_text(text: &str) -> Vec<(String, Type)> {
    let mut vars = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let stripped = if let Some(s) = trimmed.strip_prefix("const var ") {
            s
        } else if let Some(s) = trimmed.strip_prefix("var ") {
            s
        } else {
            continue;
        };
        if let Some(colon_idx) = stripped.find(':') {
            let name = stripped[..colon_idx].trim();
            let rest = &stripped[colon_idx + 1..];
            let type_str = if let Some(eq_idx) = rest.find('=') {
                rest[..eq_idx].trim()
            } else {
                rest.trim()
            };
            let ty = match type_str {
                "string" | "String" => Type::String,
                "int" => Type::Int,
                "unt" => Type::Unt,
                "float" => Type::Float,
                "char" => Type::Char,
                "boolean" | "bool" => Type::Boolean,
                "Io.File" | "File" => Type::Custom("Io.File".to_string()),
                "Fs.Dir" | "Dir" => Type::Custom("Fs.Dir".to_string()),
                s if s.starts_with("vect<") || s == "vect" => Type::Vect(Box::new(Type::Something)),
                s if s.starts_with("map<") || s == "map" => Type::Map(Box::new(Type::Something), Box::new(Type::Something)),
                other => Type::Custom(other.to_string()),
            };
            if !name.is_empty() {
                vars.push((name.to_string(), ty));
            }
        } else if let Some(eq_idx) = stripped.find('=') {
            let name = stripped[..eq_idx].trim();
            let rhs = stripped[eq_idx + 1..].trim();
            let ty = if rhs.starts_with('"') {
                Type::String
            } else if rhs.starts_with('\'') {
                Type::Char
            } else if rhs == "true" || rhs == "false" {
                Type::Boolean
            } else if rhs.starts_with("Io.open") {
                Type::Custom("Io.File".to_string())
            } else if rhs.starts_with("Fs.open") {
                Type::Custom("Fs.Dir".to_string())
            } else if rhs.contains(".content()") {
                Type::String
            } else if rhs.contains(".chars()") {
                Type::Vect(Box::new(Type::Char))
            } else if rhs.starts_with("cli.arguments") || rhs.starts_with("Cli.arguments") {
                Type::Vect(Box::new(Type::String))
            } else {
                Type::Something
            };
            if !name.is_empty() {
                vars.push((name.to_string(), ty));
            }
        }
    }
    vars
}

fn resolve_method_return_type(base_ty: &Type, method: &str, all_programs: &[&Program]) -> Option<Type> {
    match base_ty {
        Type::String => match method {
            "chars" => Some(Type::Vect(Box::new(Type::Char))),
            "split" => Some(Type::Vect(Box::new(Type::String))),
            "len" | "to_int" => Some(Type::Int),
            "to_float" => Some(Type::Float),
            "empty?" | "contains" | "starts_with?" | "starts_with" | "ends_with?" | "ends_with" | "nothing?" => Some(Type::Boolean),
            "trim" | "trim_start" | "trim_end" | "replace" | "to_lower" | "to_upper" => Some(Type::String),
            _ => None,
        },
        Type::Vect(inner) => match method {
            "push" => Some(Type::Nothing),
            "len" => Some(Type::Int),
            "get" | "pop" => Some(*inner.clone()),
            "empty?" | "nothing?" => Some(Type::Boolean),
            _ => None,
        },
        Type::Map(_, v) => match method {
            "insert" => Some(Type::Nothing),
            "get" => Some(*v.clone()),
            "contains" | "empty?" | "nothing?" => Some(Type::Boolean),
            "len" => Some(Type::Int),
            _ => None,
        },
        Type::Custom(custom_name) => {
            let norm = custom_name.to_lowercase();
            if norm == "io.file" || norm == "file" {
                match method {
                    "content" | "read" | "getline" => Some(Type::String),
                    "write" | "reset" | "delete" | "close" => Some(Type::Nothing),
                    "exists?" | "exists" | "empty?" | "something?" | "nothing?" => Some(Type::Boolean),
                    "size" | "bytelen" | "linelen" | "symlen" => Some(Type::Int),
                    _ => None,
                }
            } else if norm == "fs.dir" || norm == "dir" {
                match method {
                    "open" => Some(Type::Custom("Fs.Dir".to_string())),
                    "create" | "delete" | "rename" | "reset" => Some(Type::Nothing),
                    "get_children" | "files" | "dirs" | "walk" => Some(Type::Vect(Box::new(Type::String))),
                    "join" => Some(Type::String),
                    "exists?" | "exists" | "empty?" | "nothing?" => Some(Type::Boolean),
                    _ => None,
                }
            } else if norm == "io" {
                match method {
                    "open" => Some(Type::Custom("Io.File".to_string())),
                    "read" => Some(Type::String),
                    "write" | "copy" | "move" | "delete" => Some(Type::Nothing),
                    "size" => Some(Type::Int),
                    "exists?" | "file?" => Some(Type::Boolean),
                    _ => None,
                }
            } else if norm == "fs" {
                match method {
                    "open" => Some(Type::Custom("Fs.Dir".to_string())),
                    "exists?" | "file?" | "dir?" => Some(Type::Boolean),
                    "join" | "extension" | "filename" | "stem" | "parent" | "abs" => Some(Type::String),
                    _ => None,
                }
            } else if norm == "cli" {
                match method {
                    "arguments" => Some(Type::Vect(Box::new(Type::String))),
                    "paint" => Some(Type::String),
                    _ => None,
                }
            } else if norm == "time" {
                match method {
                    "now" | "now_ms" | "elapsed" | "benchmark" => Some(Type::Int),
                    "sleep" => Some(Type::Nothing),
                    _ => None,
                }
            } else if norm == "math" {
                match method {
                    "abs" | "min" | "max" | "clamp" | "pow" | "sqr" => Some(Type::Int),
                    _ => None,
                }
            } else if norm == "mem" || norm == "mem.arena" {
                match method {
                    "allocate" => Some(Type::Custom("Mem.ArenaSector".to_string())),
                    _ => None,
                }
            } else if norm == "mem.arenasector" || norm == "sector" {
                match method {
                    "insert" | "reset" => Some(Type::Nothing),
                    "count" | "allocated" => Some(Type::Int),
                    "nothing?" => Some(Type::Boolean),
                    _ => None,
                }
            } else {
                for p in all_programs {
                    for st in &p.structures {
                        if st.name == *custom_name {
                            for m in &st.methods {
                                if m.name == method {
                                    return Some(m.return_type.clone());
                                }
                            }
                        }
                    }
                }
                None
            }
        }
        _ => None,
    }
}

fn resolve_type_of_prefix(prefix: &str, all_programs: &[&Program], full_text: &str) -> Option<Type> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "io" => return Some(Type::Custom("Io".to_string())),
        "fs" => return Some(Type::Custom("Fs".to_string())),
        "cli" => return Some(Type::Custom("Cli".to_string())),
        "time" => return Some(Type::Custom("Time".to_string())),
        "math" => return Some(Type::Custom("Math".to_string())),
        "mem" => return Some(Type::Custom("Mem".to_string())),
        "mem.arena" => return Some(Type::Custom("Mem.arena".to_string())),
        "arenasector" | "mem.arenasector" | "sector" => return Some(Type::Custom("Mem.ArenaSector".to_string())),
        "env" => return Some(Type::Custom("Env".to_string())),
        "machine" | "console" => return Some(Type::Custom("Machine".to_string())),
        "random" => return Some(Type::Custom("Random".to_string())),
        "http" => return Some(Type::Custom("Http".to_string())),
        "json" => return Some(Type::Custom("Json".to_string())),
        "webview" => return Some(Type::Custom("Webview".to_string())),
        "file" => return Some(Type::Custom("Io.File".to_string())),
        "dir" => return Some(Type::Custom("Fs.Dir".to_string())),
        _ => {}
    }

    if trimmed.ends_with(']') {
        if let Some(open_bracket) = trimmed.rfind('[') {
            let base = &trimmed[..open_bracket];
            if let Some(base_ty) = resolve_type_of_prefix(base, all_programs, full_text) {
                match base_ty {
                    Type::Vect(inner) => return Some(*inner),
                    Type::Map(_, v) => return Some(*v),
                    Type::String => return Some(Type::Char),
                    _ => {}
                }
            }
        }
    }

    if trimmed.ends_with(')') {
        if let Some(open_paren) = trimmed.rfind('(') {
            let before_paren = trimmed[..open_paren].trim_end();
            if let Some(dot_idx) = before_paren.rfind('.') {
                let base = &before_paren[..dot_idx];
                let method = &before_paren[dot_idx + 1..];
                if let Some(base_ty) = resolve_type_of_prefix(base, all_programs, full_text) {
                    return resolve_method_return_type(&base_ty, method, all_programs);
                }
            }
        }
    } else if let Some(dot_idx) = trimmed.rfind('.') {
        let base = &trimmed[..dot_idx];
        let field = &trimmed[dot_idx + 1..];
        if let Some(base_ty) = resolve_type_of_prefix(base, all_programs, full_text) {
            if let Some(ret) = resolve_method_return_type(&base_ty, field, all_programs) {
                return Some(ret);
            }
            if let Type::Custom(ref st_name) = base_ty {
                for p in all_programs {
                    for st in &p.structures {
                        if st.name == *st_name {
                            for (f_name, f_ty) in &st.fields {
                                if f_name == field {
                                    return Some(f_ty.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for p in all_programs {
        let vars = collect_all_variables(p);
        for (name, ty, _) in vars {
            if name == trimmed {
                return Some(ty);
            }
        }
    }

    for p in all_programs {
        for st in &p.structures {
            if st.name == trimmed {
                return Some(Type::Custom(st.name.clone()));
            }
        }
        for m in &p.modules {
            if m.name == trimmed {
                return Some(Type::Custom(m.name.clone()));
            }
        }
    }

    for (name, ty) in scan_variables_from_text(full_text) {
        if name == trimmed {
            return Some(ty);
        }
    }

    None
}

fn string_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "chars()",
            "kind": 2,
            "detail": "chars(): vect<char> - Returns a vector of characters",
            "insertText": "chars()"
        }),
        json!({
            "label": "len()",
            "kind": 2,
            "detail": "len(): int - Returns length of string",
            "insertText": "len()"
        }),
        json!({
            "label": "empty?()",
            "kind": 2,
            "detail": "empty?(): bool - Checks if string is empty",
            "insertText": "empty?()"
        }),
        json!({
            "label": "split(sep)",
            "kind": 2,
            "detail": "split(sep: string): vect<string> - Splits string by separator",
            "insertText": "split(\"${1:sep}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "trim()",
            "kind": 2,
            "detail": "trim(): string - Trims whitespace from both ends",
            "insertText": "trim()"
        }),
        json!({
            "label": "trim_start()",
            "kind": 2,
            "detail": "trim_start(): string - Trims leading whitespace",
            "insertText": "trim_start()"
        }),
        json!({
            "label": "trim_end()",
            "kind": 2,
            "detail": "trim_end(): string - Trims trailing whitespace",
            "insertText": "trim_end()"
        }),
        json!({
            "label": "contains(sub)",
            "kind": 2,
            "detail": "contains(sub: string): bool - Checks if string contains substring",
            "insertText": "contains(\"${1:sub}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "replace(from, to)",
            "kind": 2,
            "detail": "replace(from: string, to: string): string - Replaces occurrences of from with to",
            "insertText": "replace(\"${1:from}\", \"${2:to}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "starts_with?(prefix)",
            "kind": 2,
            "detail": "starts_with?(prefix: string): bool - Checks if string starts with prefix",
            "insertText": "starts_with?(\"${1:prefix}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "ends_with?(suffix)",
            "kind": 2,
            "detail": "ends_with?(suffix: string): bool - Checks if string ends with suffix",
            "insertText": "ends_with?(\"${1:suffix}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "to_lower()",
            "kind": 2,
            "detail": "to_lower(): string - Converts string to lowercase",
            "insertText": "to_lower()"
        }),
        json!({
            "label": "to_upper()",
            "kind": 2,
            "detail": "to_upper(): string - Converts string to uppercase",
            "insertText": "to_upper()"
        }),
        json!({
            "label": "to_int()",
            "kind": 2,
            "detail": "to_int(): int - Parses string as signed integer",
            "insertText": "to_int()"
        }),
        json!({
            "label": "to_float()",
            "kind": 2,
            "detail": "to_float(): float - Parses string as float",
            "insertText": "to_float()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?(): bool - Checks if string is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn file_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "content()",
            "kind": 2,
            "detail": "content(): string - Reads the entire file content as string",
            "insertText": "content()"
        }),
        json!({
            "label": "getline(line)",
            "kind": 2,
            "detail": "getline(line_number: int): string - Reads a single line by 1-based index",
            "insertText": "getline(${1:1})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "write(text)",
            "kind": 2,
            "detail": "write(text: string): nothing - Appends text to the file",
            "insertText": "write(${1:text})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "reset()",
            "kind": 2,
            "detail": "reset(): nothing - Clears file content",
            "insertText": "reset()"
        }),
        json!({
            "label": "delete()",
            "kind": 2,
            "detail": "delete(): nothing - Deletes the file",
            "insertText": "delete()"
        }),
        json!({
            "label": "exists?()",
            "kind": 2,
            "detail": "exists?(): bool - Checks whether the file exists",
            "insertText": "exists?()"
        }),
        json!({
            "label": "empty?()",
            "kind": 2,
            "detail": "empty?(): bool - Checks whether the file is empty",
            "insertText": "empty?()"
        }),
        json!({
            "label": "something?()",
            "kind": 2,
            "detail": "something?(): bool - Checks whether file has content",
            "insertText": "something?()"
        }),
        json!({
            "label": "size()",
            "kind": 2,
            "detail": "size(): int - Returns file size in bytes",
            "insertText": "size()"
        }),
        json!({
            "label": "bytelen()",
            "kind": 2,
            "detail": "bytelen(): int - Returns file size in bytes",
            "insertText": "bytelen()"
        }),
        json!({
            "label": "linelen()",
            "kind": 2,
            "detail": "linelen(): int - Returns line count of file",
            "insertText": "linelen()"
        }),
        json!({
            "label": "symlen()",
            "kind": 2,
            "detail": "symlen(): int - Returns character count of file",
            "insertText": "symlen()"
        }),
        json!({
            "label": "read()",
            "kind": 2,
            "detail": "read(): string - Reads entire file content",
            "insertText": "read()"
        }),
        json!({
            "label": "close()",
            "kind": 2,
            "detail": "close(): nothing - Closes file handle",
            "insertText": "close()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?(): bool - Checks if file handle is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn dir_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "open(sub)",
            "kind": 2,
            "detail": "open(sub: string): Fs.Dir - Opens subdirectory handle",
            "insertText": "open(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "create()",
            "kind": 2,
            "detail": "create(): nothing - Creates directory if not exists",
            "insertText": "create()"
        }),
        json!({
            "label": "delete()",
            "kind": 2,
            "detail": "delete(): nothing - Deletes directory",
            "insertText": "delete()"
        }),
        json!({
            "label": "rename(name)",
            "kind": 2,
            "detail": "rename(name: string): nothing - Renames directory",
            "insertText": "rename(\"${1:name}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "reset()",
            "kind": 2,
            "detail": "reset(): nothing - Clears directory contents",
            "insertText": "reset()"
        }),
        json!({
            "label": "get_children()",
            "kind": 2,
            "detail": "get_children(): vect<string> - Lists all child entries",
            "insertText": "get_children()"
        }),
        json!({
            "label": "files()",
            "kind": 2,
            "detail": "files(): vect<string> - Lists files in directory",
            "insertText": "files()"
        }),
        json!({
            "label": "dirs()",
            "kind": 2,
            "detail": "dirs(): vect<string> - Lists subdirectories",
            "insertText": "dirs()"
        }),
        json!({
            "label": "walk()",
            "kind": 2,
            "detail": "walk(): vect<string> - Recursively walks descendant files",
            "insertText": "walk()"
        }),
        json!({
            "label": "join(sub)",
            "kind": 2,
            "detail": "join(sub: string): string - Joins subpath to directory path",
            "insertText": "join(\"${1:sub}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "exists?()",
            "kind": 2,
            "detail": "exists?(): bool - Checks if directory exists",
            "insertText": "exists?()"
        }),
        json!({
            "label": "empty?()",
            "kind": 2,
            "detail": "empty?(): bool - Checks if directory is empty",
            "insertText": "empty?()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?(): bool - Checks if directory handle is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn vect_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "push(item)",
            "kind": 2,
            "detail": "push(item: T): nothing - Appends an element to the vector",
            "insertText": "push(${1:item})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "len()",
            "kind": 2,
            "detail": "len(): int - Returns number of elements",
            "insertText": "len()"
        }),
        json!({
            "label": "get(index)",
            "kind": 2,
            "detail": "get(index: int): T - Gets element at index",
            "insertText": "get(${1:index})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "pop()",
            "kind": 2,
            "detail": "pop(): T - Removes and returns the last element",
            "insertText": "pop()"
        }),
        json!({
            "label": "empty?()",
            "kind": 2,
            "detail": "empty?(): bool - Checks if vector is empty",
            "insertText": "empty?()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?(): bool - Checks if vector is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn map_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "insert(key, value)",
            "kind": 2,
            "detail": "insert(key: K, value: V): nothing - Inserts key-value pair",
            "insertText": "insert(${1:key}, ${2:value})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "get(key)",
            "kind": 2,
            "detail": "get(key: K): V - Gets value for key or nothing",
            "insertText": "get(${1:key})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "contains(key)",
            "kind": 2,
            "detail": "contains(key: K): bool - Checks if key exists in map",
            "insertText": "contains(${1:key})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "len()",
            "kind": 2,
            "detail": "len(): int - Returns number of entries in map",
            "insertText": "len()"
        }),
        json!({
            "label": "empty?()",
            "kind": 2,
            "detail": "empty?(): bool - Checks if map is empty",
            "insertText": "empty?()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?(): bool - Checks if map is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn io_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "open(path)",
            "kind": 2,
            "detail": "open(path: string) -> Io.File - Opens a file handle",
            "insertText": "open(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "read(path)",
            "kind": 2,
            "detail": "read(path: string) -> string - Reads entire file content",
            "insertText": "read(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "write(path, content)",
            "kind": 2,
            "detail": "write(path: string, content: string) -> nothing - Writes text into file",
            "insertText": "write(\"${1:path}\", \"${2:content}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "copy(from, to)",
            "kind": 2,
            "detail": "copy(from: string, to: string) -> nothing - Copies file to destination",
            "insertText": "copy(\"${1:from}\", \"${2:to}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "move(from, to)",
            "kind": 2,
            "detail": "move(from: string, to: string) -> nothing - Moves file to destination",
            "insertText": "move(\"${1:from}\", \"${2:to}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "delete(path)",
            "kind": 2,
            "detail": "delete(path: string) -> nothing - Deletes file",
            "insertText": "delete(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "size(path)",
            "kind": 2,
            "detail": "size(path: string) -> int - Returns file size in bytes",
            "insertText": "size(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "exists?(path)",
            "kind": 2,
            "detail": "exists?(path: string) -> bool - Checks if file exists",
            "insertText": "exists?(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "file?(path)",
            "kind": 2,
            "detail": "file?(path: string) -> bool - Checks if path is a file",
            "insertText": "file?(\"${1:path}\")",
            "insertTextFormat": 2
        }),
    ]
}

fn fs_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "open(path)",
            "kind": 2,
            "detail": "open(path: string) -> Fs.Dir - Opens directory handle",
            "insertText": "open(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "exists?(path)",
            "kind": 2,
            "detail": "exists?(path: string) -> bool - Checks if file or directory exists",
            "insertText": "exists?(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "file?(path)",
            "kind": 2,
            "detail": "file?(path: string) -> bool - Checks if path is a regular file",
            "insertText": "file?(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "dir?(path)",
            "kind": 2,
            "detail": "dir?(path: string) -> bool - Checks if path is a directory",
            "insertText": "dir?(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "join(a, b)",
            "kind": 2,
            "detail": "join(a: string, b: string) -> string - Joins two path components",
            "insertText": "join(\"${1:a}\", \"${2:b}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "extension(path)",
            "kind": 2,
            "detail": "extension(path: string) -> string - Returns file extension",
            "insertText": "extension(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "filename(path)",
            "kind": 2,
            "detail": "filename(path: string) -> string - Returns file name with extension",
            "insertText": "filename(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "stem(path)",
            "kind": 2,
            "detail": "stem(path: string) -> string - Returns file name without extension",
            "insertText": "stem(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "parent(path)",
            "kind": 2,
            "detail": "parent(path: string) -> string - Returns parent directory path",
            "insertText": "parent(\"${1:path}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "abs(path)",
            "kind": 2,
            "detail": "abs(path: string) -> string - Returns absolute canonical path",
            "insertText": "abs(\"${1:path}\")",
            "insertTextFormat": 2
        }),
    ]
}

fn cli_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "arguments()",
            "kind": 2,
            "detail": "arguments() -> vect<string> - Returns command-line arguments",
            "insertText": "arguments()"
        }),
        json!({
            "label": "paint(text, color)",
            "kind": 2,
            "detail": "paint(text: string, color: hexcolor) -> string - Colors text with ANSI hexcolor",
            "insertText": "paint(\"${1:text}\", #(00FF00))",
            "insertTextFormat": 2
        }),
    ]
}

fn time_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "now()",
            "kind": 2,
            "detail": "now() -> int - Current unix timestamp in seconds",
            "insertText": "now()"
        }),
        json!({
            "label": "now_ms()",
            "kind": 2,
            "detail": "now_ms() -> int - Current unix timestamp in milliseconds",
            "insertText": "now_ms()"
        }),
        json!({
            "label": "sleep(ms)",
            "kind": 2,
            "detail": "sleep(ms: int) -> nothing - Suspends thread execution",
            "insertText": "sleep(${1:1000})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "elapsed(start_ms)",
            "kind": 2,
            "detail": "elapsed(start_ms: int) -> int - Milliseconds elapsed since start_ms",
            "insertText": "elapsed(${1:start_ms})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "benchmark({ ... })",
            "kind": 2,
            "detail": "benchmark(block) -> int - Measures execution duration in ms",
            "insertText": "benchmark({\n\t$0\n})",
            "insertTextFormat": 2
        }),
    ]
}

fn math_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "abs(n)",
            "kind": 2,
            "detail": "abs(n: int) -> int - Absolute value",
            "insertText": "abs(${1:n})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "min(a, b)",
            "kind": 2,
            "detail": "min(a: int, b: int) -> int - Minimum of two integers",
            "insertText": "min(${1:a}, ${2:b})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "max(a, b)",
            "kind": 2,
            "detail": "max(a: int, b: int) -> int - Maximum of two integers",
            "insertText": "max(${1:a}, ${2:b})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "clamp(val, min, max)",
            "kind": 2,
            "detail": "clamp(val: int, min: int, max: int) -> int - Clamps integer in range",
            "insertText": "clamp(${1:val}, ${2:min}, ${3:max})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "pow(base, exp)",
            "kind": 2,
            "detail": "pow(base: int, exp: int) -> int - Base raised to exponent",
            "insertText": "pow(${1:base}, ${2:exp})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "sqr(n)",
            "kind": 2,
            "detail": "sqr(n: int) -> int - Square of an integer",
            "insertText": "sqr(${1:n})",
            "insertTextFormat": 2
        }),
    ]
}

fn mem_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "arena",
            "kind": 9,
            "detail": "module arena - Memory Arena Sector Allocator"
        }),
        json!({
            "label": "arena.allocate(cap)",
            "kind": 2,
            "detail": "arena.allocate([capacity: int]) -> Mem.ArenaSector",
            "insertText": "arena.allocate(${1:1024})",
            "insertTextFormat": 2
        }),
    ]
}

fn sector_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "insert(object)",
            "kind": 2,
            "detail": "insert(object: everything) -> nothing - Stores object in arena",
            "insertText": "insert(${1:object})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "reset()",
            "kind": 2,
            "detail": "reset() -> nothing - Frees all objects in arena",
            "insertText": "reset()"
        }),
        json!({
            "label": "count()",
            "kind": 2,
            "detail": "count() -> int - Number of active objects in arena",
            "insertText": "count()"
        }),
        json!({
            "label": "allocated()",
            "kind": 2,
            "detail": "allocated() -> int - Bytes allocated in arena",
            "insertText": "allocated()"
        }),
        json!({
            "label": "nothing?()",
            "kind": 2,
            "detail": "nothing?() -> bool - Checks if arena sector is nothing",
            "insertText": "nothing?()"
        }),
    ]
}

fn random_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "int(min, max)",
            "kind": 2,
            "detail": "int(min: int, max: int) -> int - Generates random integer",
            "insertText": "int(${1:min}, ${2:max})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "float(min, max)",
            "kind": 2,
            "detail": "float(min: float, max: float) -> float - Generates random float",
            "insertText": "float(${1:min}, ${2:max})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "boolean()",
            "kind": 2,
            "detail": "boolean() -> bool - Generates random boolean",
            "insertText": "boolean()"
        }),
        json!({
            "label": "choice(v)",
            "kind": 2,
            "detail": "choice(v: vect) -> something - Picks random element",
            "insertText": "choice(${1:v})",
            "insertTextFormat": 2
        }),
    ]
}

fn http_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "get(url)",
            "kind": 2,
            "detail": "get(url: string) -> string - HTTP GET request returning body",
            "insertText": "get(\"${1:url}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "post(url, data)",
            "kind": 2,
            "detail": "post(url: string, data: string) -> string - HTTP POST request",
            "insertText": "post(\"${1:url}\", \"${2:data}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "download(url, path)",
            "kind": 2,
            "detail": "download(url: string, path: string) -> bool - Downloads file from URL",
            "insertText": "download(\"${1:url}\", \"${2:path}\")",
            "insertTextFormat": 2
        }),
    ]
}

fn json_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "parse(text)",
            "kind": 2,
            "detail": "parse(text: string) -> something - Parses JSON string into data structure",
            "insertText": "parse(${1:text})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "stringify(value)",
            "kind": 2,
            "detail": "stringify(value: something) -> string - Serializes value into JSON string",
            "insertText": "stringify(${1:value})",
            "insertTextFormat": 2
        }),
    ]
}

fn webview_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "open(title, url, width, height)",
            "kind": 2,
            "detail": "open(title: string, url: string, width: int, height: int) -> nothing",
            "insertText": "open(\"${1:title}\", \"${2:url}\", ${3:800}, ${4:600})",
            "insertTextFormat": 2
        }),
        json!({
            "label": "eval(code)",
            "kind": 2,
            "detail": "eval(code: string) -> nothing - Evaluates JavaScript in webview",
            "insertText": "eval(\"${1:code}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "run()",
            "kind": 2,
            "detail": "run() -> nothing - Runs webview message loop",
            "insertText": "run()"
        }),
        json!({
            "label": "on_message(fn)",
            "kind": 2,
            "detail": "on_message(callback: function) -> nothing - Registers IPC handler",
            "insertText": "on_message(${1:callback})",
            "insertTextFormat": 2
        }),
    ]
}

fn env_completions() -> Vec<Value> {
    vec![
        json!({
            "label": "current()",
            "kind": 2,
            "detail": "current() -> string - Returns current working directory",
            "insertText": "current()"
        }),
        json!({
            "label": "home()",
            "kind": 2,
            "detail": "home() -> string - Returns user home directory",
            "insertText": "home()"
        }),
        json!({
            "label": "get(key)",
            "kind": 2,
            "detail": "get(key: string) -> string - Gets environment variable",
            "insertText": "get(\"${1:key}\")",
            "insertTextFormat": 2
        }),
        json!({
            "label": "set(key, val)",
            "kind": 2,
            "detail": "set(key: string, val: string) -> nothing - Sets environment variable",
            "insertText": "set(\"${1:key}\", \"${2:val}\")",
            "insertTextFormat": 2
        }),
    ]
}

fn get_member_completions(prefix: &str, program_opt: Option<&Program>, imported: &[Program], full_text: &str) -> Vec<Value> {
    let all_programs: Vec<&Program> = program_opt.into_iter().chain(imported.iter()).collect();
    let resolved_ty = resolve_type_of_prefix(prefix, &all_programs, full_text);

    if let Some(ref ty) = resolved_ty {
        match ty {
            Type::String => return string_completions(),
            Type::Vect(_) => return vect_completions(),
            Type::Map(_, _) => return map_completions(),
            Type::Custom(name) => {
                let norm = name.to_lowercase();
                if norm == "io.file" || norm == "file" {
                    return file_completions();
                } else if norm == "fs.dir" || norm == "dir" {
                    return dir_completions();
                } else if norm == "io" {
                    return io_completions();
                } else if norm == "fs" {
                    return fs_completions();
                } else if norm == "cli" {
                    return cli_completions();
                } else if norm == "time" {
                    return time_completions();
                } else if norm == "math" {
                    return math_completions();
                } else if norm == "mem" {
                    return mem_completions();
                } else if norm == "mem.arena" {
                    return vec![json!({
                        "label": "allocate(cap)",
                        "kind": 2,
                        "detail": "allocate([capacity: int]) -> Mem.ArenaSector",
                        "insertText": "allocate(${1:1024})",
                        "insertTextFormat": 2
                    })];
                } else if norm == "mem.arenasector" || norm == "sector" {
                    return sector_completions();
                } else if norm == "env" {
                    return env_completions();
                } else if norm == "random" {
                    return random_completions();
                } else if norm == "http" {
                    return http_completions();
                } else if norm == "json" {
                    return json_completions();
                } else if norm == "webview" {
                    return webview_completions();
                } else {
                    let mut struct_members = Vec::new();
                    for p in &all_programs {
                        for st in &p.structures {
                            if st.name == *name {
                                for m in &st.methods {
                                    struct_members.push(method_to_completion(m, &st.name));
                                }
                                for (f_name, f_ty) in &st.fields {
                                    struct_members.push(json!({
                                        "label": f_name,
                                        "kind": 5, // Field
                                        "detail": format!("{}: {}", f_name, f_ty)
                                    }));
                                }
                            }
                        }
                    }
                    if !struct_members.is_empty() {
                        return struct_members;
                    }
                }
            }
            _ => {}
        }
    }

    let lower = prefix.to_lowercase();
    match lower.as_str() {
        "io" => return io_completions(),
        "fs" => return fs_completions(),
        "cli" => return cli_completions(),
        "time" => return time_completions(),
        "math" => return math_completions(),
        "mem" => return mem_completions(),
        "mem.arena" => return vec![json!({
            "label": "allocate(cap)",
            "kind": 2,
            "detail": "allocate([capacity: int]) -> Mem.ArenaSector",
            "insertText": "allocate(${1:1024})",
            "insertTextFormat": 2
        })],
        "mem.arenasector" | "sector" => return sector_completions(),
        "env" => return env_completions(),
        "random" => return random_completions(),
        "http" => return http_completions(),
        "json" => return json_completions(),
        "webview" => return webview_completions(),
        "file" => return file_completions(),
        "dir" => return dir_completions(),
        _ => {}
    }

    let mut members = Vec::new();
    for p in &all_programs {
        for m in &p.modules {
            if m.name == prefix {
                for func in &m.functions {
                    members.push(func_to_completion(func, None));
                }
            }
        }
        for st in &p.structures {
            if st.name == prefix {
                for m in &st.methods {
                    members.push(method_to_completion(m, &st.name));
                }
                for (f_name, f_ty) in &st.fields {
                    members.push(json!({
                        "label": f_name,
                        "kind": 5, // Field
                        "detail": format!("{}: {}", f_name, f_ty)
                    }));
                }
            }
        }
    }

    members
}
