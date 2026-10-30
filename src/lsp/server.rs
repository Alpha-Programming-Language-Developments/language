use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;
use serde_json::{json, Value};
use crate::diagnostics::error::{AlphaError, Span};
use crate::frontend::ast::{AttachedMethod, FunctionDecl, ImportTarget, Program, Statement};
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

        // Check variables
        for stmt in &program.statements {
            if let Statement::Var(v) = stmt {
                if v.name == word {
                    let (range, _) = span_to_lsp_range(&v.span, &v.name);
                    return json!({ "uri": uri, "range": range });
                }
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
        let current_program = parse_program_safely(&full_text);
        let imported_programs = if let Some(ref prog) = current_program {
            self.get_imported_programs(uri, prog)
        } else {
            Vec::new()
        };

        // 3. Member Access after '.'
        if let Some(prefix) = extract_prefix_before_dot(&line_prefix) {
            let members = get_member_completions(&prefix, current_program.as_ref(), &imported_programs);
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
            for stmt in &program.statements {
                if let Statement::Var(v) = stmt {
                    push_item(json!({
                        "label": v.name,
                        "kind": 6, // Variable
                        "detail": format!("var {}: {}", v.name, v.ty)
                    }));
                }
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
        }

        // C. Standard Core Library Functions
        for name in &["print", "printf", "format", "error"] {
            push_item(json!({
                "label": *name,
                "kind": 3, // Function
                "detail": "function"
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
    let trimmed = &line_prefix[..line_prefix.len() - 1];
    let chars: Vec<char> = trimmed.chars().collect();
    let mut start = chars.len();
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '_' || c == '?' || c == '@' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == chars.len() {
        None
    } else {
        Some(chars[start..].iter().collect())
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

fn get_member_completions(prefix: &str, program_opt: Option<&Program>, imported: &[Program]) -> Vec<Value> {
    let lower = prefix.to_lowercase();
    let builtin_methods: &[(&str, &[&str])] = &[
        ("time", &["now", "now_ms", "sleep", "elapsed", "benchmark"]),
        ("io", &["open", "read", "write", "exists?", "delete", "copy", "move"]),
        ("fs", &["open", "exists?", "file?", "dir?", "join", "extension", "filename", "stem", "parent", "abs"]),
        ("dir", &["open", "create", "delete", "files", "dirs", "walk", "get_children"]),
        ("file", &["read", "write", "close", "size", "lines"]),
        ("math", &["abs", "min", "max", "clamp", "pow", "sqr"]),
        ("cli", &["arguments", "paint"]),
        ("env", &["current", "home", "get", "set"]),
        ("console", &["out"]),
        ("machine", &["out"]),
        ("random", &["int", "float", "boolean", "choice"]),
        ("http", &["get", "post", "download"]),
        ("json", &["parse", "stringify"]),
        ("webview", &["open", "on_message", "eval", "run"]),
        ("mem", &["arena", "allocate"]),
        ("mem.arena", &["allocate"]),
        ("mem.arenasector", &["insert", "reset", "count", "allocated"]),
        ("arenasector", &["insert", "reset", "count", "allocated"]),
        ("sector", &["insert", "reset", "count", "allocated"]),
    ];

    if let Some((_, methods)) = builtin_methods.iter().find(|(m, _)| *m == lower) {
        return methods
            .iter()
            .map(|name| {
                json!({
                    "label": *name,
                    "kind": 2, // Method
                    "detail": "function"
                })
            })
            .collect();
    }

    let mut members = Vec::new();
    let all_programs: Vec<&Program> = program_opt.into_iter().chain(imported.iter()).collect();

    // 1. Direct Module or Structure member match
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

    // 2. Variable instance member match (e.g. `app.`, `win.`, `player.`)
    let mut resolved_type = None;
    for p in &all_programs {
        for stmt in &p.statements {
            if let Statement::Var(v) = stmt {
                if v.name == prefix {
                    resolved_type = Some(format!("{}", v.ty));
                    break;
                }
            }
        }
        if resolved_type.is_some() {
            break;
        }
    }

    if let Some(ty_str) = resolved_type {
        for p in &all_programs {
            for st in &p.structures {
                if st.name == ty_str {
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
    }

    members
}
