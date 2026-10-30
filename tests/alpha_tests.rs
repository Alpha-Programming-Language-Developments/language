use alpha_lang::interpreter::Interpreter;
use alpha_lang::lexer::Lexer;
use alpha_lang::parser::Parser;
use alpha_lang::typechecker::TypeChecker;
use alpha_lang::value::Value;

#[test]
fn test_syntax_example_program() {
    let source = r#"
import Core/Basic.alpha

var players: vect<string> = vect.new()
var scores: map<string, int> = map.new()

function add_player(name: string) [public, nothing] {
  players.push(name)
  scores.insert(name, 0)
}

function get_score(name: string) [public, <int, nothing>] {
  if scores.contains(name) {
    return scores.get(name)
  } else {
    return nothing
  }
}

function main() [public, nothing] {
  add_player("Player1")
  add_player("Player2")

  for each player in players {
    print(player)
  }
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type checking failed");

    let mut interp = Interpreter::new();
    let res = interp.execute_program(&program, None).expect("Execution failed");
    assert_eq!(res, Value::Nothing);
}

#[test]
fn test_count_for_loop() {
    let source = r#"
var sum: int = 0
for i = 0, i < 5, i++ {
    sum = sum + i
}
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    let sum_val = interp.env.get("sum").expect("sum variable should exist");
    assert_eq!(sum_val, Value::Int(10)); // 0+1+2+3+4 = 10
}

#[test]
fn test_format_strings() {
    let source = r#"
import Core/Basic.alpha
var time: string = format("{2}:{2}:{2}", 2, 30, 10)
var greeting: string = format("Hello, {}!", "Alpha")
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("time"), Some(Value::String("02:30:10".to_string())));
    assert_eq!(interp.env.get("greeting"), Some(Value::String("Hello, Alpha!".to_string())));
}

#[test]
fn test_nothing_checking() {
    let source = r#"
var empty_val: nothing = nothing
var is_none: boolean = empty_val.nothing?()
var num: int = 42
var not_none: boolean = num.nothing?()
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("is_none"), Some(Value::Boolean(true)));
    assert_eq!(interp.env.get("not_none"), Some(Value::Boolean(false)));
}

#[test]
fn test_conditions_and_not() {
    let source = r#"
var condition: boolean = false
var branch: string = "none"

if not condition {
    branch = "not branch"
} elseif condition {
    branch = "elif branch"
} else {
    branch = "else branch"
}
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("branch"), Some(Value::String("not branch".to_string())));
}

#[test]
fn test_function_with_question_mark() {
    let source = r#"
function is_valid?() [public, boolean] {
    return true
}

var result: boolean = is_valid?()
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("result"), Some(Value::Boolean(true)));
}

#[test]
fn test_question_mark_anywhere_in_functions_and_identifiers() {
    let source = r#"
function is?prime(n: int) [public, boolean] {
    return n > 1
}

function ?starts_with_question(x: int) [public, int] {
    return x * 2
}

function check???(msg: string) [public, string] {
    return msg + "!!!"
}

function ?all?around?(a: int, b: int) [public, int] {
    return a + b
}

function ??calc??(v: int) [public, int] {
    return v + 10
}

function ???() [public, int] {
    return 100
}

function ?(x: int) [public, int] {
    return x + 1
}

function something() [public, string] {
    return "runned"
}

structure Checker {
    val?: int,
    attach {
        function ?is?positive?() [nonstatic, here, boolean] {
            return this.val? > 0
        }
    }
}

var r1: boolean = is?prime(7)
var r2: int = ?starts_with_question(5)
var r3: string = check???("alpha")
var r4: int = ?all?around?(10, 20)
var r5: int = ??calc??(32)

var c: Checker = make Checker{val?: 42}
var r6: boolean = c.?is?positive?()

var ?val?: <int, nothing> = 99
var r7: int = ?val? !?
var r8: int = ???()
var r9: int = ?(41)
var r10: string = somet?hing()
var r11: string = some?thing?()
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("r1"), Some(Value::Boolean(true)));
    assert_eq!(interp.env.get("r2"), Some(Value::Int(10)));
    assert_eq!(interp.env.get("r3"), Some(Value::String("alpha!!!".to_string())));
    assert_eq!(interp.env.get("r4"), Some(Value::Int(30)));
    assert_eq!(interp.env.get("r5"), Some(Value::Int(42)));
    assert_eq!(interp.env.get("r6"), Some(Value::Boolean(true)));
    assert_eq!(interp.env.get("r7"), Some(Value::Int(99)));
    assert_eq!(interp.env.get("r8"), Some(Value::Int(100)));
    assert_eq!(interp.env.get("r9"), Some(Value::Int(42)));
    assert_eq!(interp.env.get("r10"), Some(Value::String("runned".to_string())));
    assert_eq!(interp.env.get("r11"), Some(Value::String("runned".to_string())));
}

#[test]
fn test_system_conditional_execution() {
    let source = r#"
!system win {
    function get_os_name() [public, string] {
        return "windows"
    }
}

!system posix {
    function get_os_name() [public, string] {
        return "posix"
    }
}

function get_slash() [public, string] {
    var slash: string = ""
    !system win {
        slash = "\\"
    }
    !system posix {
        slash = "/"
    }
    return slash
}

function get_direct_return() [public, string] {
    !system win {
        return "win_direct"
    }
    !system posix {
        return "posix_direct"
    }
}

var os_res: string = get_os_name()
var slash_res: string = get_slash()
var direct_res: string = get_direct_return()
"#;

    // By default on Windows host:
    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed on host");
    if cfg!(windows) {
        assert_eq!(interp.env.get("os_res"), Some(Value::String("windows".to_string())));
        assert_eq!(interp.env.get("slash_res"), Some(Value::String("\\".to_string())));
        assert_eq!(interp.env.get("direct_res"), Some(Value::String("win_direct".to_string())));
    } else {
        assert_eq!(interp.env.get("os_res"), Some(Value::String("posix".to_string())));
        assert_eq!(interp.env.get("slash_res"), Some(Value::String("/".to_string())));
        assert_eq!(interp.env.get("direct_res"), Some(Value::String("posix_direct".to_string())));
    }

    // Now test with overridden target OS:
    unsafe {
        std::env::set_var("ALPHA_TARGET_OS", "posix");
    }
    let mut interp_posix = Interpreter::new();
    interp_posix.eval_source(source, None).expect("Evaluation failed for posix target");
    assert_eq!(interp_posix.env.get("os_res"), Some(Value::String("posix".to_string())));
    assert_eq!(interp_posix.env.get("slash_res"), Some(Value::String("/".to_string())));
    assert_eq!(interp_posix.env.get("direct_res"), Some(Value::String("posix_direct".to_string())));

    // Clean up env var
    unsafe {
        std::env::remove_var("ALPHA_TARGET_OS");
    }
}

#[test]
fn test_const_reassignment_fails() {
    let source = r#"
const var max_speed: int = 100
max_speed = 200
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    let err = typechecker.check_program(&program);
    assert!(err.is_err(), "Expected type checker to reject reassigning const var");
}

#[test]
fn test_map_and_vect_operations() {
    let source = r#"
var v: vect<int> = vect.new()
v.push(10)
v.push(20)
v.push(30)
var v_len: int = v.len()

var m: map<string, int> = map.new()
m.insert("A", 1)
m.insert("B", 2)
var has_a: boolean = m.contains("A")
var val_a: int = m.get("A")
var has_c: boolean = m.contains("C")
var val_c: nothing = m.get("C")
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");
    assert_eq!(interp.env.get("v_len"), Some(Value::Int(3)));
    assert_eq!(interp.env.get("has_a"), Some(Value::Boolean(true)));
    assert_eq!(interp.env.get("val_a"), Some(Value::Int(1)));
    assert_eq!(interp.env.get("has_c"), Some(Value::Boolean(false)));
    assert_eq!(interp.env.get("val_c"), Some(Value::Nothing));
}

#[test]
fn test_io_file_operations() {
    let test_dir = std::env::temp_dir().join("alpha_test_io");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let file_path = test_dir.join("test.txt");

    let source = format!(
        r#"
import Core/Io.alpha
var file: Io.File = Io.open("{path}")
file.reset()
file.write("line 1\nline 2")
var exists: boolean = file.something?()
var empty: boolean = file.empty?()
var lines: int = file.linelen()
var content: string = file.content()
"#,
        path = file_path.to_string_lossy().replace('\\', "/")
    );

    let mut interp = Interpreter::new();
    interp.eval_source(&source, None).expect("Evaluation failed");

    assert_eq!(interp.env.get("exists"), Some(Value::Boolean(true)));
    assert_eq!(interp.env.get("empty"), Some(Value::Boolean(false)));
    assert_eq!(interp.env.get("lines"), Some(Value::Int(2)));
    assert_eq!(interp.env.get("content"), Some(Value::String("line 1\nline 2".to_string())));

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_fs_dir_and_env_operations() {
    let test_dir = std::env::temp_dir().join("alpha_test_fs");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();

    let source = format!(
        r#"
import Core/Fs.alpha
import Core/Env.alpha
var d: Fs.Dir = dir.open("{path}")
d.create("subfolder")
d.file.create("sample.txt")
var exists: boolean = d.nothing?("subfolder")
var children: vect<string> = d.get_children(".")
var child_count: int = children.len()
var curr_dir: string = env.current
"#,
        path = test_dir.to_string_lossy().replace('\\', "/")
    );

    let mut interp = Interpreter::new();
    interp.eval_source(&source, None).expect("Evaluation failed");

    assert_eq!(interp.env.get("exists"), Some(Value::Boolean(false))); // not nothing -> exists
    assert_eq!(interp.env.get("child_count"), Some(Value::Int(2)));
    assert!(interp.env.get("curr_dir").is_some());

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_project_init_and_build() {
    let test_proj_dir = std::env::temp_dir().join("alpha_test_build_proj");
    let _ = std::fs::remove_dir_all(&test_proj_dir);
    std::fs::create_dir_all(&test_proj_dir).unwrap();

    // 1. Initialize project
    alpha_lang::project::init_project(&test_proj_dir, Some("sample_app".to_string()))
        .expect("Project init failed");

    // Write some code in main.alpha
    let main_alpha = test_proj_dir.join("src").join("main.alpha");
    let code = r#"
import Core/Basic.alpha

function main() [public, nothing] {
    var greeting: string = format("Hello from compiled {}", "Alpha")
    print(greeting)
}
"#;
    std::fs::write(&main_alpha, code).unwrap();

    // 2. Build project to executable
    let binary_path = alpha_lang::compiler::build_project(&test_proj_dir)
        .expect("Project build failed");

    assert!(binary_path.exists(), "Binary executable must exist in build/");

    // 3. Run the compiled executable and verify output!
    let output = std::process::Command::new(&binary_path)
        .output()
        .expect("Failed to run compiled binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from compiled Alpha"));

    let _ = std::fs::remove_dir_all(&test_proj_dir);
}

#[test]
fn test_try_catch_runtime_error() {
    let source = r#"
import Core/Basic.alpha
var caught: string = "none"

try {
    error("something went wrong")
} catch err {
    caught = err
}
"#;

    let mut interp = Interpreter::new();
    interp.eval_source(source, None).expect("Evaluation failed");

    assert_eq!(
        interp.env.get("caught"),
        Some(Value::String("something went wrong".to_string()))
    );
}

#[test]
fn test_compile_error_formatting() {
    let bad_source = "var x: int = \"not an int\"";

    let mut lexer = Lexer::new(bad_source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    let err = typechecker.check_program(&program).expect_err("Must fail typecheck");

    let formatted = err.format_with_source(bad_source);
    assert!(formatted.starts_with("Compile Error (Type):"));
    assert!(formatted.contains("-->"));
    assert!(formatted.contains("Hint:"));
}

#[test]
fn test_custom_core_module_creation_and_visibility() {
    let test_proj_dir = std::env::temp_dir().join("alpha_test_custom_core");
    let _ = std::fs::remove_dir_all(&test_proj_dir);
    std::fs::create_dir_all(&test_proj_dir).unwrap();

    // 1. Initialize project
    alpha_lang::project::init_project(&test_proj_dir, Some("math_proj".to_string()))
        .expect("Project init failed");

    // 2. Create custom module: src/Core/Math.alpha
    let core_dir = test_proj_dir.join("src").join("Core");
    std::fs::create_dir_all(&core_dir).unwrap();
    let math_alpha = core_dir.join("Math.alpha");
    let math_code = r#"
function square(n: int) [public, int] {
    return n * n
}

function private_calc() [here, int] {
    return 777
}
"#;
    std::fs::write(&math_alpha, math_code).unwrap();

    // 3. Create src/main.alpha importing Core/Math.alpha
    let main_alpha = test_proj_dir.join("src").join("main.alpha");
    let main_code = r#"
import Core/Basic.alpha
import Core/Math.alpha

function main() [public, nothing] {
    var sq: int = square(6)
    var msg: string = format("Square of 6 is {}", sq)
    print(msg)
}
"#;
    std::fs::write(&main_alpha, main_code).unwrap();

    // 4. Test type checking
    let mut lexer = Lexer::new(main_code);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new()
        .with_file(main_alpha.to_string_lossy())
        .with_working_dir(test_proj_dir.clone());
    typechecker.check_program(&program).expect("Type check failed for custom Core module");

    // 5. Test running with Interpreter
    let mut interp = Interpreter::new().with_working_dir(test_proj_dir.clone());
    interp.eval_source(main_code, Some(&main_alpha.to_string_lossy())).expect("Eval failed");

    // 6. Test compiling to native binary
    let binary_path = alpha_lang::compiler::build_project(&test_proj_dir)
        .expect("Project build failed");

    let output = std::process::Command::new(&binary_path)
        .output()
        .expect("Failed to execute native binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Square of 6 is 36"));

    // 7. Test visibility violation: calling private 'here' function from outside
    let bad_main_code = r#"
import Core/Math.alpha

function main() [public, nothing] {
    var p: int = private_calc()
}
"#;
    let mut bad_lexer = Lexer::new(bad_main_code);
    let bad_tokens = bad_lexer.tokenize().expect("Lexing failed");
    let mut bad_parser = Parser::new(bad_tokens);
    let bad_program = bad_parser.parse_program().expect("Parsing failed");

    let mut bad_typechecker = TypeChecker::new()
        .with_file(main_alpha.to_string_lossy())
        .with_working_dir(test_proj_dir.clone());
    let err = bad_typechecker.check_program(&bad_program).expect_err("Must reject private 'here' call");
    assert!(err.message.contains("private"));

    let _ = std::fs::remove_dir_all(&test_proj_dir);
}

#[test]
fn test_alpha_programs_showcase() {
    let programs = vec![
        "tests/alpha_programs/01_math_and_primes.alpha",
        "tests/alpha_programs/02_game_inventory.alpha",
        "tests/alpha_programs/03_file_and_system.alpha",
        "tests/alpha_programs/04_error_resilience.alpha",
        "tests/alpha_programs/05_task_manager.alpha",
        "tests/alpha_programs/06_matrix_and_algorithms.alpha",
    ];

    for prog_path in programs {
        let code = std::fs::read_to_string(prog_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", prog_path, e));

        let mut lexer = Lexer::new(&code);
        let tokens = lexer.tokenize().unwrap_or_else(|e| panic!("Lexing {} failed: {}", prog_path, e));
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap_or_else(|e| panic!("Parsing {} failed: {}", prog_path, e));

        let mut typechecker = TypeChecker::new().with_file(prog_path);
        typechecker.check_program(&program).unwrap_or_else(|e| panic!("Type checking {} failed: {}", prog_path, e));

        let mut interp = Interpreter::new();
        interp.eval_source(&code, Some(prog_path)).unwrap_or_else(|e| panic!("Execution of {} failed: {}", prog_path, e));
    }
}

#[test]
fn test_structures_and_methods() {
    let source = r#"
structure Player {
    name: string,
    health: int,
attach {
    function new(name: string) [static, public, Player] {
        return make this{name: name, health: 100}
    }

    function take_damage(dmg: int) [nonstatic, public, nothing] {
        this.health = this.health - dmg
    }

    function get_health() [nonstatic, public, int] {
        return this.health
    }
}
}

function main() [public, int] {
    var player: Player = Player.new("Hero")
    player.take_damage(25)
    return player.get_health()
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(75));
}

#[test]
fn test_check_pattern_matching() {
    let source = r#"
function find_item(id: int) [public, <string, nothing>] {
    if id == 1 {
        return "Sword"
    } else {
        return nothing
    }
}

function main() [public, int] {
    var found_name: string = check find_item(1) {
        something => item,
        nothing => "Unknown"
    }

    var missing_name: string = check find_item(2) {
        something => item,
        nothing => "Unknown"
    }

    var status: int = 0
    check found_name {
        (found_name) => { status = 1 },
        else => { status = -1 }
    }

    if found_name == "Sword" && missing_name == "Unknown" && status == 1 {
        return 42
    } else {
        return 0
    }
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(42));
}

#[test]
fn test_force_unwrap_and_elvis() {
    let source = r#"
var scores: map<string, int> = map.new()
scores.insert("Player1", 99)

function main() [public, int] {
    var box: <int, nothing> = scores.get("Player1")
    var value: something = box!?

    var fallback_val: int = scores.get("Player2") or 10

    return (value -> int) + fallback_val
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(109)); // 99 + 10
}

#[test]
fn test_force_unwrap_fails_on_nothing() {
    let source = r#"
var scores: map<string, int> = map.new()

function main() [public, nothing] {
    var box: <int, nothing> = scores.get("MissingPlayer")
    var value: something = box!?
}
"#;

    let mut interp = Interpreter::new();
    let err = interp.eval_source(source, None).expect_err("Should error on unwrap of nothing");
    assert!(err.message.contains("unwrap"));
}

#[test]
fn test_range_for_loop() {
    let source = r#"
function main() [public, int] {
    var sum1: int = 0
    for n in 1...3 {
        sum1 = sum1 + n
    }

    var sum2: int = 0
    for n in 1..3 {
        sum2 = sum2 + n
    }

    // sum1 is 1+2+3 = 6; sum2 is 1+2 = 3
    return sum1 * 10 + sum2
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(63));
}

#[test]
fn test_logical_operators_and_grouping() {
    let source = r#"
function main() [public, int] {
    var a: int = 10
    var b: int = 10
    var c: int = 20

    var cond1: boolean = a == b && b == a || c == a
    var cond2: boolean = !(a == c)

    var math_res: int = (2 + 3) * 4

    if cond1 && cond2 && math_res == 20 {
        return 1
    } else {
        return 0
    }
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(1));
}

#[test]
fn test_string_features_and_casting() {
    let source = r#"
function get_num_str() [public, string] {
    return "42"
}

function main() [public, int] {
    var result: string = get_num_str()
    var a: int = (result -> int)

    var ternary_res: int = if a > 40: 100 else 0
    var ternary_str: string = if a > 40: "above" else "below"

    var s: string = "hello"
    s += " world"

    var is_emp: boolean = "".empty?()
    var str_len: int = s.len() // "hello world".len() == 11

    var name: string = "Alpha"
    var greeting: string = $"Welcome to {name} language!"

    if a == 42 && ternary_res == 100 && ternary_str == "above" && s == "hello world" && is_emp && str_len == 11 && greeting == "Welcome to Alpha language!" {
        return 777
    } else {
        return 0
    }
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Int(777));
}

#[test]
fn test_core_machine_console_out() {
    let source = r#"
import Core/Machine.alpha

function main() [public, string] {
    var out: string = console.out("echo alpha_machine_test")
    return out
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::String("alpha_machine_test".to_string()));
}

#[test]
fn test_core_random_and_global_random() {
    let source = r#"
import Core/Random.alpha

function main() [public, boolean] {
    random.seed(12345)

    var n: int = random.int(10, 20)
    var n_max: int = random.int(5)
    var f: float = random.float()
    var f_range: float = random.float(10.0, 20.0)
    var b: boolean = random.bool()

    var fruits: vect<string> = vect.new()
    fruits.push("apple")
    fruits.push("banana")
    fruits.push("cherry")
    var pick: string = (random.choice(fruits) -> string)

    var token: string = random.string(12)
    var rand_call: int = random(1, 10)

    if n >= 10 && n <= 20 && n_max >= 0 && n_max <= 5 && f >= 0.0 && f <= 1.0 && f_range >= 10.0 && f_range <= 20.0 && token.len() == 12 && rand_call >= 1 && rand_call <= 10 && (pick == "apple" || pick == "banana" || pick == "cherry") {
        return true
    } else {
        return false
    }
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_tuples_and_unpacking() {
    let source = r#"
function get_pair() [public, (int, string)] {
    return (42, "answer")
}

function get_triple() [public, (int, string, boolean)] {
    return (10, "alpha", true)
}

function main() [public, boolean] {
    // 1. Literal and indexing
    var t: (int, string, boolean) = get_triple()
    var v1: int = (t[1] -> int)
    var v2: string = (t[2] -> string)
    var v3: boolean = (t[3] -> boolean)

    // 2. Unpacking without types
    var (num, text) = get_pair()

    // 3. Unpacking with explicit element types
    var (x: int, y: string) = (100, "hello")

    // 4. Tuple assignment
    (num, text) = (77, "updated")

    if v1 == 10 && v2 == "alpha" && v3 == true && num == 77 && text == "updated" && x == 100 && y == "hello" {
        return true
    } else {
        return false
    }
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_slices_string_and_vector() {
    let source = r#"
function main() [public, boolean] {
    // String slices
    var s: string = "hello world"
    var sub1: string = s[1..6]      // "hello"
    var sub2: string = s[7...11]    // "world" (inclusive)
    var sub3: string = s[7..]       // "world"
    var sub4: string = s[..6]       // "hello"

    // Vector slices
    var v: vect<int> = vect.new()
    v.push(10)
    v.push(20)
    v.push(30)
    v.push(40)
    v.push(50)

    var v_slice1: vect<int> = v[2..4]    // [20, 30]
    var v_slice2: vect<int> = v[4...]    // [40, 50]
    var v_slice3: vect<int> = v[..3]     // [10, 20]

    var ok_str: boolean = sub1 == "hello" && sub2 == "world" && sub3 == "world" && sub4 == "hello"
    var ok_vec: boolean = v_slice1.len() == 2 && v_slice1[1] == 20 && v_slice1[2] == 30 && v_slice2.len() == 2 && v_slice2[1] == 40 && v_slice3.len() == 2 && v_slice3[1] == 10 && v_slice3[2] == 20

    if ok_str && ok_vec {
        return true
    } else {
        return false
    }
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_core_time() {
    let source = r#"
import Core/Time.alpha

function main() [public, boolean] {
    var sec: int = time.now()
    var start_ms: int = time.now_ms()
    time.sleep(15)
    var elapsed: int = time.elapsed(start_ms)

    if sec > 0 && start_ms > 0 && elapsed >= 10 {
        return true
    } else {
        return false
    }
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_core_json() {
    let source = r#"
import Core/Json.alpha

function main() [public, boolean] {
    var json_str: string = "{\"name\": \"Player1\", \"score\": 100, \"active\": true, \"items\": [1, 2, 3]}"
    var data: something = json.parse(json_str)

    var name: string = (data.get("name") -> string)
    var score: int = (data.get("score") -> int)
    var active: boolean = (data.get("active") -> boolean)
    var items: vect<int> = (data.get("items") -> vect<int>)
    var first_item: int = (items[1] -> int)

    // Serialization test
    var out_json: string = json.stringify(data)

    if name == "Player1" && score == 100 && active == true && first_item == 1 && out_json.len() > 10 {
        return true
    } else {
        return false
    }
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_enhanced_core_fs() {
    let temp_dir = std::env::temp_dir().join(format!("alpha_fs_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let temp_dir_str = temp_dir.to_string_lossy().replace('\\', "/");

    let source = format!(r#"
import Core/Fs.alpha
import Core/Io.alpha

function main() [public, boolean] {{
    var base: string = "{temp_dir_str}"
    var test_file: string = Fs.join(base, "docs", "hello.txt")

    // Check directory and path functions with Alpha ? style
    var ext: string = Fs.extension(test_file)
    var fname: string = Fs.filename(test_file)
    var fstem: string = Fs.stem(test_file)

    Io.write(test_file, "Alpha Language Core Fs!")
    var exists_now: boolean = Fs.exists?(test_file)
    var is_file: boolean = Fs.file?(test_file)
    var is_dir: boolean = Fs.dir?(Fs.join(base, "docs"))
    var content: string = Io.read(test_file)
    var size: int = Io.size(test_file)

    // Copy file via Io
    var copy_dest: string = Fs.join(base, "docs", "hello_copy.txt")
    Io.copy(test_file, copy_dest)
    var copy_exists: boolean = Io.exists?(copy_dest)

    // Move file via Io
    var moved_dest: string = Fs.join(base, "docs", "hello_moved.txt")
    Io.move(copy_dest, moved_dest)
    var moved_exists: boolean = Io.exists?(moved_dest)
    var old_copy_gone: boolean = !Io.exists?(copy_dest)

    // Test dir object methods with exists?
    var docs_dir: Fs.Dir = Fs.open(Fs.join(base, "docs"))
    var files: vect<string> = (docs_dir.files() -> vect<string>)
    var file_count: int = files.len()
    var has_hello: boolean = docs_dir.exists?("hello.txt")

    if ext == "txt" && fname == "hello.txt" && fstem == "hello" && exists_now && is_file && is_dir && content == "Alpha Language Core Fs!" && size > 0 && copy_exists && moved_exists && old_copy_gone && file_count == 2 && has_hello {{
        return true
    }} else {{
        return false
    }}
}}
"#, temp_dir_str = temp_dir_str);

    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(&source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_core_http() {
    let source = r#"
import Core/Http.alpha

function main() [public, boolean] {
    // Testing http namespace methods availability
    var check_http: boolean = http.something?()
    return check_http
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_defer_lifo_execution() {
    let source = r#"
var trace: string = ""

function push_trace(s: string) [public, nothing] {
    trace = trace + s
}

function run_deferred() [public, int] {
    push_trace("start;")
    make schedule push_trace("first_defer;")
    make schedule {
        push_trace("second_defer_block;")
    }
    make schedule push_trace("third_defer;")

    push_trace("middle;")
    return 42
}

function main() [public, boolean] {
    var result: int = run_deferred()
    // Defers must execute in reverse (LIFO): third -> second -> first
    var expected: string = "start;middle;third_defer;second_defer_block;first_defer;"
    return result == 42 && trace == expected
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_make_schedule_syntax() {
    let source = r#"
var trace: string = ""

function push_trace(s: string) [public, nothing] {
    trace = trace + s
}

function cleanup_step() [public, nothing] {
    push_trace("func_call;")
}

function run_scheduled() [public, int] {
    push_trace("init;")
    
    // 1. make schedule (function call)
    make schedule (push_trace("step1;"))
    
    // 2. make schedule with block
    make schedule {
        push_trace("block_step;")
    }
    
    // 3. make schedule with direct function name without parens
    make schedule (cleanup_step)

    push_trace("work;")
    return 100
}

function main() [public, boolean] {
    var r: int = run_scheduled()
    // LIFO execution on exit: cleanup_step -> block_step -> step1
    var expected: string = "init;work;func_call;block_step;step1;"
    return r == 100 && trace == expected
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_verify_assertion() {
    // 1. Success case
    let source_ok = r#"
function main() [public, boolean] {
    var a: int = 10
    verify a == 10, "a must equal 10"
    verify a > 0, "a must be positive"
    verify true
    return true
}
"#;
    let mut interp = Interpreter::new();
    let res = interp.eval_source(source_ok, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));

    // 2. Failure case
    let source_fail = r#"
function main() [public, nothing] {
    var score: int = 42
    verify score == 100, "Score must be exactly 100!"
}
"#;
    let mut interp_fail = Interpreter::new();
    let err = interp_fail.eval_source(source_fail, None).expect_err("Must fail assertion");
    assert!(err.message.contains("Score must be exactly 100!"));

    // 3. Catching assertion failure in try-catch
    let source_catch = r#"
function main() [public, string] {
    var caught: string = ""
    try {
        verify 1 + 1 == 3, "Math broken"
    } catch err {
        caught = err
    }
    return caught
}
"#;
    let mut interp_catch = Interpreter::new();
    let res_catch = interp_catch.eval_source(source_catch, None).expect("Execution failed");
    if let Value::String(msg) = res_catch {
        assert!(msg.contains("Math broken"));
    } else {
        panic!("Expected string caught message");
    }
}

#[test]
fn test_dir_walk_for_in() {
    let temp_dir = std::env::temp_dir().join("alpha_test_walk");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("sub1")).expect("Failed to create dir");
    std::fs::create_dir_all(temp_dir.join("sub2")).expect("Failed to create dir");
    std::fs::write(temp_dir.join("sub1").join("a.txt"), "hello").expect("write a");
    std::fs::write(temp_dir.join("sub2").join("b.txt"), "world").expect("write b");

    let temp_dir_str = temp_dir.to_str().unwrap().replace('\\', "/");

    let source = format!(r#"
import Core/Fs.alpha

function main() [public, int] {{
    var d: Fs.Dir = Fs.open("{temp_dir_str}")
    var count: int = 0
    for item in d.walk() {{
        count = count + 1
    }}
    return count
}}
"#, temp_dir_str = temp_dir_str);

    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(&source, None).expect("Execution failed");
    // walk() yields sub1, sub1/a.txt, sub2, sub2/b.txt (at least 4 items)
    if let Value::Int(c) = res {
        assert_eq!(c, 4);
    } else {
        panic!("Expected int count, got {:?}", res);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_io_file_and_static_methods() {
    let temp_dir = std::env::temp_dir().join("alpha_test_io_methods");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("Failed to create dir");

    let temp_dir_str = temp_dir.to_str().unwrap().replace('\\', "/");

    let source = format!(r#"
import Core/Fs.alpha
import Core/Io.alpha

function main() [public, boolean] {{
    var path: string = Fs.join("{temp_dir_str}", "test_io.txt")

    // Static Io write & read
    Io.write(path, "Hello Alpha!")
    var exists: boolean = Io.exists?(path)
    var is_file: boolean = Io.file?(path)
    var content: string = Io.read(path)
    var sz: int = Io.size(path)

    // Handle-based Io.File
    var f: Io.File = Io.open(path)
    var f_exists: boolean = f.exists?()
    var f_content: string = f.content()
    var f_size: int = f.size()

    // Delete
    Io.delete(path)
    var gone: boolean = !Io.exists?(path)

    return exists && is_file && content == "Hello Alpha!" && sz > 0 && f_exists && f_content == "Hello Alpha!" && f_size > 0 && gone
}}
"#, temp_dir_str = temp_dir_str);

    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(&source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));

    let _ = std::fs::remove_dir_all(&temp_dir);
}







#[test]
fn test_enum_feature() {
    let source = r#"
enum Status {
    Pending,
    Active,
    Done
}

function main() [public, boolean] {
    var s1: Status = Status.Pending
    var s2: Status = Status.Active
    var s3: Status = Status.Done

    var is_pending: boolean = s1 == Status.Pending
    var not_done: boolean = s2 != Status.Done

    return is_pending && not_done && (s3 == Status.Done)
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_hexcolor_and_cli_paint() {
    let source = r#"
import Core/Cli.alpha

function main() [public, boolean] {
    var color: hexcolor = #(FFF00)
    var bg: hexcolor = #(00AAFF)

    var painted1: string = paint("Alpha Text", color)
    var painted2: string = cli.paint("CLI Painted", bg)

    return painted1.len() > 10 && painted2.len() > 10
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_1_based_indexing_vector_and_map() {
    let source = r#"
function main() [public, boolean] {
    // Vector 1-based indexing
    var v: vect<string> = vect.new()
    v.push("first")
    v.push("second")
    v.push("third")

    var elem1: string = v[1]
    var elem2: string = v[2]
    var elem3: string = v[3]

    // Vector assignment at 1-based index
    v[2] = "updated_second"
    var elem2_new: string = v[2]

    // Map 1-based keys
    var m: map<int, string> = map.new()
    m.insert(1, "gold")
    m.insert(2, "silver")

    var val1: string = m[1]
    var val2: string = m[2]

    return elem1 == "first" && elem2 == "second" && elem3 == "third" && elem2_new == "updated_second" && val1 == "gold" && val2 == "silver"
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_module_namespace() {
    let source = r#"
module MathOps {
    function add(a: int, b: int) [public, int] {
        return a + b
    }

    function square(n: int) [public, int] {
        return n * n
    }
}

function main() [public, boolean] {
    var sum: int = MathOps.add(10, 20)
    var sq: int = MathOps.square(5)

    return sum == 30 && sq == 25
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type check failed");

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_cli_arguments() {
    let source = r#"
import Core/Cli.alpha

function main() [public, boolean] {
    var arguments: vect<string> = cli.arguments()
    return arguments.len() == 2 && arguments[1] == "--verbose" && arguments[2] == "config.json"
}
"#;

    let mut interp = Interpreter::new().with_cli_args(vec!["--verbose".to_string(), "config.json".to_string()]);
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_direct_web_import_prohibited() {
    let source = r#"
import web::"https://example.com/mod.alpha"

function main() [public, nothing] {}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let err = parser.parse_program().unwrap_err();
    assert!(err.to_string().contains("packages.alpaks"));
}

#[test]
fn test_packages_alpaks_formats_and_github_normalization() {
    let content = r#"
# Standard web:: syntax
"webalph_raw" = import web::"https://raw.githubusercontent.com/Alpha-Programming-Language-Developments/webalph/main/src/lib.alpha"

# Direct URL without web:: and with GitHub blob URL
"webalph" = import "https://github.com/Alpha-Programming-Language-Developments/webalph/blob/main/src/lib.alpha"
"#;

    let map = alpha_lang::runtime::stdlib::parse_packages_alpaks(content);
    assert_eq!(
        map.get("webalph_raw"),
        Some(&"https://raw.githubusercontent.com/Alpha-Programming-Language-Developments/webalph/main/src/lib.alpha".to_string())
    );
    assert_eq!(
        map.get("webalph"),
        Some(&"https://raw.githubusercontent.com/Alpha-Programming-Language-Developments/webalph/main/src/lib.alpha".to_string())
    );
}

#[test]
fn test_repeat_range_loop_without_variable() {
    let source = r#"
var counter: int = 0
for 1...3 {
    counter = counter + 1
}

var non_inc: int = 0
for 1..3 {
    non_inc = non_inc + 1
}

function main() [public, boolean] {
    return counter == 3 && non_inc == 2
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_time_benchmark() {
    let source = r#"
import Core/Time.alpha

function main() [public, boolean] {
    var count: int = 0
    var ms: int = time.benchmark({
        time.sleep(20)
        count = count + 10
    })

    var ms_capital: int = Time.benchmark({
        time.sleep(15)
        count = count + 5
    })

    var ms_global: int = benchmark({
        time.sleep(10)
        count = count + 1
    })

    return ms >= 15 && ms_capital >= 10 && ms_global >= 5 && count == 16
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}


#[test]
fn test_unt_type_and_operations() {
    let source = r#"
function main() [public, boolean] {
    var a: unt = 100
    var b: unt = 25
    var sum: unt = a + b
    var diff: unt = a - b
    var prod: unt = a * 2
    var quot: unt = a / b
    var rem: unt = a % 30

    var is_greater: boolean = a > b
    var is_equal: boolean = (quot == 4)

    return sum == 125 && diff == 75 && prod == 200 && quot == 4 && rem == 10 && is_greater && is_equal
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_unt_negative_assignment_fails() {
    let source = r#"
function main() [public, nothing] {
    var a: unt = -5
}
"#;

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().message;
    assert!(err_msg.contains("Cannot assign negative value") || err_msg.contains("unt"));
}

#[test]
fn test_strict_mode_valid_program() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
@strict

function safe_divide(a: int, b: int) [public, int] {
    if b == 0 {
        return 0
    }
    return a / b
}

function main() [public, boolean] {
    var a: int = 10
    var b: int = 2
    var res: int = safe_divide(a, b)
    var success: boolean = check res {
        (5) => true,
        else => false
    }
    return success
}
"#;

    let errors = validate_alpha_code(source);
    assert!(errors.is_empty(), "Expected no validation errors, got: {:?}", errors);

    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution failed");
    assert_eq!(res, Value::Boolean(true));
}

#[test]
fn test_strict_mode_forbids_force_unwrap() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
@strict

function main() [public, int] {
    var res: <int, string> = 42
    var val: int = res!?
    return val
}
"#;

    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected strict mode error for force unwrap");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("Force unwrap"));
}

#[test]
fn test_strict_mode_forbids_untyped_variable() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
@strict

function main() [public, int] {
    var x = 10
    return x
}
"#;

    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected strict mode error for untyped variable");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("explicit type annotation"));
}

#[test]
fn test_strict_mode_forbids_something_and_nothing() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
@strict

function main() [public, int] {
    var x: something = 10
    return 0
}
"#;

    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected strict mode error for 'something'");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("forbidden"));
}

#[test]
fn test_alpha_lsp_validation() {
    use alpha_lang::lsp::server::validate_alpha_code;

    // 1. Valid code
    let valid_code = "var x: int = 10
var y: unt = 20";
    let errors = validate_alpha_code(valid_code);
    assert!(errors.is_empty());

    // 2. Syntax error
    let invalid_code = "var x: int = ";
    let errors = validate_alpha_code(invalid_code);
    assert!(!errors.is_empty());

    // 3. Strict mode violation
    let strict_violation = "@strict
var x = 10";
    let errors = validate_alpha_code(strict_violation);
    assert!(!errors.is_empty());
    assert!(errors[0].message.contains("[Strict Mode]"));
}


#[test]
fn test_core_module_import_required_for_basic() {
    let source = r#"
function main() [public, nothing] {
    print("hello without import")
}
"#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut tc = TypeChecker::new();
    let err = tc.check_program(&program).expect_err("Should fail without import Core/Basic.alpha");
    assert!(err.message.contains("requires 'import Core/Basic.alpha'"));
}

#[test]
fn test_core_module_import_required_for_time() {
    let source = r#"
function main() [public, int] {
    return Time.now()
}
"#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut tc = TypeChecker::new();
    let err = tc.check_program(&program).expect_err("Should fail without import Core/Time.alpha");
    assert!(err.message.contains("requires importing its Core library module"));
}

#[test]
fn test_defer_keyword_rejected_with_hint() {
    let source = r#"
function test() [public, nothing] {
    defer do_something()
}
"#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let err = parser.parse_program().expect_err("defer should be rejected");
    assert!(err.message.contains("The 'defer' keyword has been removed"));
}


#[test]
fn test_alpha_lsp_features() {
    use alpha_lang::lsp::LspServer;
    use serde_json::json;

    let mut server = LspServer::new();
    let uri = "file:///test.alpha";
    let doc = r#"import Core/Basic.alpha
import Core/Time.alpha

structure Player {
    name: string,
    score: unt,
}

enum Status {
    Active,
    Idle,
}

function calculate_sum(a: int, b: int) [public, int] {
    return a + b
}

var global_count: int = 100
"#;

    server.add_document(uri, doc);

    // 1. Document symbols outline
    let sym_req = json!({
        "params": {
            "textDocument": { "uri": uri }
        }
    });
    let symbols = server.get_document_symbols(&sym_req);
    let sym_arr = symbols.as_array().expect("Expected symbols array");
    assert!(sym_arr.iter().any(|s| s["name"].as_str() == Some("calculate_sum(...)")));
    assert!(sym_arr.iter().any(|s| s["name"].as_str() == Some("Player")));
    assert!(sym_arr.iter().any(|s| s["name"].as_str() == Some("Status")));
    assert!(sym_arr.iter().any(|s| s["name"].as_str() == Some("global_count")));

    // 2. Go to definition
    let def_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 13, "character": 12 } // on "calculate_sum"
        }
    });
    let def_res = server.get_definition(&def_req);
    assert_eq!(def_res["uri"].as_str(), Some(uri));
    assert_eq!(def_res["range"]["start"]["line"].as_u64(), Some(13));

    // 3. Hover
    let hover_unt = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 5, "character": 12 } // on "unt"
        }
    });
    let hover_res = server.get_hover(&hover_unt);
    let hover_text = hover_res["contents"]["value"].as_str().unwrap_or_default();
    assert!(hover_text.contains("64-bit unsigned integer"));

    let hover_func = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 13, "character": 12 } // on "calculate_sum"
        }
    });
    let hover_func_res = server.get_hover(&hover_func);
    let func_text = hover_func_res["contents"]["value"].as_str().unwrap_or_default();
    assert!(func_text.contains("calculate_sum"));

    // 4. Completions after "import "
    let comp_import_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 7 } // right after "import "
        }
    });
    let comp_import = server.get_completions(&comp_import_req);
    assert!(comp_import.iter().any(|c| c["label"].as_str() == Some("Core/Basic.alpha")));
    assert!(comp_import.iter().any(|c| c["label"].as_str() == Some("Core/Time.alpha")));

    // 5. Signature help
    let sig_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 13, "character": 24 } // after "calculate_sum("
        }
    });
    let sig_res = server.get_signature_help(&sig_req);
    assert!(sig_res["signatures"].is_array());
}

#[test]
fn test_everything_type_and_strict_mode() {
    let source = r#"
@strict

var a: everything = 42
var b: everything = "hello world"
var c: everything = true

function check_everything(val: everything) [public, everything] {
    return val
}

function main() [public, int] {
    var res: everything = check_everything(a)
    return 0
}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");
    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Typechecking everything in strict mode should succeed");

    let invalid_source = r#"
var bad: everything = nothing
"#;
    let mut lexer2 = Lexer::new(invalid_source);
    let tokens2 = lexer2.tokenize().unwrap();
    let mut parser2 = Parser::new(tokens2);
    let prog2 = parser2.parse_program().unwrap();
    let mut tc2 = TypeChecker::new();
    assert!(tc2.check_program(&prog2).is_err());
}

#[test]
fn test_arena_allocator_full_lifecycle() {
    let source = r#"
import Core/ArenaAllocator

var sector: Mem.ArenaSector = mem.arena.allocate()
var my_text: string = "Alpha Arena Memory"

sector.insert(my_text)

var c1: int = sector.count()
var read_back: string = my_text

sector.reset()
var c2: int = sector.count()

function main() [public, nothing] {}
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Typechecking arena allocator should succeed");

    let mut interp = Interpreter::new();
    interp.execute_program(&program, None).expect("Running arena lifecycle should succeed");

    assert_eq!(interp.env.get("c1"), Some(alpha_lang::runtime::value::Value::Int(1)));
    assert_eq!(interp.env.get("c2"), Some(alpha_lang::runtime::value::Value::Int(0)));
    assert_eq!(interp.env.get("read_back"), Some(alpha_lang::runtime::value::Value::String("Alpha Arena Memory".to_string())));

    // Accessing my_text after sector.reset() must trigger use-after-free error!
    let uaf_source = r#"
import Core/ArenaAllocator

var sector: Mem.ArenaSector = mem.arena.allocate()
var secret: string = "Secret Data"
sector.insert(secret)
sector.reset()

var fail: string = secret
"#;
    let mut lexer_uaf = Lexer::new(uaf_source);
    let tokens_uaf = lexer_uaf.tokenize().unwrap();
    let mut parser_uaf = Parser::new(tokens_uaf);
    let prog_uaf = parser_uaf.parse_program().unwrap();
    let mut interp_uaf = Interpreter::new();
    let err = interp_uaf.execute_program(&prog_uaf, None).unwrap_err();
    assert!(err.to_string().contains("Arena memory error") || err.to_string().contains("Use-After-Free"));
}

#[test]
fn test_typechecked_function_valid() {
    let source = r#"
function compute(x: int) [here, int] #typechecked {
    var result: int = x + 42
    return result
}

function name() [here, nothing] #typechecked {
    var a: int = 10
}

function main() [public, int] {
    name()
    return compute(10)
}
"#;
    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution should succeed");
    assert_eq!(res, Value::Int(52));
}

#[test]
fn test_typechecked_forbids_untyped_variable() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
function calc() [here, nothing] #typechecked {
    var x = 10
}
"#;
    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected error for untyped variable in #typechecked function");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("explicit type annotation"));
}

#[test]
fn test_typechecked_forbids_something_and_nothing() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
function calc() [here, nothing] #typechecked {
    var x: something = 10
}
"#;
    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected error for 'something' in #typechecked function");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("forbidden"));

    let source2 = r#"
function calc(param: something) [here, nothing] #typechecked {}
"#;
    let errors2 = validate_alpha_code(source2);
    assert!(!errors2.is_empty(), "Expected error for 'something' parameter in #typechecked function");
    assert!(errors2[0].message.contains("[Strict Mode]") && errors2[0].message.contains("forbidden"));
}

#[test]
fn test_typechecked_forbids_force_unwrap() {
    use alpha_lang::lsp::server::validate_alpha_code;

    let source = r#"
function calc(x: int) [here, int] #typechecked {
    var res: int = x!?
    return res
}
"#;
    let errors = validate_alpha_code(source);
    assert!(!errors.is_empty(), "Expected error for force unwrap in #typechecked function");
    assert!(errors[0].message.contains("[Strict Mode]") && errors[0].message.contains("Force unwrap"));
}

#[test]
fn test_typechecked_attached_method() {
    let source = r#"
structure Calculator {
    base: int,
attach {
    function add(amount: int) [nonstatic, public, int] #typechecked {
        var total: int = this.base + amount
        return total
    }
}
}

function main() [public, int] {
    var calc: Calculator = make Calculator { base: 100 }
    return calc.add(50)
}
"#;
    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution should succeed");
    assert_eq!(res, Value::Int(150));
}

#[test]
fn test_string_chars_iteration() {
    let source = r#"
function main() [public, int] {
    var content: string = "Alpha"
    var char_count: int = 0
    var collected: string = ""

    for each c in content.chars() {
        char_count = char_count + 1
        collected = collected + c
    }

    var direct_count: int = 0
    for each ch in content {
        direct_count = direct_count + 1
    }

    if char_count == 5 && collected == "Alpha" && direct_count == 5 {
        return 1
    }
    return 0
}
"#;
    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution should succeed");
    assert_eq!(res, Value::Int(1));
}

#[test]
fn test_string_indexing_and_chars_indexing() {
    let source = r#"
function main() [public, int] {
    var content: string = "Hello"
    var c1: char = content.chars()[1]
    var c2: char = content.chars()[2]
    var c3: char = content[1]
    var c4: char = content[5]

    if c1 == 'H' && c2 == 'e' && c3 == 'H' && c4 == 'o' {
        return 100
    }
    return 0
}
"#;
    let mut interp = Interpreter::new();
    let res = interp.eval_source(source, None).expect("Execution should succeed");
    assert_eq!(res, Value::Int(100));
}


#[test]
fn test_alpha_lsp_smart_member_completions() {
    use alpha_lang::lsp::LspServer;
    use serde_json::json;

    let mut server = LspServer::new();
    let uri = "file:///main.alpha";
    let doc = r#"@strict
import Core/Io.alpha
import Core/Basic.alpha
import Core/Cli.alpha

function main() [here, nothing] #typechecked {
    var args: vect<string> = cli.arguments()
    var file: Io.File = Io.open(args[1])
    var content: string = file.content()
    content.
}
"#;
    server.add_document(uri, doc);

    // 1. Completion after "content."
    let comp_content_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 9, "character": 12 }
        }
    });
    let comps = server.get_completions(&comp_content_req);
    assert!(comps.iter().any(|c| c["label"].as_str() == Some("chars()")), "chars() should be suggested on content.");
    assert!(comps.iter().any(|c| c["label"].as_str() == Some("split(sep)")), "split() should be suggested on content.");
    assert!(comps.iter().any(|c| c["label"].as_str() == Some("len()")), "len() should be suggested on content.");
    assert!(comps.iter().any(|c| c["label"].as_str() == Some("trim()")), "trim() should be suggested on content.");

    // 2. Completion after "file."
    let doc2 = r#"@strict
import Core/Io.alpha

function main() [here, nothing] #typechecked {
    var file: Io.File = Io.open("test.txt")
    file.
}
"#;
    server.add_document(uri, doc2);
    let comp_file_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 5, "character": 9 }
        }
    });
    let file_comps = server.get_completions(&comp_file_req);
    assert!(file_comps.iter().any(|c| c["label"].as_str() == Some("content()")), "content() should be suggested on file.");
    assert!(file_comps.iter().any(|c| c["label"].as_str() == Some("write(text)")), "write() should be suggested on file.");
    assert!(file_comps.iter().any(|c| c["label"].as_str() == Some("reset()")), "reset() should be suggested on file.");

    // 3. Chained completion after "file.content()."
    let doc3 = r#"@strict
import Core/Io.alpha

function main() [here, nothing] #typechecked {
    var file: Io.File = Io.open("test.txt")
    file.content().
}
"#;
    server.add_document(uri, doc3);
    let comp_chained_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 5, "character": 19 }
        }
    });
    let chained_comps = server.get_completions(&comp_chained_req);
    assert!(chained_comps.iter().any(|c| c["label"].as_str() == Some("chars()")), "chars() should be suggested on file.content().");

    // 4. General completion suggests local variable "content"
    let doc4 = r#"@strict
import Core/Io.alpha

function main() [here, nothing] #typechecked {
    var content: string = "hello"
    c
}
"#;
    server.add_document(uri, doc4);
    let comp_scope_req = json!({
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": 5, "character": 5 }
        }
    });
    let scope_comps = server.get_completions(&comp_scope_req);
    assert!(scope_comps.iter().any(|c| c["label"].as_str() == Some("content")), "content local variable should be in general completions");
}

#[test]
fn test_alpha_pkg_manager() {
    use std::fs;
    let temp_dir = std::env::temp_dir().join("alpha_pkg_test_project_unit");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let toml_path = temp_dir.join("alpha-project.toml");
    fs::write(&toml_path, "[project]\nname = \"my_test_app\"\nversion = \"0.1.0\"\n").unwrap();

    let args = vec!["add".to_string(), "https://github.com/Alpha-Programming-Language-Developments/webalph".to_string()];
    let res = alpha_lang::driver::pkg::handle_pkg_command(&args, &temp_dir);
    assert!(res.is_ok());

    let alpaks_path = temp_dir.join("packages.alpaks");
    assert!(alpaks_path.exists());
    let alpaks_content = fs::read_to_string(&alpaks_path).unwrap();
    assert!(alpaks_content.contains("\"webalph\" = import"));
    assert!(alpaks_content.contains("https://github.com/Alpha-Programming-Language-Developments/webalph/blob/main/src/lib.alpha"));

    let _ = fs::remove_dir_all(&temp_dir);
}


#[test]
fn test_alpha_test_runner_and_hash_test_directive() {
    use std::fs;
    let temp_dir = std::env::temp_dir().join("alpha_test_runner_unit");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("src").join("tests")).unwrap();

    let toml_path = temp_dir.join("alpha-project.toml");
    fs::write(&toml_path, "[project]\nname = \"test_runner_app\"\nversion = \"0.1.0\"\n").unwrap();

    // 1. Create test file src/tests/math_test.alpha
    let math_test_src = r#"import Core/Basic.alpha

#test add() {
    var a: int = 2 + 3
    verify a == 5, "2 + 3 should be 5"
}

#test sub() {
    var b: int = 10 - 4
    assert b == 6, "10 - 4 should be 6"
}
"#;
    fs::write(temp_dir.join("src").join("tests").join("math_test.alpha"), math_test_src).unwrap();

    // 2. Create another test file src/tests/str_test.alpha
    let str_test_src = r#"import Core/Basic.alpha

#test check_string() {
    var s: string = "alpha"
    verify s.len() == 5, "s.len() must be 5"
    verify s.chars()[1] == 'a', "first char must be a"
}
"#;
    fs::write(temp_dir.join("src").join("tests").join("str_test.alpha"), str_test_src).unwrap();

    // Run all tests
    let res_all = alpha_lang::driver::test::run_tests(&temp_dir, None);
    assert!(res_all.is_ok(), "Running all tests should succeed");

    // Run filtered test by name "add"
    let res_single = alpha_lang::driver::test::run_tests(&temp_dir, Some("add"));
    assert!(res_single.is_ok(), "Running single test 'add' should succeed");

    // Run filtered test by name "check_string"
    let res_str = alpha_lang::driver::test::run_tests(&temp_dir, Some("check_string"));
    assert!(res_str.is_ok(), "Running single test 'check_string' should succeed");

    // 3. Add a failing test
    let fail_test_src = r#"
#test failed_assertion() {
    verify 1 == 2, "Math is broken"
}
"#;
    fs::write(temp_dir.join("src").join("tests").join("fail_test.alpha"), fail_test_src).unwrap();

    let res_fail = alpha_lang::driver::test::run_tests(&temp_dir, Some("failed_assertion"));
    assert!(res_fail.is_err(), "Failing test should return an error");

    let _ = fs::remove_dir_all(&temp_dir);
}


#[test]
fn test_io_file_getline() {
    use std::fs;
    let temp_file = std::env::temp_dir().join("alpha_getline_test.txt");
    fs::write(&temp_file, "First line\nSecond line\nThird line\n").unwrap();
    let temp_path_escaped = temp_file.to_string_lossy().replace('\\', "/");

    let source = format!(r#"@strict
import Core/Io.alpha
import Core/Basic.alpha

function main() [public, int] #typechecked {{
    var file: Io.File = Io.open("{}")
    var line1: string = file.getline(1)
    var line2: string = file.getline(2)
    var line3: string = file.getline(3)

    if line1 != "First line" {{ return 1 }}
    if line2 != "Second line" {{ return 2 }}
    if line3 != "Third line" {{ return 3 }}

    var line4 = file.getline(4)
    if !line4.nothing?() {{ return 4 }}

    var line0 = file.getline(0)
    if !line0.nothing?() {{ return 5 }}

    return 100
}}
"#, temp_path_escaped);

    let mut interp = Interpreter::new();
    let res = interp.eval_source(&source, None).expect("Execution should succeed");
    assert_eq!(res, Value::Int(100));

    let _ = fs::remove_file(&temp_file);
}

#[test]
fn test_type_alias_and_type_meta_type() {
    let source = r#"
var mytype: type = vect<map<int, string>>
var data: mytype = vect.new()
var item: map<int, string> = map.new()
item.insert(1, "Alpha")
data.push(item)

var first_item: map<int, string> = data[1]
var text: string = first_item[1]
"#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parsing failed");

    let mut typechecker = TypeChecker::new();
    typechecker.check_program(&program).expect("Type checking failed");

    let mut interp = Interpreter::new();
    interp.execute_program(&program, None).expect("Execution failed");
    assert_eq!(interp.env.get("text"), Some(Value::String("Alpha".to_string())));
}

#[test]
fn test_visibility_public_vs_private_structure() {
    let temp_dir = std::env::temp_dir().join(format!("alpha_vis_struct_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let helper_file = temp_dir.join("helper.alpha");
    let main_file = temp_dir.join("main.alpha");

    std::fs::write(&helper_file, r#"
public structure ExportedModel {
    id: int
}

structure InternalModel {
    id: int
}
"#).unwrap();

    // 1. Success case: using public structure
    let valid_main = r#"
import "helper.alpha"

function main() [public, int] {
    var m: ExportedModel = make ExportedModel { id: 42 }
    return m.id
}
"#;
    std::fs::write(&main_file, valid_main).unwrap();

    let mut lexer = Lexer::new(valid_main);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();

    let mut typechecker = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    typechecker.check_program(&program).expect("Public structure should be accessible");

    // 2. Failure case: using private structure
    let invalid_main = r#"
import "helper.alpha"

function main() [public, int] {
    var m: InternalModel = make InternalModel { id: 42 }
    return m.id
}
"#;
    let mut lexer_bad = Lexer::new(invalid_main);
    let tokens_bad = lexer_bad.tokenize().unwrap();
    let mut parser_bad = Parser::new(tokens_bad);
    let program_bad = parser_bad.parse_program().unwrap();

    let mut typechecker_bad = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    let err = typechecker_bad.check_program(&program_bad);
    assert!(err.is_err(), "Private structure should not be accessible across files");
    let err_msg = err.err().unwrap().message;
    assert!(err_msg.contains("private ('here')"), "Error should mention private ('here'), got: {}", err_msg);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_visibility_public_vs_private_enum() {
    let temp_dir = std::env::temp_dir().join(format!("alpha_vis_enum_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let helper_file = temp_dir.join("helper.alpha");
    let main_file = temp_dir.join("main.alpha");

    std::fs::write(&helper_file, r#"
public enum PublicColor {
    Red,
    Green
}

enum PrivateColor {
    Blue,
    Yellow
}
"#).unwrap();

    // 1. Success case: using public enum
    let valid_main = r#"
import "helper.alpha"

function main() [public, PublicColor] {
    return PublicColor.Red
}
"#;
    std::fs::write(&main_file, valid_main).unwrap();

    let mut lexer = Lexer::new(valid_main);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();

    let mut typechecker = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    typechecker.check_program(&program).expect("Public enum should be accessible");

    // 2. Failure case: using private enum
    let invalid_main = r#"
import "helper.alpha"

function main() [public, nothing] {
    var c = PrivateColor.Blue
}
"#;
    let mut lexer_bad = Lexer::new(invalid_main);
    let tokens_bad = lexer_bad.tokenize().unwrap();
    let mut parser_bad = Parser::new(tokens_bad);
    let program_bad = parser_bad.parse_program().unwrap();

    let mut typechecker_bad = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    let err = typechecker_bad.check_program(&program_bad);
    assert!(err.is_err(), "Private enum should not be accessible across files");
    let err_msg = err.err().unwrap().message;
    assert!(err_msg.contains("private ('here')"), "Error should mention private ('here'), got: {}", err_msg);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_visibility_public_vs_private_var_and_type_alias() {
    let temp_dir = std::env::temp_dir().join(format!("alpha_vis_var_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let helper_file = temp_dir.join("helper.alpha");
    let main_file = temp_dir.join("main.alpha");

    std::fs::write(&helper_file, r#"
public var public_score: int = 100
var private_score: int = 999

public var PublicList: type = vect<int>
var PrivateList: type = vect<string>
"#).unwrap();

    // 1. Success case: using public var and public type alias
    let valid_main = r#"
import "helper.alpha"

function main() [public, int] {
    var list: PublicList = vect.new()
    list.push(public_score)
    return list[1]
}
"#;
    std::fs::write(&main_file, valid_main).unwrap();

    let mut lexer = Lexer::new(valid_main);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap();

    let mut typechecker = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    typechecker.check_program(&program).expect("Public var and type alias should be accessible");

    // 2. Failure case: accessing private var
    let invalid_var_main = r#"
import "helper.alpha"

function main() [public, int] {
    return private_score
}
"#;
    let mut lexer_var = Lexer::new(invalid_var_main);
    let tokens_var = lexer_var.tokenize().unwrap();
    let mut parser_var = Parser::new(tokens_var);
    let program_var = parser_var.parse_program().unwrap();

    let mut typechecker_var = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    let err_var = typechecker_var.check_program(&program_var);
    assert!(err_var.is_err(), "Private var should not be accessible across files");
    let msg_var = err_var.err().unwrap().message;
    assert!(msg_var.contains("private ('here')"), "Error should mention private ('here'), got: {}", msg_var);

    // 3. Failure case: accessing private type alias
    let invalid_type_main = r#"
import "helper.alpha"

function main() [public, nothing] {
    var l: PrivateList = vect.new()
}
"#;
    let mut lexer_type = Lexer::new(invalid_type_main);
    let tokens_type = lexer_type.tokenize().unwrap();
    let mut parser_type = Parser::new(tokens_type);
    let program_type = parser_type.parse_program().unwrap();

    let mut typechecker_type = TypeChecker::new()
        .with_working_dir(temp_dir.clone())
        .with_file(main_file.to_string_lossy().to_string());
    let err_type = typechecker_type.check_program(&program_type);
    assert!(err_type.is_err(), "Private type alias should not be accessible across files");
    let msg_type = err_type.err().unwrap().message;
    assert!(msg_type.contains("private ('here')"), "Error should mention private ('here'), got: {}", msg_type);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

