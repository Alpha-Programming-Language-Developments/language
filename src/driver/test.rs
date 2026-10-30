// Test runner for Alpha (`alpha test [name]`)
// Discovers and executes #test functions in src/tests/ (and tests/)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use crate::diagnostics::error::AlphaError;
use crate::driver::project::find_project_root;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::runtime::interpreter::Interpreter;
use crate::semantics::typechecker::TypeChecker;

#[derive(Debug, Clone)]
pub struct TestCase {
    pub file_path: PathBuf,
    pub name: String,
}

pub fn run_tests(cur_dir: &Path, filter: Option<&str>) -> Result<(), AlphaError> {
    let root = find_project_root(cur_dir).unwrap_or_else(|| cur_dir.to_path_buf());

    // Search directories: src/tests (primary) and tests (secondary)
    let candidates = [
        root.join("src").join("tests"),
        root.join("tests"),
        cur_dir.join("src").join("tests"),
        cur_dir.join("tests"),
    ];

    let mut test_dirs = Vec::new();
    for cand in &candidates {
        if cand.is_dir() && !test_dirs.contains(cand) {
            test_dirs.push(cand.clone());
        }
    }

    if test_dirs.is_empty() {
        println!("[Alpha Test] No test directory found.");
        println!("Hint: Create 'src/tests/' (e.g. 'src/tests/math_test.alpha') and declare:");
        println!("\n    #test my_test() {{\n        verify 1 == 1, \"Math works!\"\n    }}\n");
        return Ok(());
    }

    // 1. Collect all .alpha files in test directories
    let mut alpha_files = Vec::new();
    for dir in &test_dirs {
        collect_alpha_files(dir, &mut alpha_files);
    }

    if alpha_files.is_empty() {
        println!("[Alpha Test] No .alpha test files found in {:?}", test_dirs);
        return Ok(());
    }

    // 2. Discover all #test functions in all files
    let mut test_cases = Vec::new();
    for file in &alpha_files {
        if let Ok(content) = fs::read_to_string(file) {
            let mut lexer = Lexer::new(&content);
            if let Ok(tokens) = lexer.tokenize() {
                let mut parser = Parser::new(tokens);
                if let Ok(program) = parser.parse_program() {
                    for func in &program.functions {
                        if func.is_test {
                            test_cases.push(TestCase {
                                file_path: file.clone(),
                                name: func.name.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    // 3. Filter by test name if provided
    let filtered_cases: Vec<&TestCase> = if let Some(target_name) = filter {
        let exact: Vec<&TestCase> = test_cases.iter().filter(|t| t.name == target_name).collect();
        if !exact.is_empty() {
            exact
        } else {
            test_cases.iter().filter(|t| t.name.contains(target_name)).collect()
        }
    } else {
        test_cases.iter().collect()
    };

    if filtered_cases.is_empty() {
        if let Some(target_name) = filter {
            println!("[Alpha Test] No test named '{}' found in {:?}", target_name, test_dirs);
            if !test_cases.is_empty() {
                println!("\nAvailable tests in project:");
                for t in &test_cases {
                    println!("  - {}", t.name);
                }
            }
        } else {
            println!("[Alpha Test] No '#test' functions found in {:?}", test_dirs);
            println!("Hint: Mark your test functions with '#test':");
            println!("\n    #test my_test() {{\n        verify 1 == 1, \"ok\"\n    }}\n");
        }
        return Ok(());
    }

    println!("[Alpha Test] Running {} test(s)...", filtered_cases.len());
    println!("------------------------------------------------------------");

    let mut passed = 0;
    let mut failed = 0;
    let total_start = Instant::now();

    for test in filtered_cases {
        let rel_path = test.file_path.strip_prefix(&root).unwrap_or(&test.file_path);
        print!("test {} [{}] ... ", test.name, rel_path.display());

        let source = match fs::read_to_string(&test.file_path) {
            Ok(s) => s,
            Err(e) => {
                println!("FAILED (failed to read file: {})", e);
                failed += 1;
                continue;
            }
        };

        // Parse program
        let mut lexer = Lexer::new(&source);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) => {
                println!("FAILED (syntax error)");
                println!("  Error: {}", e);
                failed += 1;
                continue;
            }
        };
        let mut parser = Parser::new(tokens);
        let program = match parser.parse_program() {
            Ok(p) => p,
            Err(e) => {
                println!("FAILED (parse error)");
                println!("  Error: {}", e);
                failed += 1;
                continue;
            }
        };

        // Typecheck
        let mut typechecker = TypeChecker::new()
            .with_working_dir(root.clone())
            .with_file(test.file_path.to_string_lossy().to_string());
        if let Err(e) = typechecker.check_program(&program) {
            println!("FAILED (type error)");
            println!("  Error: {}", e);
            failed += 1;
            continue;
        }

        // Run in fresh interpreter
        let mut interp = Interpreter::new().with_working_dir(root.clone());
        let test_start = Instant::now();
        match interp.run_test_function(&program, Some(&test.file_path.to_string_lossy()), &test.name) {
            Ok(_) => {
                let duration = test_start.elapsed();
                println!("OK ({}ms)", duration.as_millis().max(1));
                passed += 1;
            }
            Err(e) => {
                let duration = test_start.elapsed();
                println!("FAILED ({}ms)", duration.as_millis());
                println!("  Error: {}", e);
                failed += 1;
            }
        }
    }

    let total_duration = total_start.elapsed();
    println!("------------------------------------------------------------");
    if failed == 0 {
        println!("[Alpha Test] Result: {} passed; 0 failed (total {}ms)\n", passed, total_duration.as_millis());
        Ok(())
    } else {
        println!("[Alpha Test] Result: {} passed; {} FAILED (total {}ms)\n", passed, failed, total_duration.as_millis());
        Err(AlphaError::runtime(format!("{} test(s) failed", failed), Default::default()))
    }
}

fn collect_alpha_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                collect_alpha_files(&path, out);
            } else if path.extension().map_or(false, |ext| ext == "alpha") {
                out.push(path);
            }
        }
    }
}
