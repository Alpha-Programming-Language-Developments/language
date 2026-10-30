// Native compiler backend for the Alpha programming language (`alpha build`).
use crate::diagnostics::error::{AlphaError, Span};
use crate::driver::project::load_project_config;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::typechecker::TypeChecker;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let all_typechecked = !program.functions.is_empty()
        && program.functions.iter().all(|f| f.is_typechecked);

    if all_typechecked {
        println!("[Alpha] Skipping type checking (#typechecked)...");
        let mut typechecker = TypeChecker::new()
            .with_file(entry_path.to_string_lossy())
            .with_working_dir(working_dir.to_path_buf());
        typechecker.check_typechecked_only(&program).map_err(|e| e.with_file(entry_path.to_string_lossy()))?;
    } else {
        let has_any_tc = program.functions.iter().any(|f| f.is_typechecked);
        if has_any_tc {
            println!("[Alpha] Type checking program (skipping #typechecked functions)...");
        } else {
            println!("[Alpha] Type checking program...");
        }
        let mut typechecker = TypeChecker::new()
            .with_file(entry_path.to_string_lossy())
            .with_working_dir(working_dir.to_path_buf());
        typechecker.check_program(&program).map_err(|e| e.with_file(entry_path.to_string_lossy()))?;
    }

    // 4. Prepare build folder
    fs::create_dir_all(build_dir).map_err(|e| {
        AlphaError::runtime(format!("Failed to create build directory: {}", e), Span::default())
    })?;

    let cache_dir = build_dir.join(".cache");
    fs::create_dir_all(&cache_dir).map_err(|e| {
        AlphaError::runtime(format!("Failed to create build cache: {}", e), Span::default())
    })?;

    let exe_name = if cfg!(windows) {
        format!("{}.exe", project_name)
    } else {
        project_name.to_string()
    };
    let output_exe_path = build_dir.join(&exe_name);

    // 1. Direct LLVM IR Backend (Maximum performance directly via LLVM optimizer & Clang)
    let clang_cmd = if Path::new(r"C:\Program Files\LLVM\bin\clang.exe").exists() {
        r"C:\Program Files\LLVM\bin\clang.exe".to_string()
    } else {
        "clang".to_string()
    };

    let rt_lib_name = if cfg!(windows) { "alpha_rt.lib" } else { "libalpha_rt.a" };
    let mut rt_lib_path: Option<PathBuf> = None;

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join(rt_lib_name);
            if candidate.exists() {
                rt_lib_path = Some(candidate);
            }
        }
    }

    if rt_lib_path.is_none() {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let candidate = PathBuf::from(home).join(".cargo").join("bin").join(rt_lib_name);
            if candidate.exists() {
                rt_lib_path = Some(candidate);
            }
        }
    }

    if rt_lib_path.is_none() {
        let candidate = PathBuf::from("target/release").join(rt_lib_name);
        if candidate.exists() {
            rt_lib_path = Some(candidate);
        }
    }

    let mut llvm_cg = crate::backend::LlvmCodegen::new();
    if let Ok(llvm_ir) = llvm_cg.generate_module(&program) {
        let ll_path = cache_dir.join("program.ll");
        if fs::write(&ll_path, &llvm_ir).is_ok() {
            println!("[Alpha] Compiling native binary '{}' via direct LLVM Backend (-O3)...", exe_name);
            let mut cmd = Command::new(&clang_cmd);
            cmd.arg("-O3")
                .arg(&ll_path);

            if let Some(ref lib) = rt_lib_path {
                cmd.arg(lib);
                if cfg!(windows) {
                    cmd.arg("-luserenv").arg("-lws2_32").arg("-lntdll");
                } else {
                    cmd.arg("-lpthread").arg("-ldl").arg("-lm");
                }
            }

            cmd.arg("-o").arg(&output_exe_path);
            let output = cmd.output();

            if let Ok(out) = output {
                if out.status.success() {
                    let _ = fs::remove_dir_all(&cache_dir);
                    println!("[Alpha] Direct LLVM build successful! Output: {:?}", output_exe_path);
                    return Ok(output_exe_path);
                }
            }
        }
    }

    // 2. Standalone Alpha Bundle: Full runtime engine with Webview (tao + wry), networking, and all modules
    println!("[Alpha] Packaging native binary '{}' with full runtime...", exe_name);
    let mut embedded_files = Vec::new();
    collect_alpha_files(search_dir, search_dir, &mut embedded_files)?;

    let entry_file = entry_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "main.alpha".to_string());

    let bundle = crate::driver::project::AlphaProjectBundle {
        entry_file,
        source: source.clone(),
        files: embedded_files,
    };

    let base_exe = find_alpha_base_exe()?;
    create_bundled_exe(&base_exe, &output_exe_path, &bundle)?;

    let _ = fs::remove_dir_all(&cache_dir);
    println!("[Alpha] Build successful! Output: {:?}", output_exe_path);
    Ok(output_exe_path)
}

fn find_alpha_base_exe() -> Result<PathBuf, AlphaError> {
    let exe_name = if cfg!(windows) { "alpha.exe" } else { "alpha" };

    if let Ok(cur) = std::env::current_exe() {
        if cur.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case(exe_name)) {
            return Ok(cur);
        }
        if let Some(parent) = cur.parent() {
            let candidate = parent.join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let candidate = PathBuf::from(home).join(".cargo").join("bin").join(exe_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let candidate = PathBuf::from("target/release").join(exe_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    Err(AlphaError::runtime(
        "Could not locate base 'alpha' executable for bundling.",
        Span::default(),
    ))
}

fn create_bundled_exe(
    base_exe: &Path,
    output_exe: &Path,
    bundle: &crate::driver::project::AlphaProjectBundle,
) -> Result<(), AlphaError> {
    use std::io::Write;

    if output_exe.exists() {
        let _ = fs::remove_file(output_exe);
    }

    fs::copy(base_exe, output_exe).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to copy base binary {:?} to {:?}: {}", base_exe, output_exe, e),
            Span::default(),
        )
    })?;

    let payload_bytes = serde_json::to_vec(bundle).map_err(|e| {
        AlphaError::runtime(format!("Failed to serialize bundle: {}", e), Span::default())
    })?;

    let payload_len = payload_bytes.len() as u64;
    let magic = b"ALPHA_BUNDLE_V1";

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(output_exe)
        .map_err(|e| {
            AlphaError::runtime(format!("Failed to open output exe for bundling: {}", e), Span::default())
        })?;

    file.write_all(&payload_bytes).map_err(|e| {
        AlphaError::runtime(format!("Failed to write bundle payload: {}", e), Span::default())
    })?;
    file.write_all(&payload_len.to_le_bytes()).map_err(|e| {
        AlphaError::runtime(format!("Failed to write bundle length: {}", e), Span::default())
    })?;
    file.write_all(magic).map_err(|e| {
        AlphaError::runtime(format!("Failed to write bundle magic: {}", e), Span::default())
    })?;

    Ok(())
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
