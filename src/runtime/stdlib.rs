// Standard Library and Core Modules implementation for Alpha.
use crate::diagnostics::error::{AlphaError, Span};
use crate::runtime::value::{FsDirHandle, IoFileHandle, Value, ValueKey};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Formats a template string containing placeholders like `{}` and `{2}` with values.
pub fn format_alpha_string(template: &str, args: &[Value]) -> Result<String, String> {
    let mut result = String::new();
    let mut arg_idx = 0;
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut inside = String::new();
            let mut closed = false;
            while let Some(&nc) = chars.peek() {
                if nc == '}' {
                    chars.next();
                    closed = true;
                    break;
                }
                inside.push(chars.next().unwrap());
            }

            if !closed {
                result.push('{');
                result.push_str(&inside);
                continue;
            }

            if arg_idx >= args.len() {
                return Err(format!(
                    "Not enough arguments for format string: placeholder '{}' at index {}",
                    inside, arg_idx
                ));
            }

            let val = &args[arg_idx];
            arg_idx += 1;

            if inside.is_empty() {
                // Regular {}
                result.push_str(&val.to_string());
            } else if let Ok(width) = inside.parse::<usize>() {
                // Padded placeholder e.g. {2} -> zero-pad numbers or pad strings
                match val {
                    Value::Int(i) => {
                        let formatted = format!("{:0width$}", i, width = width);
                        result.push_str(&formatted);
                    }
                    Value::Float(f) => {
                        let formatted = format!("{:0width$}", f, width = width);
                        result.push_str(&formatted);
                    }
                    _ => {
                        let s = val.to_string();
                        if s.len() < width {
                            let pad = "0".repeat(width - s.len());
                            result.push_str(&pad);
                        }
                        result.push_str(&s);
                    }
                }
            } else {
                result.push_str(&val.to_string());
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Dispatches method calls on Io.File objects.
pub fn call_io_file_method(
    handle: &Rc<RefCell<IoFileHandle>>,
    method: &str,
    args: &[Value],
    span: Span,
) -> Result<Value, AlphaError> {
    let path = handle.borrow().path.clone();

    match method {
        "content" => {
            match fs::read_to_string(&path) {
                Ok(content) => Ok(Value::String(content)),
                Err(e) => Err(AlphaError::runtime(
                    format!("Failed to read file {:?}: {}", path, e),
                    span,
                )),
            }
        }
        "reset" => {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::write(&path, "") {
                Ok(_) => Ok(Value::Nothing),
                Err(e) => Err(AlphaError::runtime(
                    format!("Failed to reset file {:?}: {}", path, e),
                    span,
                )),
            }
        }
        "write" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("file.write() requires 1 argument", span));
            }
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| {
                    AlphaError::runtime(format!("Failed to open file {:?} for writing: {}", path, e), span)
                })?;

            let text = args[0].to_string();
            file.write_all(text.as_bytes()).map_err(|e| {
                AlphaError::runtime(format!("Failed to write to file {:?}: {}", path, e), span)
            })?;

            Ok(Value::Nothing)
        }
        "something?" => Ok(Value::Boolean(path.exists())),
        "empty?" => {
            if !path.exists() {
                Ok(Value::Boolean(true))
            } else {
                match fs::metadata(&path) {
                    Ok(meta) => Ok(Value::Boolean(meta.len() == 0)),
                    Err(_) => Ok(Value::Boolean(true)),
                }
            }
        }
        "linelen" => {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let count = if content.is_empty() { 0 } else { content.lines().count() };
            Ok(Value::Int(count as i64))
        }
        "symlen" => {
            let content = fs::read_to_string(&path).unwrap_or_default();
            Ok(Value::Int(content.chars().count() as i64))
        }
        "bytelen" => {
            let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(Value::Int(len as i64))
        }
        "size" => {
            let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(Value::Int(len as i64))
        }
        "exists?" | "exists" => Ok(Value::Boolean(path.exists())),
        "delete" => {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| {
                    AlphaError::runtime(format!("Failed to delete file {:?}: {}", path, e), span)
                })?;
            }
            Ok(Value::Nothing)
        }
        "nothing?" => Ok(Value::Boolean(!path.exists())),
        _ => Err(AlphaError::runtime(
            format!("Unknown method '{}' on Io.File", method),
            span,
        )),
    }
}

/// Dispatches method calls on Fs.Dir objects.
pub fn call_fs_dir_method(
    handle: &Rc<RefCell<FsDirHandle>>,
    method: &str,
    args: &[Value],
    span: Span,
) -> Result<Value, AlphaError> {
    let current_path = handle.borrow().path.clone();

    let resolve_target = |arg_str: &str| -> PathBuf {
        if arg_str == "." || arg_str.is_empty() {
            current_path.clone()
        } else {
            let p = Path::new(arg_str);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                current_path.join(p)
            }
        }
    };

    match method {
        "open" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let new_path = resolve_target(&target_str);
            Ok(Value::FsDir(Rc::new(RefCell::new(FsDirHandle { path: new_path }))))
        }
        "delete" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            if target_path.exists() {
                if target_path.is_dir() {
                    fs::remove_dir_all(&target_path).map_err(|e| {
                        AlphaError::runtime(format!("Failed to delete directory {:?}: {}", target_path, e), span)
                    })?;
                } else {
                    fs::remove_file(&target_path).map_err(|e| {
                        AlphaError::runtime(format!("Failed to delete file {:?}: {}", target_path, e), span)
                    })?;
                }
            }
            Ok(Value::Nothing)
        }
        "create" => {
            let name = args.first().map(|a| a.to_string()).unwrap_or_default();
            let new_dir = resolve_target(&name);
            fs::create_dir_all(&new_dir).map_err(|e| {
                AlphaError::runtime(format!("Failed to create directory {:?}: {}", new_dir, e), span)
            })?;
            Ok(Value::Nothing)
        }
        "rename" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("dir.rename() requires new name argument", span));
            }
            let new_name = args[0].to_string();
            let target_str = args.get(1).map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);

            let new_path = target_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(&new_name);

            fs::rename(&target_path, &new_path).map_err(|e| {
                AlphaError::runtime(
                    format!("Failed to rename {:?} to {:?}: {}", target_path, new_path, e),
                    span,
                )
            })?;
            Ok(Value::Nothing)
        }
        "reset" => {
            if current_path.exists() && current_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&current_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let _ = fs::remove_dir_all(&path);
                        } else {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
            Ok(Value::Nothing)
        }
        "get_children" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            let mut list = Vec::new();

            if target_path.exists() && target_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&target_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let abs = fs::canonicalize(&p).unwrap_or(p);
                        list.push(Value::String(abs.to_string_lossy().to_string()));
                    }
                }
            }
            Ok(Value::Vect(Rc::new(RefCell::new(list))))
        }
        "files" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            let mut list = Vec::new();
            if target_path.exists() && target_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&target_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            list.push(Value::String(p.to_string_lossy().to_string()));
                        }
                    }
                }
            }
            Ok(Value::Vect(Rc::new(RefCell::new(list))))
        }
        "dirs" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            let mut list = Vec::new();
            if target_path.exists() && target_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&target_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            list.push(Value::String(p.to_string_lossy().to_string()));
                        }
                    }
                }
            }
            Ok(Value::Vect(Rc::new(RefCell::new(list))))
        }
        "walk" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            let mut list = Vec::new();
            fn walk_dir(dir: &Path, list: &mut Vec<Value>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        list.push(Value::String(p.to_string_lossy().to_string()));
                        if p.is_dir() {
                            walk_dir(&p, list);
                        }
                    }
                }
            }
            if target_path.exists() && target_path.is_dir() {
                walk_dir(&target_path, &mut list);
            }
            Ok(Value::Vect(Rc::new(RefCell::new(list))))
        }
        "exists" | "exists?" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            Ok(Value::Boolean(target_path.exists()))
        }
        "join" => {
            if args.is_empty() {
                return Ok(Value::String(current_path.to_string_lossy().to_string()));
            }
            let mut res = current_path.clone();
            for a in args {
                res = res.join(a.to_string());
            }
            Ok(Value::String(res.to_string_lossy().to_string()))
        }
        "nothing?" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            Ok(Value::Boolean(!target_path.exists()))
        }
        "empty?" => {
            let target_str = args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
            let target_path = resolve_target(&target_str);
            if !target_path.exists() || !target_path.is_dir() {
                Ok(Value::Boolean(true))
            } else {
                let count = fs::read_dir(&target_path).map(|mut d| d.next().is_none()).unwrap_or(true);
                Ok(Value::Boolean(count))
            }
        }
        _ => Err(AlphaError::runtime(
            format!("Unknown method '{}' on Fs.Dir", method),
            span,
        )),
    }
}

/// Executes a terminal/shell command cross-platform and returns its terminal output.
pub fn execute_console_command(command: &str, span: Span) -> Result<String, AlphaError> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    let output_res = Command::new("cmd")
        .args(["/D", "/C", command])
        .output();

    #[cfg(not(target_os = "windows"))]
    let output_res = Command::new("sh")
        .args(["-c", command])
        .output();

    let output = output_res.map_err(|e| {
        AlphaError::runtime(
            format!("Failed to execute terminal command '{}': {}", command, e),
            span,
        )
    })?;

    let raw_bytes = if !output.stdout.is_empty() {
        output.stdout
    } else {
        output.stderr
    };

    let text = decode_terminal_bytes(&raw_bytes);
    let trimmed = text.strip_suffix("\r\n").unwrap_or_else(|| text.strip_suffix('\n').unwrap_or(&text));
    let cleaned = trimmed.trim_start_matches('\u{c}');
    Ok(cleaned.to_string())
}

#[cfg(target_os = "windows")]
fn decode_terminal_bytes(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MultiByteToWideChar(
            CodePage: u32,
            dwFlags: u32,
            lpMultiByteStr: *const u8,
            cbMultiByte: i32,
            lpWideCharStr: *mut u16,
            cchWideChar: i32,
        ) -> i32;
    }
    const CP_OEMCP: u32 = 1;
    unsafe {
        let len = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if len > 0 {
            let mut wide = vec![0u16; len as usize];
            MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), len);
            return String::from_utf16_lossy(&wide);
        }
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(not(target_os = "windows"))]
fn decode_terminal_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

/// Downloads a remote Alpha module from a URL.
pub fn fetch_web_module(url: &str) -> Result<String, String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    let res = Command::new("curl.exe")
        .args(["-s", "-L", "-A", "Mozilla/5.0", url])
        .output();

    #[cfg(not(target_os = "windows"))]
    let res = Command::new("curl")
        .args(["-s", "-L", "-A", "Mozilla/5.0", url])
        .output();

    match res {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout).to_string();
            if body.trim().is_empty() {
                Err(format!("Empty response from web module '{}'", url))
            } else {
                Ok(body)
            }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(format!("Failed to fetch web module '{}': {}", url, err))
        }
        Err(e) => Err(format!("Could not execute curl to download web module '{}': {}", url, e)),
    }
}

/// Formats text with 24-bit ANSI truecolor escape codes for terminal output.
pub fn format_hex_paint(text: &str, hex: &str) -> String {
    let (r, g, b) = crate::runtime::value::Value::hex_to_rgb(hex);
    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text)
}

/// Parses the contents of a `packages.alpaks` file:
/// "alias" = import web::"url"
pub fn parse_packages_alpaks(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        if let Some((left, right)) = trimmed.split_once('=') {
            let alias = left.trim().trim_matches('"').trim_matches('\'').trim().to_string();
            let right_trimmed = right.trim();
            if let Some(url_part) = right_trimmed.strip_prefix("import") {
                let url_part = url_part.trim();
                if let Some(url) = url_part.strip_prefix("web::") {
                    let cleaned_url = url.trim().trim_matches('"').trim_matches('\'').to_string();
                    map.insert(alias, cleaned_url);
                }
            }
        }
    }
    map
}

// -------------------------------------------------------------
// Random Number Generator (SplitMix64)
// -------------------------------------------------------------
use std::sync::atomic::{AtomicU64, Ordering};

static RNG_STATE: AtomicU64 = AtomicU64::new(0);

fn init_rng_if_needed() {
    if RNG_STATE.load(Ordering::Relaxed) == 0 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        let pid = std::process::id() as u64;
        let seed = nanos ^ (pid << 32) ^ 0xda942042e4dd58b5;
        RNG_STATE.store(if seed == 0 { 0x123456789abcdef } else { seed }, Ordering::Relaxed);
    }
}

pub fn rng_seed(seed: i64) {
    let s = (seed as u64) ^ 0xda942042e4dd58b5;
    RNG_STATE.store(if s == 0 { 0x123456789abcdef } else { s }, Ordering::Relaxed);
}

pub fn rng_next_u64() -> u64 {
    init_rng_if_needed();
    loop {
        let current = RNG_STATE.load(Ordering::Relaxed);
        let mut x = current.wrapping_add(0x9e3779b97f4a7c15);
        if RNG_STATE.compare_exchange_weak(current, x, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            return x ^ (x >> 31);
        }
    }
}

pub fn rng_int_range(min: i64, max: i64) -> i64 {
    if min >= max {
        return min;
    }
    let range = (max - min + 1) as u64;
    let val = rng_next_u64() % range;
    min + val as i64
}

pub fn rng_float_range(min: f64, max: f64) -> f64 {
    if min >= max {
        return min;
    }
    let unit = (rng_next_u64() >> 11) as f64 / ((1u64 << 53) as f64);
    min + unit * (max - min)
}

pub fn rng_bool() -> bool {
    (rng_next_u64() & 1) == 1
}

pub fn rng_string(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let idx = (rng_next_u64() % CHARSET.len() as u64) as usize;
        s.push(CHARSET[idx] as char);
    }
    s
}

// -------------------------------------------------------------
// -------------------------------------------------------------
// Enhanced Fs Namespace Methods (Paths & Directories)
// -------------------------------------------------------------
pub fn call_fs_namespace_method(
    method: &str,
    args: &[Value],
    span: Span,
) -> Result<Value, AlphaError> {
    match method {
        "join" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let mut path = PathBuf::from(args[0].to_string());
            for arg in &args[1..] {
                path.push(arg.to_string());
            }
            Ok(Value::String(path.to_string_lossy().to_string()))
        }
        "exists?" | "exists" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Fs.exists?(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            Ok(Value::Boolean(p.exists()))
        }
        "file?" | "is_file" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Fs.file?(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            Ok(Value::Boolean(p.is_file()))
        }
        "dir?" | "is_dir" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Fs.dir?(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            Ok(Value::Boolean(p.is_dir()))
        }
        "extension" | "ext" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let p = PathBuf::from(args[0].to_string());
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            Ok(Value::String(ext.to_string()))
        }
        "filename" | "name" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let p = PathBuf::from(args[0].to_string());
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            Ok(Value::String(name.to_string()))
        }
        "stem" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let p = PathBuf::from(args[0].to_string());
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            Ok(Value::String(stem.to_string()))
        }
        "parent" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let p = PathBuf::from(args[0].to_string());
            let par = p.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            Ok(Value::String(par))
        }
        "abs" | "absolute" => {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            let p = PathBuf::from(args[0].to_string());
            let abs = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            Ok(Value::String(abs.to_string_lossy().to_string()))
        }
        _ => Err(AlphaError::runtime(format!("Unknown method '{}' on Fs", method), span)),
    }
}

// -------------------------------------------------------------
// Io Namespace Methods (File Operations)
// -------------------------------------------------------------
pub fn call_io_namespace_method(
    method: &str,
    args: &[Value],
    span: Span,
) -> Result<Value, AlphaError> {
    match method {
        "read" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Io.read(path) requires a path argument", span));
            }
            let p = args[0].to_string();
            match fs::read_to_string(&p) {
                Ok(content) => Ok(Value::String(content)),
                Err(e) => Err(AlphaError::runtime(format!("Failed to read file '{}': {}", p, e), span)),
            }
        }
        "write" => {
            if args.len() < 2 {
                return Err(AlphaError::runtime("Io.write(path, content) requires path and content", span));
            }
            let p = PathBuf::from(args[0].to_string());
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let content = args[1].to_string();
            fs::write(&p, content).map_err(|e| {
                AlphaError::runtime(format!("Failed to write to file '{:?}': {}", p, e), span)
            })?;
            Ok(Value::Nothing)
        }
        "copy" => {
            if args.len() < 2 {
                return Err(AlphaError::runtime("Io.copy(from, to) requires 2 arguments", span));
            }
            let from = PathBuf::from(args[0].to_string());
            let to = PathBuf::from(args[1].to_string());
            copy_recursive(&from, &to).map_err(|e| {
                AlphaError::runtime(format!("Failed to copy from '{:?}' to '{:?}': {}", from, to, e), span)
            })?;
            Ok(Value::Nothing)
        }
        "move" | "rename" => {
            if args.len() < 2 {
                return Err(AlphaError::runtime("Io.move(from, to) requires 2 arguments", span));
            }
            let from = PathBuf::from(args[0].to_string());
            let to = PathBuf::from(args[1].to_string());
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&from, &to).map_err(|e| {
                AlphaError::runtime(format!("Failed to move '{:?}' to '{:?}': {}", from, to, e), span)
            })?;
            Ok(Value::Nothing)
        }
        "delete" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Io.delete(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            if p.exists() {
                if p.is_dir() {
                    fs::remove_dir_all(&p).map_err(|e| {
                        AlphaError::runtime(format!("Failed to delete directory '{:?}': {}", p, e), span)
                    })?;
                } else {
                    fs::remove_file(&p).map_err(|e| {
                        AlphaError::runtime(format!("Failed to delete file '{:?}': {}", p, e), span)
                    })?;
                }
            }
            Ok(Value::Nothing)
        }
        "size" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Io.size(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            let len = fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0);
            Ok(Value::Int(len))
        }
        "exists?" | "exists" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Io.exists?(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            Ok(Value::Boolean(p.exists()))
        }
        "file?" | "is_file" => {
            if args.is_empty() {
                return Err(AlphaError::runtime("Io.file?(path) requires a path argument", span));
            }
            let p = PathBuf::from(args[0].to_string());
            Ok(Value::Boolean(p.is_file()))
        }
        _ => Err(AlphaError::runtime(format!("Unknown method '{}' on Io", method), span)),
    }
}

fn copy_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let to_child = to.join(entry.file_name());
            if file_type.is_dir() {
                copy_recursive(&entry.path(), &to_child)?;
            } else {
                fs::copy(entry.path(), to_child)?;
            }
        }
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

// -------------------------------------------------------------
// Core/Time Implementation
// -------------------------------------------------------------
pub fn time_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn time_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn time_sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

pub fn time_elapsed(start_ms: i64) -> i64 {
    time_now_ms() - start_ms
}

// -------------------------------------------------------------
// Core/Http Implementation
// -------------------------------------------------------------
pub fn http_get(url: &str, span: Span) -> Result<String, AlphaError> {
    let output = Command::new("curl")
        .args(["-s", "-L", url])
        .output()
        .map_err(|e| AlphaError::runtime(format!("Failed to execute http request via curl: {}", e), span))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(AlphaError::runtime(format!("HTTP request failed: {}", err_msg.trim()), span));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn http_post(url: &str, body: &str, span: Span) -> Result<String, AlphaError> {
    let mut child = Command::new("curl")
        .args(["-s", "-L", "-X", "POST", "-H", "Content-Type: application/json", "--data-binary", "@-", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AlphaError::runtime(format!("Failed to execute http post via curl: {}", e), span))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(body.as_bytes());
    }

    let output = child.wait_with_output()
        .map_err(|e| AlphaError::runtime(format!("Failed to wait for curl: {}", e), span))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(AlphaError::runtime(format!("HTTP post failed: {}", err_msg.trim()), span));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn http_download(url: &str, dest_path: &str, span: Span) -> Result<bool, AlphaError> {
    let p = Path::new(dest_path);
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let output = Command::new("curl")
        .args(["-s", "-L", "-o", dest_path, url])
        .output()
        .map_err(|e| AlphaError::runtime(format!("Failed to download via curl: {}", e), span))?;
    Ok(output.status.success())
}

// -------------------------------------------------------------
// Core/Json Implementation (Zero-dependency std-only)
// -------------------------------------------------------------
struct JsonParser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(chars: &'a [char]) -> Self {
        Self { chars, pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('"') => self.parse_string(),
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("Unexpected character '{}' at position {}", c, self.pos)),
            None => Err("Unexpected end of JSON input".to_string()),
        }
    }

    fn parse_string(&mut self) -> Result<Value, String> {
        self.advance(); // consume opening '"'
        let mut s = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => return Ok(Value::String(s)),
                '\\' => {
                    match self.advance() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('b') => s.push('\x08'),
                        Some('f') => s.push('\x0c'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                if let Some(h) = self.advance() {
                                    hex.push(h);
                                }
                            }
                            if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    s.push(ch);
                                }
                            }
                        }
                        Some(other) => {
                            s.push('\\');
                            s.push(other);
                        }
                        None => return Err("Unterminated escape sequence in string".to_string()),
                    }
                }
                other => s.push(other),
            }
        }
        Err("Unterminated string in JSON".to_string())
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        self.advance(); // consume '{'
        self.skip_whitespace();
        let mut map = HashMap::new();
        if let Some('}') = self.peek() {
            self.advance();
            return Ok(Value::Map(Rc::new(RefCell::new(map))));
        }

        loop {
            self.skip_whitespace();
            let key = match self.parse_value()? {
                Value::String(s) => s,
                other => return Err(format!("Expected string key in object, found {}", other)),
            };
            self.skip_whitespace();
            if self.advance() != Some(':') {
                return Err("Expected ':' after object key".to_string());
            }
            let val = self.parse_value()?;
            map.insert(ValueKey::String(key), val);

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                _ => return Err("Expected ',' or '}' in object".to_string()),
            }
        }
        Ok(Value::Map(Rc::new(RefCell::new(map))))
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.advance(); // consume '['
        self.skip_whitespace();
        let mut list = Vec::new();
        if let Some(']') = self.peek() {
            self.advance();
            return Ok(Value::Vect(Rc::new(RefCell::new(list))));
        }

        loop {
            let val = self.parse_value()?;
            list.push(val);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                _ => return Err("Expected ',' or ']' in array".to_string()),
            }
        }
        Ok(Value::Vect(Rc::new(RefCell::new(list))))
    }

    fn parse_bool(&mut self) -> Result<Value, String> {
        if self.chars[self.pos..].starts_with(&['t', 'r', 'u', 'e']) {
            self.pos += 4;
            Ok(Value::Boolean(true))
        } else if self.chars[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            self.pos += 5;
            Ok(Value::Boolean(false))
        } else {
            Err("Expected boolean 'true' or 'false'".to_string())
        }
    }

    fn parse_null(&mut self) -> Result<Value, String> {
        if self.chars[self.pos..].starts_with(&['n', 'u', 'l', 'l']) {
            self.pos += 4;
            Ok(Value::Nothing)
        } else {
            Err("Expected 'null'".to_string())
        }
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        let mut is_float = false;
        if let Some('-') = self.peek() {
            self.advance();
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' {
                is_float = true;
                self.advance();
            } else if c == 'e' || c == 'E' {
                is_float = true;
                self.advance();
                if let Some('+') | Some('-') = self.peek() {
                    self.advance();
                }
            } else {
                break;
            }
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        if is_float {
            raw.parse::<f64>().map(Value::Float).map_err(|e| e.to_string())
        } else {
            raw.parse::<i64>().map(Value::Int).map_err(|e| e.to_string())
        }
    }
}

pub fn json_parse(json_str: &str, span: Span) -> Result<Value, AlphaError> {
    let chars: Vec<char> = json_str.chars().collect();
    let mut parser = JsonParser::new(&chars);
    let val = parser.parse_value().map_err(|e| AlphaError::runtime(format!("Failed to parse JSON: {}", e), span))?;
    Ok(val)
}

pub fn json_stringify(val: &Value, _span: Span) -> Result<String, AlphaError> {
    let mut out = String::new();
    format_json_value(val, &mut out);
    Ok(out)
}

fn format_json_value(val: &Value, out: &mut String) {
    match val {
        Value::Nothing => out.push_str("null"),
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&f.to_string()),
        Value::Char(c) => format_json_str(&c.to_string(), out),
        Value::String(s) => format_json_str(s, out),
        Value::Vect(rc) => {
            out.push('[');
            let v = rc.borrow();
            for (idx, item) in v.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_json_value(item, out);
            }
            out.push(']');
        }
        Value::Map(rc) => {
            out.push('{');
            let m = rc.borrow();
            let mut first = true;
            for (k, v) in m.iter() {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                format_json_str(&k.to_string(), out);
                out.push_str(": ");
                format_json_value(v, out);
            }
            out.push('}');
        }
        Value::Tuple(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                format_json_value(item, out);
            }
            out.push(']');
        }
        Value::StructureInstance(inst) => {
            out.push('{');
            let fields = inst.fields.borrow();
            let mut first = true;
            for (k, v) in fields.iter() {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                format_json_str(k, out);
                out.push_str(": ");
                format_json_value(v, out);
            }
            out.push('}');
        }
        _ => format_json_str(&val.to_string(), out),
    }
}

fn format_json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
}


