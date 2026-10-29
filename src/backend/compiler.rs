// Native compiler backend for the Alpha programming language (`alpha build`).
use crate::diagnostics::error::{AlphaError, Span};
use crate::driver::project::load_project_config;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::typechecker::TypeChecker;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// Embed the engine source files into the compiler binary.
const ERROR_RS: &str = include_str!("../diagnostics/error.rs");
const TOKEN_RS: &str = include_str!("../frontend/token.rs");
const AST_RS: &str = include_str!("../frontend/ast.rs");
const LEXER_RS: &str = include_str!("../frontend/lexer.rs");
const PARSER_RS: &str = include_str!("../frontend/parser.rs");
const TYPES_RS: &str = include_str!("../semantics/types.rs");
const TYPECHECKER_RS: &str = include_str!("../semantics/typechecker.rs");
const VALUE_RS: &str = include_str!("../runtime/value.rs");
const ENV_RS: &str = include_str!("../runtime/env.rs");
const STDLIB_RS: &str = include_str!("../runtime/stdlib.rs");
const INTERPRETER_RS: &str = include_str!("../runtime/interpreter.rs");
const STUB_WEBVIEW_RS: &str = r#"
#[derive(Clone, Debug)]
pub struct WebviewWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub html: Option<String>,
    pub url: Option<String>,
}

impl Default for WebviewWindowConfig {
    fn default() -> Self {
        Self {
            title: "WebAlph Application".to_string(),
            width: 900,
            height: 650,
            resizable: true,
            html: None,
            url: None,
        }
    }
}

pub fn run_webview_loop(_interp: &mut crate::runtime::interpreter::Interpreter, span: crate::diagnostics::error::Span) -> Result<crate::runtime::value::Value, crate::diagnostics::error::AlphaError> {
    Err(crate::diagnostics::error::AlphaError::runtime("Webview GUI mode requires running via `alpha run`.", span))
}
"#;

/// Compiles an Alpha project into a native binary located in the `build/` folder.
pub fn build_project(project_dir: &Path) -> Result<PathBuf, AlphaError> {
    let config = load_project_config(project_dir)?;
    let project_name = config.project.name;

    let src_dir = project_dir.join("src");
    let main_alpha_path = src_dir.join("main.alpha");

    if !main_alpha_path.exists() {
        return Err(AlphaError::runtime(
            format!("Entry point not found: {:?}", main_alpha_path),
            Span::default(),
        ));
    }

    let build_dir = project_dir.join("build");
    build_custom(
        &project_name,
        &main_alpha_path,
        project_dir,
        &build_dir,
        &src_dir,
    )
}

/// Compiles a single standalone Alpha file into a native binary in `build/<filename>.exe`.
pub fn build_file(file_path: &Path) -> Result<PathBuf, AlphaError> {
    if !file_path.exists() {
        return Err(AlphaError::runtime(
            format!("File not found: {:?}", file_path),
            Span::default(),
        ));
    }

    let file_stem = file_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "program".to_string());

    let parent_dir = file_path.parent().unwrap_or(Path::new("."));
    let build_dir = parent_dir.join("build");

    build_custom(
        &file_stem,
        file_path,
        parent_dir,
        &build_dir,
        parent_dir,
    )
}

fn build_custom(
    project_name: &str,
    entry_path: &Path,
    working_dir: &Path,
    build_dir: &Path,
    search_dir: &Path,
) -> Result<PathBuf, AlphaError> {
    // 1. Read source
    let source = fs::read_to_string(entry_path).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to read {:?}: {}", entry_path, e),
            Span::default(),
        )
    })?;

    // 2. Lex & Parse
    println!("[Alpha] Checking syntax for '{}'...", project_name);
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| e.with_file(entry_path.to_string_lossy()))?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().map_err(|e| e.with_file(entry_path.to_string_lossy()))?;

    // 3. Type Check
    println!("[Alpha] Type checking program...");
    let mut typechecker = TypeChecker::new()
        .with_file(entry_path.to_string_lossy())
        .with_working_dir(working_dir.to_path_buf());
    typechecker.check_program(&program).map_err(|e| e.with_file(entry_path.to_string_lossy()))?;

    // 4. Prepare build folder
    fs::create_dir_all(build_dir).map_err(|e| {
        AlphaError::runtime(format!("Failed to create build directory: {}", e), Span::default())
    })?;

    let cache_dir = build_dir.join(".cache");
    fs::create_dir_all(&cache_dir).map_err(|e| {
        AlphaError::runtime(format!("Failed to create build cache: {}", e), Span::default())
    })?;

    // 5. Populate cache with modular engine source files
    let diag_dir = cache_dir.join("diagnostics");
    let front_dir = cache_dir.join("frontend");
    let sem_dir = cache_dir.join("semantics");
    let run_dir = cache_dir.join("runtime");

    fs::create_dir_all(&diag_dir).unwrap();
    fs::create_dir_all(&front_dir).unwrap();
    fs::create_dir_all(&sem_dir).unwrap();
    fs::create_dir_all(&run_dir).unwrap();

    fs::write(diag_dir.join("error.rs"), ERROR_RS).unwrap();
    fs::write(diag_dir.join("mod.rs"), "pub mod error;\npub use self::error::*;\n").unwrap();

    fs::write(front_dir.join("token.rs"), TOKEN_RS).unwrap();
    fs::write(front_dir.join("ast.rs"), AST_RS).unwrap();
    fs::write(front_dir.join("lexer.rs"), LEXER_RS).unwrap();
    fs::write(front_dir.join("parser.rs"), PARSER_RS).unwrap();
    fs::write(front_dir.join("mod.rs"), "pub mod token;\npub mod ast;\npub mod lexer;\npub mod parser;\npub use self::token::*;\npub use self::ast::*;\npub use self::lexer::*;\npub use self::parser::*;\n").unwrap();

    fs::write(sem_dir.join("types.rs"), TYPES_RS).unwrap();
    fs::write(sem_dir.join("typechecker.rs"), TYPECHECKER_RS).unwrap();
    fs::write(sem_dir.join("mod.rs"), "pub mod types;\npub mod typechecker;\npub use self::types::*;\npub use self::typechecker::*;\n").unwrap();

    fs::write(run_dir.join("value.rs"), VALUE_RS).unwrap();
    fs::write(run_dir.join("env.rs"), ENV_RS).unwrap();
    fs::write(run_dir.join("stdlib.rs"), STDLIB_RS).unwrap();
    fs::write(run_dir.join("interpreter.rs"), INTERPRETER_RS).unwrap();
    fs::write(run_dir.join("webview.rs"), STUB_WEBVIEW_RS).unwrap();
    fs::write(run_dir.join("mod.rs"), "pub mod value;\npub mod env;\npub mod stdlib;\npub mod interpreter;\npub mod webview;\npub use self::value::*;\npub use self::env::*;\npub use self::stdlib::*;\npub use self::interpreter::*;\npub use self::webview::*;\n").unwrap();

    // 6. Collect and embed all project .alpha files (including custom Core modules)
    let mut embedded_files = Vec::new();
    collect_alpha_files(search_dir, search_dir, &mut embedded_files)?;

    let mut register_code = String::new();
    for (rel_path, content) in &embedded_files {
        let escaped_content = content.replace('\\', "\\\\").replace('"', "\\\"");
        register_code.push_str(&format!(
            "    interp.register_source_module(\"{}\", \"{}\");\n",
            rel_path, escaped_content
        ));
        register_code.push_str(&format!(
            "    interp.register_source_module(\"src/{}\", \"{}\");\n",
            rel_path, escaped_content
        ));
    }

    let escaped_source = source.replace('\\', "\\\\").replace('"', "\\\"");
    let entry_file_name = entry_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "main.alpha".to_string());
    let runner_code = format!(
        r#"
#![allow(unused_imports, dead_code)]
mod diagnostics;
mod frontend;
mod semantics;
mod runtime;

const EMBEDDED_SOURCE: &str = "{source}";

fn main() {{
    let mut interp = runtime::interpreter::Interpreter::new();
{register_code}
    if let Err(e) = interp.eval_source(EMBEDDED_SOURCE, Some("{entry_file_name}")) {{
        eprintln!("{{}}", e.format_with_source(EMBEDDED_SOURCE));
        std::process::exit(1);
    }}
}}
"#,
        source = escaped_source,
        entry_file_name = entry_file_name,
        register_code = register_code
    );

    let runner_path = cache_dir.join("main.rs");
    fs::write(&runner_path, runner_code).map_err(|e| {
        AlphaError::runtime(format!("Failed to write compiler runner: {}", e), Span::default())
    })?;

    // 7. Invoke rustc to produce native executable
    let exe_name = if cfg!(windows) {
        format!("{}.exe", project_name)
    } else {
        project_name.to_string()
    };
    let output_exe_path = build_dir.join(&exe_name);

    println!("[Alpha] Compiling native binary '{}' via rustc...", exe_name);
    let status = Command::new("rustc")
        .arg("--edition=2024")
        .arg("-O")
        .arg(&runner_path)
        .arg("-o")
        .arg(&output_exe_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            // Clean up cache folder
            let _ = fs::remove_dir_all(&cache_dir);
            println!("[Alpha] Build successful! Output: {:?}", output_exe_path);
            Ok(output_exe_path)
        }
        Ok(s) => Err(AlphaError::runtime(
            format!("rustc compilation failed with exit code: {:?}", s.code()),
            Span::default(),
        )),
        Err(e) => Err(AlphaError::runtime(
            format!("Failed to execute rustc compiler: {}. Make sure 'rustc' is in your PATH.", e),
            Span::default(),
        )),
    }
}

fn collect_alpha_files(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<(String, String)>,
) -> Result<(), AlphaError> {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_alpha_files(base_dir, &path, files)?;
            } else if path.extension().map_or(false, |ext| ext == "alpha") {
                let rel = path.strip_prefix(base_dir).unwrap_or(&path);
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let content = fs::read_to_string(&path).map_err(|e| {
                    AlphaError::runtime(format!("Failed to read file {:?}: {}", path, e), Span::default())
                })?;
                files.push((rel_str, content));
            }
        }
    }
    Ok(())
}
