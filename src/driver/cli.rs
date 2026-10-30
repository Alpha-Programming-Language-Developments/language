// Command Line Interface (CLI) for the Alpha language.
use crate::backend::compiler::{build_file, build_project};
use crate::diagnostics::error::AlphaError;
use crate::driver::project::{find_project_root, init_project};
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::typechecker::TypeChecker;
use std::env;
use std::fs;
use crate::runtime::interpreter::Interpreter;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub fn run_cli() -> Result<(), AlphaError> {
    // 0. Check if the current binary is a standalone bundled Alpha project
    if let Ok(exe_path) = env::current_exe() {
        if let Ok(mut file) = fs::File::open(&exe_path) {
            let magic = b"ALPHA_BUNDLE_V1";
            let total_len = file.metadata().map(|m| m.len()).unwrap_or(0);
            if total_len > (magic.len() as u64 + 8) {
                let suffix_pos = total_len - (magic.len() as u64 + 8);
                if file.seek(SeekFrom::Start(suffix_pos)).is_ok() {
                    let mut len_buf = [0u8; 8];
                    let mut magic_buf = vec![0u8; magic.len()];
                    if file.read_exact(&mut len_buf).is_ok() && file.read_exact(&mut magic_buf).is_ok() {
                        if magic_buf == magic {
                            let payload_len = u64::from_le_bytes(len_buf) as usize;
                            let payload_pos = suffix_pos.saturating_sub(payload_len as u64);
                            if file.seek(SeekFrom::Start(payload_pos)).is_ok() {
                                let mut payload_data = vec![0u8; payload_len];
                                if file.read_exact(&mut payload_data).is_ok() {
                                    if let Ok(bundle) = serde_json::from_slice::<crate::driver::project::AlphaProjectBundle>(&payload_data) {
                                        let script_args = env::args().skip(1).collect::<Vec<_>>();
                                        return run_bundle(bundle, &script_args);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    match args[1].as_str() {
        "here" => {
            let name = args.get(2).cloned();
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;
            init_project(&cur_dir, name)
        }

        "build" => {
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;

            if let Some(custom) = args.get(2) {
                let p = Path::new(custom);
                if p.is_file() {
                    let _ = build_file(p)?;
                    return Ok(());
                } else if p.is_dir() {
                    if let Some(root) = find_project_root(p) {
                        let _ = build_project(&root)?;
                        return Ok(());
                    } else if p.join("src").join("main.alpha").exists() {
                        let _ = build_file(&p.join("src").join("main.alpha"))?;
                        return Ok(());
                    } else if p.join("main.alpha").exists() {
                        let _ = build_file(&p.join("main.alpha"))?;
                        return Ok(());
                    }
                }
                return Err(AlphaError::runtime(
                    format!("Target '{}' is neither an Alpha file nor a project directory.", custom),
                    Default::default(),
                )
                .with_hint("Specify an .alpha file (e.g. 'alpha build script.alpha') or a project directory"));
            }

            // No argument specified: check current directory and parents
            if let Some(project_dir) = find_project_root(&cur_dir) {
                let _ = build_project(&project_dir)?;
            } else if cur_dir.join("src").join("main.alpha").exists() {
                let _ = build_file(&cur_dir.join("src").join("main.alpha"))?;
            } else if cur_dir.join("main.alpha").exists() {
                let _ = build_file(&cur_dir.join("main.alpha"))?;
            } else {
                let alpha_files: Vec<_> = fs::read_dir(&cur_dir)
                    .map(|entries| {
                        entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| p.extension().map_or(false, |ext| ext == "alpha"))
                            .collect()
                    })
                    .unwrap_or_default();

                if alpha_files.len() == 1 {
                    let _ = build_file(&alpha_files[0])?;
                } else {
                    return Err(AlphaError::runtime(
                        "Not inside an Alpha project (no 'alpha-project.toml' or 'main.alpha' found).",
                        Default::default(),
                    )
                    .with_hint("Initialize a project with 'alpha here', or build a file directly: 'alpha build <file.alpha>'"));
                }
            }
            Ok(())
        }

        "run" => {
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;

            let (target_path, script_args) = if let Some(target) = args.get(2) {
                if target.ends_with(".alpha") {
                    (PathBuf::from(target), args[3..].to_vec())
                } else if let Some(root) = find_project_root(&cur_dir) {
                    (root.join("src").join("main.alpha"), args[2..].to_vec())
                } else if Path::new(target).exists() {
                    (PathBuf::from(target), args[3..].to_vec())
                } else {
                    let root = find_project_root(&cur_dir).ok_or_else(|| {
                        AlphaError::runtime(
                            "No file specified and not in an Alpha project directory.",
                            Default::default(),
                        )
                    })?;
                    (root.join("src").join("main.alpha"), args[2..].to_vec())
                }
            } else {
                let root = find_project_root(&cur_dir).ok_or_else(|| {
                    AlphaError::runtime(
                        "No file specified and not in an Alpha project directory.",
                        Default::default(),
                    )
                })?;
                (root.join("src").join("main.alpha"), Vec::new())
            };

            run_file(&target_path, &script_args)
        }

        "check" => {
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;

            let target_path = if let Some(target) = args.get(2) {
                PathBuf::from(target)
            } else {
                let root = find_project_root(&cur_dir).ok_or_else(|| {
                    AlphaError::runtime(
                        "No file specified and not in an Alpha project directory.",
                        Default::default(),
                    )
                })?;
                root.join("src").join("main.alpha")
            };

            check_file(&target_path)
        }

        "pkg" => {
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;
            crate::driver::pkg::handle_pkg_command(&args[2..], &cur_dir)
        }

        "test" => {
            let cur_dir = env::current_dir().map_err(|e| {
                AlphaError::runtime(format!("Failed to get current directory: {}", e), Default::default())
            })?;
            let filter = args.get(2).map(|s| s.as_str());
            crate::driver::test::run_tests(&cur_dir, filter)
        }

        "-v" | "--version" | "version" => {
            println!("Alpha version {}", crate::VERSION);
            Ok(())
        }

        "lsp" => {
            let mut server = crate::lsp::LspServer::new();
            server.run().map_err(|e| {
                AlphaError::runtime(format!("LSP server error: {}", e), Default::default())
            })
        }

        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }

        unknown => {
            // Check if user passed a file directly: `alpha script.alpha`
            let p = Path::new(unknown);
            if p.exists() && (p.extension().map_or(false, |ext| ext == "alpha") || p.is_file()) {
                run_file(p, &args[2..])
            } else {
                eprintln!("[Alpha Error] Unknown command or file: '{}'\n", unknown);
                print_help();
                std::process::exit(1);
            }
        }
    }
}

fn run_file(path: &Path, cli_args: &[String]) -> Result<(), AlphaError> {
    if !path.exists() {
        return Err(AlphaError::runtime(
            format!("File not found: {:?}", path),
            Default::default(),
        ));
    }

    // 1. Compile directly into a native executable via LLVM backend
    let exe_path = if let Some(parent) = path.parent() {
        if let Some(root) = find_project_root(parent) {
            build_project(&root)?
        } else {
            build_file(path)?
        }
    } else {
        build_file(path)?
    };

    // 2. Immediately execute the compiled binary with CLI arguments
    let status = std::process::Command::new(&exe_path)
        .args(cli_args)
        .status()
        .map_err(|e| {
            AlphaError::runtime(format!("Failed to execute compiled binary: {}", e), Default::default())
        })?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn run_bundle(bundle: crate::driver::project::AlphaProjectBundle, cli_args: &[String]) -> Result<(), AlphaError> {
    let mut interp = Interpreter::new().with_cli_args(cli_args.to_vec());
    let mut cur_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(root) = find_project_root(parent) {
                cur_dir = root;
            }
        }
    }
    interp = interp.with_working_dir(cur_dir);

    for (rel_path, content) in &bundle.files {
        interp.register_source_module(rel_path, content);
        interp.register_source_module(&format!("src/{}", rel_path), content);
    }

    match interp.eval_source(&bundle.source, Some(&bundle.entry_file)) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("{}", e.with_file(&bundle.entry_file).format_with_source(&bundle.source));
            std::process::exit(1);
        }
    }
}

fn check_file(path: &Path) -> Result<(), AlphaError> {
    if !path.exists() {
        return Err(AlphaError::runtime(
            format!("File not found: {:?}", path),
            Default::default(),
        ));
    }

    let source = fs::read_to_string(path).map_err(|e| {
        AlphaError::runtime(format!("Failed to read file {:?}: {}", path, e), Default::default())
    })?;

    let file_str = path.to_string_lossy().to_string();

    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e.with_file(&file_str).format_with_source(&source));
            std::process::exit(1);
        }
    };

    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e.with_file(&file_str).format_with_source(&source));
            std::process::exit(1);
        }
    };

    let mut typechecker = TypeChecker::new().with_file(&file_str);
    if let Some(parent) = path.parent() {
        if let Some(root) = find_project_root(parent) {
            typechecker = typechecker.with_working_dir(root);
        } else {
            typechecker = typechecker.with_working_dir(parent.to_path_buf());
        }
    }
    let all_typechecked = !program.functions.is_empty()
        && program.functions.iter().all(|f| f.is_typechecked);

    let check_res = if all_typechecked {
        typechecker.check_typechecked_only(&program)
    } else {
        typechecker.check_program(&program)
    };

    match check_res {
        Ok(_) => {
            if all_typechecked {
                println!("[Alpha Check] Syntax check passed (type checking skipped via #typechecked) for {:?}", path);
            } else {
                println!("[Alpha Check] Syntax and type check passed for {:?}", path);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", e.with_file(&file_str).format_with_source(&source));
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        r#"Alpha Programming Language Compiler & Runtime (v{})

USAGE:
    alpha <COMMAND> [OPTIONS]
    alpha <FILE.alpha>

COMMANDS:
    here [name]     Initialize a new Alpha project in the current directory
    pkg [url]       Manage packages (e.g. 'alpha pkg <github_url>')
    test [name]     Discover and run '#test' functions in src/tests/
    build [file]    Compile into a native binary via direct LLVM Backend (-O3)
    run [file]      Compile and execute with native hardware speed
    check [file]    Verify syntax and perform static type checking without running
    help            Print this help message

OPTIONS:
    -v, --version   Print Alpha version
    -h, --help      Print this help message

EXAMPLES:
    alpha here my_project
    alpha run src/main.alpha
    alpha build src/main.alpha
    alpha check
    alpha -v
"#,
        crate::VERSION
    );
}
