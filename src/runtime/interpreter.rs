#![allow(dead_code)]
// Tree-walking runtime interpreter for the Alpha language.
use crate::diagnostics::error::{AlphaError, Span};
use crate::frontend::ast::*;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::runtime::env::Environment;
use crate::runtime::stdlib::{call_fs_dir_method, call_io_file_method, execute_console_command, format_alpha_string};
use crate::runtime::value::{FsDirHandle, IoFileHandle, StructureInstance, Value, ValueKey};
use crate::semantics::types::Type;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
enum ControlSignal {
    None,
    Return(Value),
    Break,
    Continue,
}

#[derive(Clone)]
pub(crate) struct StoredFunction {
    decl: FunctionDecl,
    source_file: Option<String>,
}

pub struct Interpreter {
    pub env: Environment,
    functions: HashMap<String, StoredFunction>,
    pub structures: HashMap<String, StructureDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub modules: HashMap<String, ModuleDecl>,
    pub cli_args: Vec<String>,
    pub package_aliases: HashMap<String, String>,
    current_structure: Option<String>,
    loaded_files: HashSet<String>,
    source_modules: HashMap<String, String>,
    current_file: Option<String>,
    working_dir: PathBuf,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Self {
            env: Environment::new(),
            functions: HashMap::new(),
            structures: HashMap::new(),
            enums: HashMap::new(),
            modules: HashMap::new(),
            cli_args: Vec::new(),
            package_aliases: HashMap::new(),
            current_structure: None,
            loaded_files: HashSet::new(),
            source_modules: HashMap::new(),
            current_file: None,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        interp.init_globals();
        interp
    }

    pub fn with_cli_args(mut self, args: Vec<String>) -> Self {
        self.cli_args = args;
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = dir;
        self
    }

    pub fn register_source_module(&mut self, path: impl Into<String>, source: impl Into<String>) {
        let p = path.into().replace('\\', "/");
        self.source_modules.insert(p, source.into());
    }

    fn init_globals(&mut self) {
        self.env.define("Io", Value::BuiltinNamespace("Io".to_string()), true);
        self.env.define("Fs", Value::BuiltinNamespace("Fs".to_string()), true);
        self.env.define(
            "dir",
            Value::FsDir(Rc::new(RefCell::new(FsDirHandle {
                path: self.working_dir.clone(),
            }))),
            true,
        );
        self.env.define("env", Value::BuiltinNamespace("env".to_string()), true);
        self.env.define("console", Value::BuiltinNamespace("console".to_string()), true);
        self.env.define("random", Value::BuiltinNamespace("random".to_string()), true);
        self.env.define("time", Value::BuiltinNamespace("time".to_string()), true);
        self.env.define("http", Value::BuiltinNamespace("http".to_string()), true);
        self.env.define("json", Value::BuiltinNamespace("json".to_string()), true);
        self.env.define("cli", Value::BuiltinNamespace("cli".to_string()), true);
    }

    /// Load and execute an Alpha source string.
    pub fn eval_source(&mut self, source: &str, file_name: Option<&str>) -> Result<Value, AlphaError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;

        self.execute_program(&program, file_name)
    }

    /// Execute a parsed program.
    pub fn execute_program(&mut self, program: &Program, file_name: Option<&str>) -> Result<Value, AlphaError> {
        let old_file = self.current_file.clone();
        self.current_file = file_name.map(|s| s.to_string());

        // 0. Load package aliases from packages.alpaks
        self.load_packages_alpaks();

        // 1. Process imports
        for import in &program.imports {
            self.execute_import(import)?;
        }

        // 1.5 Register structures
        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
        }
        for stmt in &program.statements {
            if let Statement::Structure(st) = stmt {
                self.structures.insert(st.name.clone(), st.clone());
            }
        }

        // 1.6 Register enums
        for en in &program.enums {
            self.enums.insert(en.name.clone(), en.clone());
        }
        for stmt in &program.statements {
            if let Statement::Enum(en) = stmt {
                self.enums.insert(en.name.clone(), en.clone());
            }
        }

        // 1.7 Register modules
        for m in &program.modules {
            self.modules.insert(m.name.clone(), m.clone());
            for func in &m.functions {
                let qname = format!("{}.{}", m.name, func.name);
                self.functions.insert(
                    qname,
                    StoredFunction {
                        decl: func.clone(),
                        source_file: self.current_file.clone(),
                    },
                );
            }
        }
        for stmt in &program.statements {
            if let Statement::Module(m) = stmt {
                self.modules.insert(m.name.clone(), m.clone());
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname,
                        StoredFunction {
                            decl: func.clone(),
                            source_file: self.current_file.clone(),
                        },
                    );
                }
            }
        }

        // 2. Register functions
        for func in &program.functions {
            self.functions.insert(
                func.name.clone(),
                StoredFunction {
                    decl: func.clone(),
                    source_file: self.current_file.clone(),
                },
            );
        }

        // 3. Execute top-level statements
        let mut top_defers: Vec<Statement> = Vec::new();
        for stmt in &program.statements {
            if let Statement::Defer { statement, .. } = stmt {
                top_defers.push((**statement).clone());
                continue;
            }
            let signal = self.eval_statement(stmt)?;
            if let ControlSignal::Return(v) = signal {
                for d in top_defers.into_iter().rev() {
                    let _ = self.eval_statement(&d);
                }
                self.current_file = old_file;
                return Ok(v);
            }
        }

        // 4. Run 'main' function if present
        let mut result = Value::Nothing;
        if let Some(main_func) = self.functions.get("main").cloned() {
            result = self.call_function(&main_func.decl, &[], Span::default())?;
        }

        for d in top_defers.into_iter().rev() {
            let _ = self.eval_statement(&d);
        }

        self.current_file = old_file;
        Ok(result)
    }

    pub(crate) fn lookup_function(&self, name: &str) -> Option<StoredFunction> {
        if let Some(sf) = self.functions.get(name) {
            return Some(sf.clone());
        }
        let clean = name.replace('?', "");
        if !clean.is_empty() {
            if let Some(sf) = self.functions.get(&clean) {
                return Some(sf.clone());
            }
            for (k, sf) in &self.functions {
                if k.replace('?', "") == clean {
                    return Some(sf.clone());
                }
            }
        }
        None
    }

    fn execute_import(&mut self, import: &ImportDecl) -> Result<(), AlphaError> {
        match &import.target {
            ImportTarget::Core(core_path) => {
                let candidate1 = self.working_dir.join("src").join(core_path);
                let candidate2 = self.working_dir.join(core_path);
                let candidate3 = Path::new("src").join(core_path);
                let candidate4 = if let Some(cur) = &self.current_file {
                    Path::new(cur).parent().unwrap_or(Path::new("")).join("..").join(core_path)
                } else {
                    PathBuf::new()
                };
                let candidate5 = if let Some(cur) = &self.current_file {
                    Path::new(cur).parent().unwrap_or(Path::new("")).join("..").join("src").join(core_path)
                } else {
                    PathBuf::new()
                };

                let target_path = if self.source_modules.contains_key(core_path)
                    || self.source_modules.contains_key(&format!("src/{}", core_path))
                {
                    Some(PathBuf::from(core_path))
                } else if candidate1.exists() {
                    Some(candidate1)
                } else if candidate2.exists() {
                    Some(candidate2)
                } else if candidate3.exists() {
                    Some(candidate3)
                } else if candidate4.exists() {
                    Some(candidate4)
                } else if candidate5.exists() {
                    Some(candidate5)
                } else {
                    None
                };

                if let Some(path) = target_path {
                    self.load_file(&path)?;
                } else {
                    // If it is not a standard builtin and not on disk, return error
                    if core_path != "Core/Basic.alpha"
                        && core_path != "Core/Io.alpha"
                        && core_path != "Core/Fs.alpha"
                        && core_path != "Core/Env.alpha"
                        && core_path != "Core/Math.alpha"
                        && core_path != "Core/Machine.alpha"
                        && core_path != "Core/Random.alpha"
                        && core_path != "Core/Time.alpha"
                        && core_path != "Core/Http.alpha"
                        && core_path != "Core/Json.alpha"
                        && core_path != "Core/Cli.alpha"
                    {
                        return Err(AlphaError::runtime(
                            format!("Core module '{}' not found", core_path),
                            import.span,
                        )
                        .with_hint(format!("Place your custom module inside the 'src/{}' directory", core_path)));
                    }
                }
                Ok(())
            }
            ImportTarget::Local(path_str) => {
                let raw_name = path_str.trim_end_matches(".alpha");
                if let Some(url) = self.package_aliases.get(path_str).or_else(|| self.package_aliases.get(raw_name)).cloned() {
                    return self.load_web_module(&url, import.span);
                }

                let file_path = if self.source_modules.contains_key(path_str)
                    || self.source_modules.contains_key(&format!("src/{}", path_str))
                {
                    PathBuf::from(path_str)
                } else if let Some(cur) = &self.current_file {
                    let candidate = Path::new(cur).parent().unwrap_or(Path::new("")).join(path_str);
                    if candidate.exists() {
                        candidate
                    } else if self.working_dir.join(path_str).exists() {
                        self.working_dir.join(path_str)
                    } else if self.working_dir.join("src").join(path_str).exists() {
                        self.working_dir.join("src").join(path_str)
                    } else {
                        candidate
                    }
                } else if self.working_dir.join(path_str).exists() {
                    self.working_dir.join(path_str)
                } else {
                    self.working_dir.join("src").join(path_str)
                };

                self.load_file(&file_path)?;
                Ok(())
            }
            ImportTarget::Web(url) => {
                Err(AlphaError::runtime(
                    format!(
                        "Direct web imports in source code are prohibited. Define '{}' in 'packages.alpaks' instead: \"alias\" = import web::\"{}\"",
                        url, url
                    ),
                    import.span,
                ))
            }
        }
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), AlphaError> {
        let normalized = path.to_string_lossy().replace('\\', "/");
        let path_key = fs::canonicalize(path)
            .map(|c| c.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| normalized.clone());

        if self.loaded_files.contains(&path_key) || self.loaded_files.contains(&normalized) {
            return Ok(());
        }
        self.loaded_files.insert(path_key.clone());
        self.loaded_files.insert(normalized.clone());

        let source = if let Some(s) = self.source_modules.get(&normalized) {
            s.clone()
        } else if let Some(s) = self.source_modules.get(&path_key) {
            s.clone()
        } else if let Some(s) = self.source_modules.get(&format!("src/{}", normalized)) {
            s.clone()
        } else {
            fs::read_to_string(path).map_err(|e| {
                AlphaError::runtime(
                    format!("Failed to read imported file {:?}: {}", path, e),
                    Span::default(),
                )
            })?
        };

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;

        let old_file = self.current_file.clone();
        self.current_file = Some(path_key);

        for import in &program.imports {
            self.execute_import(import)?;
        }

        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
        }
        for stmt in &program.statements {
            if let Statement::Structure(st) = stmt {
                self.structures.insert(st.name.clone(), st.clone());
            }
        }

        for en in &program.enums {
            self.enums.insert(en.name.clone(), en.clone());
        }
        for stmt in &program.statements {
            if let Statement::Enum(en) = stmt {
                self.enums.insert(en.name.clone(), en.clone());
            }
        }

        for m in &program.modules {
            self.modules.insert(m.name.clone(), m.clone());
            for func in &m.functions {
                let qname = format!("{}.{}", m.name, func.name);
                self.functions.insert(
                    qname,
                    StoredFunction {
                        decl: func.clone(),
                        source_file: self.current_file.clone(),
                    },
                );
            }
        }
        for stmt in &program.statements {
            if let Statement::Module(m) = stmt {
                self.modules.insert(m.name.clone(), m.clone());
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname,
                        StoredFunction {
                            decl: func.clone(),
                            source_file: self.current_file.clone(),
                        },
                    );
                }
            }
        }

        for func in &program.functions {
            // Check visibility: here vs public
            self.functions.insert(
                func.name.clone(),
                StoredFunction {
                    decl: func.clone(),
                    source_file: self.current_file.clone(),
                },
            );
        }

        for stmt in &program.statements {
            let _ = self.eval_statement(stmt)?;
        }

        self.current_file = old_file;
        Ok(())
    }

    pub fn load_packages_alpaks(&mut self) {
        let candidates = [
            self.working_dir.join("packages.alpaks"),
            PathBuf::from("packages.alpaks"),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                if let Ok(content) = fs::read_to_string(candidate) {
                    let map = crate::runtime::stdlib::parse_packages_alpaks(&content);
                    for (k, v) in map {
                        self.package_aliases.insert(k, v);
                    }
                    break;
                }
            }
        }
    }

    pub fn load_web_module(&mut self, url: &str, span: Span) -> Result<(), AlphaError> {
        if self.loaded_files.contains(url) {
            return Ok(());
        }
        self.loaded_files.insert(url.to_string());
        println!("[Alpha Notice] Loaded remote module: {}", url);

        let source = crate::runtime::stdlib::fetch_web_module(url)
            .map_err(|e| AlphaError::runtime(e, span))?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| e.with_file(url))?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| e.with_file(url))?;

        for import in &program.imports {
            self.execute_import(import)?;
        }

        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
        }
        for stmt in &program.statements {
            if let Statement::Structure(st) = stmt {
                self.structures.insert(st.name.clone(), st.clone());
            }
        }

        for en in &program.enums {
            self.enums.insert(en.name.clone(), en.clone());
        }
        for stmt in &program.statements {
            if let Statement::Enum(en) = stmt {
                self.enums.insert(en.name.clone(), en.clone());
            }
        }

        for m in &program.modules {
            self.modules.insert(m.name.clone(), m.clone());
            for func in &m.functions {
                let qname = format!("{}.{}", m.name, func.name);
                self.functions.insert(
                    qname,
                    StoredFunction {
                        decl: func.clone(),
                        source_file: Some(url.to_string()),
                    },
                );
            }
        }
        for stmt in &program.statements {
            if let Statement::Module(m) = stmt {
                self.modules.insert(m.name.clone(), m.clone());
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname,
                        StoredFunction {
                            decl: func.clone(),
                            source_file: Some(url.to_string()),
                        },
                    );
                }
            }
        }

        for func in &program.functions {
            self.functions.insert(
                func.name.clone(),
                StoredFunction {
                    decl: func.clone(),
                    source_file: Some(url.to_string()),
                },
            );
        }

        for stmt in &program.statements {
            let _ = self.eval_statement(stmt)?;
        }
        Ok(())
    }

    fn eval_block(&mut self, block: &Block) -> Result<ControlSignal, AlphaError> {
        self.env.enter_scope();
        let mut defers: Vec<Statement> = Vec::new();
        let mut signal = ControlSignal::None;
        let mut block_err = None;

        for stmt in &block.statements {
            if let Statement::Defer { statement, .. } = stmt {
                defers.push((**statement).clone());
                continue;
            }
            match self.eval_statement(stmt) {
                Ok(sig) => {
                    signal = sig;
                    if !matches!(signal, ControlSignal::None) {
                        break;
                    }
                }
                Err(e) => {
                    block_err = Some(e);
                    break;
                }
            }
        }

        // Always execute defers in reverse order (LIFO) before exiting scope
        for d in defers.into_iter().rev() {
            let _ = self.eval_statement(&d);
        }

        self.env.exit_scope();

        if let Some(err) = block_err {
            return Err(err);
        }

        Ok(signal)
    }

    fn eval_statement(&mut self, stmt: &Statement) -> Result<ControlSignal, AlphaError> {
        match stmt {
            Statement::Block(b) => self.eval_block(b),
            Statement::Defer { statement, .. } => self.eval_statement(statement),

            Statement::Var(decl) => {
                let val = self.eval_expr(&decl.init)?;
                self.env.define(decl.name.clone(), val, decl.is_const);
                Ok(ControlSignal::None)
            }

            Statement::VarTuple {
                names,
                init,
                is_const,
                span,
                ..
            } => {
                let val = self.eval_expr(init)?;
                match val {
                    Value::Tuple(items) => {
                        if items.len() != names.len() {
                            return Err(AlphaError::runtime(
                                format!("Cannot unpack tuple of length {} into {} variables", items.len(), names.len()),
                                *span,
                            ));
                        }
                        for (name, item) in names.iter().zip(items) {
                            self.env.define(name.clone(), item, *is_const);
                        }
                    }
                    _ => {
                        return Err(AlphaError::runtime(
                            format!("Cannot unpack non-tuple value of type '{}'", val.type_name()),
                            *span,
                        ));
                    }
                }
                Ok(ControlSignal::None)
            }

            Statement::Assign {
                target,
                op,
                value,
                span,
            } => {
                let rhs_val = self.eval_expr(value)?;

                match target {
                    Expr::Variable(name, _) => {
                        let final_val = match op {
                            AssignOp::Assign => rhs_val,
                            AssignOp::AddAssign => {
                                let cur = self.env.get(name).unwrap_or(Value::Int(0));
                                self.add_values(&cur, &rhs_val, *span)?
                            }
                            AssignOp::SubAssign => {
                                let cur = self.env.get(name).unwrap_or(Value::Int(0));
                                self.sub_values(&cur, &rhs_val, *span)?
                            }
                            AssignOp::MulAssign => {
                                let cur = self.env.get(name).unwrap_or(Value::Int(0));
                                self.mul_values(&cur, &rhs_val, *span)?
                            }
                            AssignOp::DivAssign => {
                                let cur = self.env.get(name).unwrap_or(Value::Int(0));
                                self.div_values(&cur, &rhs_val, *span)?
                            }
                            AssignOp::ModAssign => {
                                let cur = self.env.get(name).unwrap_or(Value::Int(0));
                                self.mod_values(&cur, &rhs_val, *span)?
                            }
                        };
                        self.env.assign(name, final_val).map_err(|e| AlphaError::runtime(e, *span))?;
                    }
                    Expr::Tuple(targets, target_span) => {
                        if *op != AssignOp::Assign {
                            return Err(AlphaError::runtime("Compound assignment is not supported for tuple unpacking", *span));
                        }
                        match rhs_val {
                            Value::Tuple(items) => {
                                if items.len() != targets.len() {
                                    return Err(AlphaError::runtime(
                                        format!("Cannot unpack tuple of length {} into {} targets", items.len(), targets.len()),
                                        *span,
                                    ));
                                }
                                for (tgt, item) in targets.iter().zip(items) {
                                    if let Expr::Variable(name, var_span) = tgt {
                                        self.env.assign(name, item).map_err(|e| AlphaError::runtime(e, *var_span))?;
                                    } else {
                                        return Err(AlphaError::runtime("Tuple unpacking target must be variable", *target_span));
                                    }
                                }
                            }
                            _ => {
                                return Err(AlphaError::runtime(
                                    format!("Cannot unpack non-tuple value of type '{}'", rhs_val.type_name()),
                                    *span,
                                ));
                            }
                        }
                    }
                    Expr::Index { object, index, span: idx_span } => {
                        let obj = self.eval_expr(object)?;
                        let idx = self.eval_expr(index)?;
                        match obj {
                            Value::Vect(vec_rc) => {
                                if let Value::Int(i) = idx {
                                    let mut v = vec_rc.borrow_mut();
                                    if i < 1 || (i as usize) > v.len() {
                                        return Err(AlphaError::runtime(
                                            format!("Vector index out of bounds: 1-based index {} but length is {}", i, v.len()),
                                            *idx_span,
                                        ));
                                    }
                                    v[(i - 1) as usize] = rhs_val;
                                } else {
                                    return Err(AlphaError::runtime("Vector index must be an integer", *idx_span));
                                }
                            }
                            Value::Map(map_rc) => {
                                let key = idx.to_key().map_err(|e| AlphaError::runtime(e, *idx_span))?;
                                map_rc.borrow_mut().insert(key, rhs_val);
                            }
                            _ => return Err(AlphaError::runtime("Indexing assignment target must be vector or map", *idx_span)),
                        }
                    }
                    Expr::MemberAccess { object, member, span: mem_span } => {
                        let obj = self.eval_expr(object)?;
                        match obj {
                            Value::StructureInstance(inst) => {
                                let mut fields = inst.fields.borrow_mut();
                                let final_val = match op {
                                    AssignOp::Assign => rhs_val,
                                    AssignOp::AddAssign => {
                                        let cur = fields.get(member).cloned().unwrap_or(Value::Int(0));
                                        self.add_values(&cur, &rhs_val, *span)?
                                    }
                                    AssignOp::SubAssign => {
                                        let cur = fields.get(member).cloned().unwrap_or(Value::Int(0));
                                        self.sub_values(&cur, &rhs_val, *span)?
                                    }
                                    AssignOp::MulAssign => {
                                        let cur = fields.get(member).cloned().unwrap_or(Value::Int(0));
                                        self.mul_values(&cur, &rhs_val, *span)?
                                    }
                                    AssignOp::DivAssign => {
                                        let cur = fields.get(member).cloned().unwrap_or(Value::Int(0));
                                        self.div_values(&cur, &rhs_val, *span)?
                                    }
                                    AssignOp::ModAssign => {
                                        let cur = fields.get(member).cloned().unwrap_or(Value::Int(0));
                                        self.mod_values(&cur, &rhs_val, *span)?
                                    }
                                };
                                fields.insert(member.clone(), final_val);
                            }
                            _ => return Err(AlphaError::runtime("Member assignment only supported on structure instances", *mem_span)),
                        }
                    }
                    _ => return Err(AlphaError::runtime("Invalid assignment target", *span)),
                }

                Ok(ControlSignal::None)
            }

            Statement::IncDec { target, is_inc, span } => {
                if let Expr::Variable(name, _) = target {
                    let cur = self.env.get(name).ok_or_else(|| {
                        AlphaError::runtime(format!("Undefined variable '{}'", name), *span)
                    })?;
                    let one = Value::Int(1);
                    let new_val = if *is_inc {
                        self.add_values(&cur, &one, *span)?
                    } else {
                        self.sub_values(&cur, &one, *span)?
                    };
                    self.env.assign(name, new_val).map_err(|e| AlphaError::runtime(e, *span))?;
                } else {
                    return Err(AlphaError::runtime("Inc/dec only supported on variables", *span));
                }
                Ok(ControlSignal::None)
            }

            Statement::If {
                condition,
                then_branch,
                elseif_branches,
                else_branch,
                ..
            } => {
                let cond_val = self.eval_expr(condition)?;
                if cond_val.is_truthy() {
                    return self.eval_block(then_branch);
                }

                for (elif_cond, elif_body) in elseif_branches {
                    let elif_val = self.eval_expr(elif_cond)?;
                    if elif_val.is_truthy() {
                        return self.eval_block(elif_body);
                    }
                }

                if let Some(else_b) = else_branch {
                    return self.eval_block(else_b);
                }

                Ok(ControlSignal::None)
            }

            Statement::ForInfinite { body, .. } => {
                loop {
                    let signal = self.eval_block(body)?;
                    match signal {
                        ControlSignal::Return(_) => return Ok(signal),
                        ControlSignal::Break => break,
                        ControlSignal::Continue => continue,
                        ControlSignal::None => {}
                    }
                }
                Ok(ControlSignal::None)
            }

            Statement::ForCount {
                init,
                condition,
                step,
                body,
                ..
            } => {
                self.env.enter_scope();

                if let Some(init_s) = init {
                    if let Statement::Assign { target: Expr::Variable(name, _), op: AssignOp::Assign, value, .. } = &**init_s {
                        let val = self.eval_expr(value)?;
                        if self.env.get(name).is_none() {
                            self.env.define(name.clone(), val, false);
                        } else {
                            let _ = self.env.assign(name, val);
                        }
                    } else {
                        self.eval_statement(init_s)?;
                    }
                }

                loop {
                    if let Some(c) = condition {
                        let cond_val = self.eval_expr(c)?;
                        if !cond_val.is_truthy() {
                            break;
                        }
                    }

                    let signal = self.eval_block(body)?;
                    match signal {
                        ControlSignal::Return(_) => {
                            self.env.exit_scope();
                            return Ok(signal);
                        }
                        ControlSignal::Break => break,
                        ControlSignal::Continue => {}
                        ControlSignal::None => {}
                    }

                    if let Some(step_s) = step {
                        self.eval_statement(step_s)?;
                    }
                }

                self.env.exit_scope();
                Ok(ControlSignal::None)
            }

            Statement::ForEach {
                item_var,
                key_var,
                collection,
                body,
                span,
            } => {
                let coll_val = self.eval_expr(collection)?;

                match coll_val {
                    Value::Vect(vec_rc) => {
                        let items = vec_rc.borrow().clone();
                        for item in items {
                            self.env.enter_scope();
                            self.env.define(item_var.clone(), item, false);
                            let signal = self.eval_block(body)?;
                            self.env.exit_scope();

                            match signal {
                                ControlSignal::Return(_) => return Ok(signal),
                                ControlSignal::Break => break,
                                ControlSignal::Continue => continue,
                                ControlSignal::None => {}
                            }
                        }
                    }
                    Value::Map(map_rc) => {
                        let entries: Vec<(ValueKey, Value)> =
                            map_rc.borrow().iter().map(|(k, v)| (k.clone(), v.clone())).collect();

                        for (k, v) in entries {
                            self.env.enter_scope();
                            let k_val = match k {
                                ValueKey::Int(i) => Value::Int(i),
                                ValueKey::String(s) => Value::String(s),
                                ValueKey::Char(c) => Value::Char(c),
                                ValueKey::Boolean(b) => Value::Boolean(b),
                            };

                            if let Some(k_name) = key_var {
                                self.env.define(k_name.clone(), k_val, false);
                                self.env.define(item_var.clone(), v, false);
                            } else {
                                self.env.define(item_var.clone(), k_val, false);
                            }

                            let signal = self.eval_block(body)?;
                            self.env.exit_scope();

                            match signal {
                                ControlSignal::Return(_) => return Ok(signal),
                                ControlSignal::Break => break,
                                ControlSignal::Continue => continue,
                                ControlSignal::None => {}
                            }
                        }
                    }
                    _ => {
                        return Err(AlphaError::runtime(
                            format!("Cannot iterate over non-collection '{}'", coll_val.type_name()),
                            *span,
                        ));
                    }
                }

                Ok(ControlSignal::None)
            }

            Statement::Return { value, .. } => {
                let ret_val = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Nothing,
                };
                Ok(ControlSignal::Return(ret_val))
            }

            Statement::TryCatch {
                try_block,
                err_var,
                catch_block,
                ..
            } => {
                let res = self.eval_block(try_block);
                match res {
                    Ok(signal) => match signal {
                        ControlSignal::Return(_) | ControlSignal::Break | ControlSignal::Continue => Ok(signal),
                        ControlSignal::None => Ok(ControlSignal::None),
                    },
                    Err(e) => {
                        self.env.enter_scope();
                        self.env.define(err_var.clone(), Value::String(e.message), false);
                        let catch_signal = self.eval_block(catch_block)?;
                        self.env.exit_scope();
                        Ok(catch_signal)
                    }
                }
            }

            Statement::Break(_) => Ok(ControlSignal::Break),
            Statement::Continue(_) => Ok(ControlSignal::Continue),

            Statement::System { target, body, .. } => {
                if matches_current_system(target) {
                    self.eval_block(body)
                } else {
                    Ok(ControlSignal::None)
                }
            }

            Statement::Structure(st) => {
                self.structures.insert(st.name.clone(), st.clone());
                Ok(ControlSignal::None)
            }

            Statement::Check { target, arms, span: _ } => {
                let target_val = self.eval_expr(target)?;
                for arm in arms {
                    let mut matched = false;
                    let mut bound_val = None;
                    match &arm.pattern {
                        CheckPattern::Something => {
                            if !target_val.is_nothing() {
                                matched = true;
                                bound_val = Some(target_val.clone());
                            }
                        }
                        CheckPattern::Nothing => {
                            if target_val.is_nothing() {
                                matched = true;
                            }
                        }
                        CheckPattern::Else => {
                            matched = true;
                        }
                        CheckPattern::Expr(pat_expr) => {
                            let pat_val = self.eval_expr(pat_expr)?;
                            if pat_val == target_val {
                                matched = true;
                            }
                        }
                    }

                    if matched {
                        self.env.enter_scope();
                        if let (Some(binding), Some(val)) = (&arm.binding, bound_val) {
                            self.env.define(binding.clone(), val, false);
                        }
                        let signal = match &arm.body {
                            CheckBody::Block(b) => self.eval_block(b)?,
                            CheckBody::Expr(e) => {
                                self.eval_expr(e)?;
                                ControlSignal::None
                            }
                        };
                        self.env.exit_scope();
                        if !matches!(signal, ControlSignal::None) {
                            return Ok(signal);
                        }
                        break;
                    }
                }
                Ok(ControlSignal::None)
            }

            Statement::ForRange {
                var_name,
                start,
                end,
                inclusive,
                body,
                span,
            } => {
                let start_val = self.eval_expr(start)?;
                let end_val = self.eval_expr(end)?;

                let start_i = match start_val {
                    Value::Int(i) => i,
                    _ => return Err(AlphaError::runtime("Range start must be an integer", *span)),
                };
                let end_i = match end_val {
                    Value::Int(i) => i,
                    _ => return Err(AlphaError::runtime("Range end must be an integer", *span)),
                };

                let step = if start_i <= end_i { 1 } else { -1 };
                let mut cur = start_i;

                loop {
                    let in_bounds = if *inclusive {
                        if step > 0 { cur <= end_i } else { cur >= end_i }
                    } else {
                        if step > 0 { cur < end_i } else { cur > end_i }
                    };

                    if !in_bounds {
                        break;
                    }

                    self.env.enter_scope();
                    self.env.define(var_name.clone(), Value::Int(cur), false);
                    let signal = self.eval_block(body)?;
                    self.env.exit_scope();

                    match signal {
                        ControlSignal::Break => break,
                        ControlSignal::Continue => {},
                        ControlSignal::Return(v) => return Ok(ControlSignal::Return(v)),
                        ControlSignal::None => {},
                    }

                    cur += step;
                }

                Ok(ControlSignal::None)
            }

            Statement::Verify { condition, message, span } => {
                let cond_val = self.eval_expr(condition)?;
                if !cond_val.is_truthy() {
                    let msg_str = if let Some(msg_expr) = message {
                        self.eval_expr(msg_expr)?.to_string()
                    } else {
                        "Assertion failed".to_string()
                    };
                    return Err(AlphaError::runtime(msg_str, *span)
                        .with_hint("Assertion check failed with 'verify'"));
                }
                Ok(ControlSignal::None)
            }

            Statement::Enum(en) => {
                self.enums.insert(en.name.clone(), en.clone());
                Ok(ControlSignal::None)
            }

            Statement::Module(m) => {
                self.modules.insert(m.name.clone(), m.clone());
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname,
                        StoredFunction {
                            decl: func.clone(),
                            source_file: self.current_file.clone(),
                        },
                    );
                }
                Ok(ControlSignal::None)
            }

            Statement::Expr(e) => {
                let val = self.eval_expr(e)?;
                if let Value::Function(f_decl) = val {
                    if f_decl.params.is_empty() {
                        self.call_function(&f_decl, &[], e.span())?;
                    }
                }
                Ok(ControlSignal::None)
            }
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, AlphaError> {
        match expr {
            Expr::Literal(lit, _) => match lit {
                LiteralValue::Int(i) => Ok(Value::Int(*i)),
                LiteralValue::Float(f) => Ok(Value::Float(*f)),
                LiteralValue::String(s) => Ok(Value::String(s.clone())),
                LiteralValue::Char(c) => Ok(Value::Char(*c)),
                LiteralValue::Bool(b) => Ok(Value::Boolean(*b)),
                LiteralValue::Nothing => Ok(Value::Nothing),
            },

            Expr::Variable(name, span) => {
                if let Some(val) = self.env.get(name) {
                    Ok(val)
                } else if let Some(sf) = self.lookup_function(name) {
                    Ok(Value::Function(sf.decl.clone()))
                } else if self.enums.contains_key(name) {
                    Ok(Value::BuiltinNamespace(format!("enum:{}", name)))
                } else if self.modules.contains_key(name) {
                    Ok(Value::BuiltinNamespace(format!("module:{}", name)))
                } else if self.structures.contains_key(name) || self.structures.keys().any(|k| k.replace('?', "") == name.replace('?', "")) {
                    let st_name = if self.structures.contains_key(name) {
                        name.clone()
                    } else {
                        self.structures.keys().find(|k| k.replace('?', "") == name.replace('?', "")).unwrap().clone()
                    };
                    Ok(Value::BuiltinNamespace(format!("structure:{}", st_name)))
                } else {
                    Err(AlphaError::runtime(
                        format!("Undefined variable '{}'", name),
                        *span,
                    ))
                }
            }

            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                // Short-circuiting logical operators
                if *op == BinaryOp::And {
                    let l = self.eval_expr(left)?;
                    if !l.is_truthy() {
                        return Ok(Value::Boolean(false));
                    }
                    let r = self.eval_expr(right)?;
                    return Ok(Value::Boolean(r.is_truthy()));
                }
                if *op == BinaryOp::Or {
                    let l = self.eval_expr(left)?;
                    match l {
                        Value::Boolean(true) => return Ok(Value::Boolean(true)),
                        Value::Boolean(false) => {
                            let r = self.eval_expr(right)?;
                            return Ok(r);
                        }
                        Value::Nothing => {
                            let r = self.eval_expr(right)?;
                            return Ok(r);
                        }
                        other => return Ok(other),
                    }
                }

                let left_val = self.eval_expr(left)?;
                let right_val = self.eval_expr(right)?;

                match op {
                    BinaryOp::Add => self.add_values(&left_val, &right_val, *span),
                    BinaryOp::Sub => self.sub_values(&left_val, &right_val, *span),
                    BinaryOp::Mul => self.mul_values(&left_val, &right_val, *span),
                    BinaryOp::Div => self.div_values(&left_val, &right_val, *span),
                    BinaryOp::Mod => self.mod_values(&left_val, &right_val, *span),
                    BinaryOp::Equal => Ok(Value::Boolean(left_val == right_val)),
                    BinaryOp::NotEqual => Ok(Value::Boolean(left_val != right_val)),
                    BinaryOp::Less => self.cmp_less(&left_val, &right_val, *span),
                    BinaryOp::LessEqual => {
                        let lt = self.cmp_less(&left_val, &right_val, *span)?;
                        let eq = left_val == right_val;
                        Ok(Value::Boolean(lt.is_truthy() || eq))
                    }
                    BinaryOp::Greater => {
                        let lt = self.cmp_less(&right_val, &left_val, *span)?;
                        Ok(lt)
                    }
                    BinaryOp::GreaterEqual => {
                        let lt = self.cmp_less(&right_val, &left_val, *span)?;
                        let eq = left_val == right_val;
                        Ok(Value::Boolean(lt.is_truthy() || eq))
                    }
                    BinaryOp::And | BinaryOp::Or => unreachable!(),
                }
            }

            Expr::Unary { op, expr, span } => {
                let inner = self.eval_expr(expr)?;
                match op {
                    UnaryOp::Not => Ok(Value::Boolean(!inner.is_truthy())),
                    UnaryOp::Neg => match inner {
                        Value::Int(i) => Ok(Value::Int(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(AlphaError::runtime("Unary '-' requires a number", *span)),
                    },
                    UnaryOp::Pos => match inner {
                        Value::Int(i) => Ok(Value::Int(i)),
                        Value::Float(f) => Ok(Value::Float(f)),
                        _ => Err(AlphaError::runtime("Unary '+' requires a number", *span)),
                    },
                }
            }

            Expr::Call { callee, args, span } => {
                let mut evaluated_args = Vec::new();
                for a in args {
                    evaluated_args.push(self.eval_expr(a)?);
                }

                // Check for builtin functions: print, printf, format, error
                if let Expr::Variable(name, _) = &**callee {
                    match name.as_str() {
                        "print" => {
                            if evaluated_args.is_empty() {
                                println!();
                                return Ok(Value::Nothing);
                            }
                            for arg in &evaluated_args {
                                if arg.is_nothing() {
                                    return Err(AlphaError::runtime(
                                        "Cannot print 'nothing' value (print accepts any type except nothing)",
                                        *span,
                                    )
                                    .with_hint("Check if the value is nothing using '.nothing?()' before printing"));
                                }
                            }
                            let s: Vec<String> = evaluated_args.iter().map(|a| a.to_string()).collect();
                            println!("{}", s.join(" "));
                            return Ok(Value::Nothing);
                        }
                        "printf" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("printf() requires a format string", *span));
                            }
                            let fmt_str = evaluated_args[0].to_string();
                            let formatted = format_alpha_string(&fmt_str, &evaluated_args[1..])
                                .map_err(|e| AlphaError::runtime(e, *span))?;
                            print!("{}", formatted);
                            return Ok(Value::Nothing);
                        }
                        "format" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("format() requires a format string", *span));
                            }
                            let fmt_str = evaluated_args[0].to_string();
                            let formatted = format_alpha_string(&fmt_str, &evaluated_args[1..])
                                .map_err(|e| AlphaError::runtime(e, *span))?;
                            return Ok(Value::String(formatted));
                        }
                        "error" => {
                            let fmt_str = evaluated_args.first().map(|a| a.to_string()).unwrap_or_default();
                            let formatted = format_alpha_string(&fmt_str, &evaluated_args[1..])
                                .unwrap_or_else(|_| fmt_str.clone());
                            return Err(AlphaError::runtime(formatted, *span)
                                .with_hint("An explicit error was triggered via the error() function"));
                        }
                        "paint" => {
                            if evaluated_args.len() < 2 {
                                return Err(AlphaError::runtime("paint(text, hexcolor) requires 2 arguments", *span));
                            }
                            let text = evaluated_args[0].to_string();
                            let hex = match &evaluated_args[1] {
                                Value::HexColor(h) => h.clone(),
                                Value::String(s) => s.clone(),
                                other => return Err(AlphaError::runtime(format!("paint expects hexcolor as 2nd argument, found '{}'", other.type_name()), *span)),
                            };
                            return Ok(Value::String(crate::runtime::stdlib::format_hex_paint(&text, &hex)));
                        }
                        _ => {}
                    }

                    if name == "random" || name == "rand" {
                        match evaluated_args.len() {
                            0 => return Ok(Value::Float(crate::runtime::stdlib::rng_float_range(0.0, 1.0))),
                            1 => {
                                let max = evaluated_args[0].as_int().ok_or_else(|| {
                                    AlphaError::runtime("random(max) expects integer argument", *span)
                                })?;
                                return Ok(Value::Int(crate::runtime::stdlib::rng_int_range(0, max)));
                            }
                            _ => {
                                let min = evaluated_args[0].as_int().ok_or_else(|| {
                                    AlphaError::runtime("random(min, max) expects integer min", *span)
                                })?;
                                let max = evaluated_args[1].as_int().ok_or_else(|| {
                                    AlphaError::runtime("random(min, max) expects integer max", *span)
                                })?;
                                return Ok(Value::Int(crate::runtime::stdlib::rng_int_range(min, max)));
                            }
                        }
                    }

                    if let Some(sf) = self.lookup_function(name) {
                        // Check visibility
                        if sf.decl.visibility == Visibility::Here {
                            if let (Some(sf_file), Some(cur_file)) = (&sf.source_file, &self.current_file) {
                                if sf_file != cur_file {
                                    return Err(AlphaError::runtime(
                                        format!("Function '{}' is private ('here') to '{}'", name, sf_file),
                                        *span,
                                    ));
                                }
                            }
                        }
                        return self.call_function(&sf.decl, &evaluated_args, *span);
                    }
                }

                let callee_val = self.eval_expr(callee)?;
                if let Value::Function(func_decl) = callee_val {
                    self.call_function(&func_decl, &evaluated_args, *span)
                } else {
                    Err(AlphaError::runtime(
                        format!("'{}' is not a callable function", callee_val.type_name()),
                        *span,
                    ))
                }
            }

            Expr::MethodCall {
                object,
                method,
                args,
                span,
            } => {
                if let Expr::Variable(mod_name, _) = &**object {
                    if self.modules.contains_key(mod_name) {
                        let qname = format!("{}.{}", mod_name, method);
                        if let Some(sf) = self.lookup_function(&qname) {
                            let mut evaluated_args = Vec::new();
                            for a in args {
                                evaluated_args.push(self.eval_expr(a)?);
                            }
                            return self.call_function(&sf.decl, &evaluated_args, *span);
                        } else {
                            return Err(AlphaError::runtime(format!("Module '{}' has no function '{}'", mod_name, method), *span));
                        }
                    }
                }

                let obj = self.eval_expr(object)?;

                // Universal .nothing?() / .something?() check on values (unless specialized on IoFile / FsDir)
                if method == "nothing?" && !matches!(obj, Value::FsDir(_) | Value::IoFile(_)) {
                    return Ok(Value::Boolean(obj.is_nothing()));
                }
                if method == "something?" && !matches!(obj, Value::FsDir(_) | Value::IoFile(_)) {
                    return Ok(Value::Boolean(!obj.is_nothing()));
                }

                let mut evaluated_args = Vec::new();
                for a in args {
                    evaluated_args.push(self.eval_expr(a)?);
                }

                match &obj {
                    Value::Vect(vec_rc) => match method.as_str() {
                        "push" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("push() expects 1 argument", *span));
                            }
                            vec_rc.borrow_mut().push(evaluated_args[0].clone());
                            Ok(Value::Nothing)
                        }
                        "len" => Ok(Value::Int(vec_rc.borrow().len() as i64)),
                        "get" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("get() expects index argument", *span));
                            }
                            if let Value::Int(i) = evaluated_args[0] {
                                let v = vec_rc.borrow();
                                if i < 1 || (i as usize) > v.len() {
                                    Ok(Value::Nothing)
                                } else {
                                    Ok(v[(i - 1) as usize].clone())
                                }
                            } else {
                                Err(AlphaError::runtime("Vector index must be an integer", *span))
                            }
                        }
                        "pop" => {
                            let item = vec_rc.borrow_mut().pop().unwrap_or(Value::Nothing);
                            Ok(item)
                        }
                        _ => Err(AlphaError::runtime(
                            format!("Unknown method '{}' on vect", method),
                            *span,
                        )),
                    },

                    Value::Map(map_rc) => match method.as_str() {
                        "insert" => {
                            if evaluated_args.len() < 2 {
                                return Err(AlphaError::runtime("insert() expects 2 arguments: (key, value)", *span));
                            }
                            let key = evaluated_args[0].to_key().map_err(|e| AlphaError::runtime(e, *span))?;
                            let val = evaluated_args[1].clone();
                            map_rc.borrow_mut().insert(key, val);
                            Ok(Value::Nothing)
                        }
                        "contains" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("contains() expects 1 key argument", *span));
                            }
                            let key = evaluated_args[0].to_key().map_err(|e| AlphaError::runtime(e, *span))?;
                            let contains = map_rc.borrow().contains_key(&key);
                            Ok(Value::Boolean(contains))
                        }
                        "get" => {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("get() expects 1 key argument", *span));
                            }
                            let key = evaluated_args[0].to_key().map_err(|e| AlphaError::runtime(e, *span))?;
                            let m = map_rc.borrow();
                            let res = m.get(&key).cloned().unwrap_or(Value::Nothing);
                            Ok(res)
                        }
                        "len" => Ok(Value::Int(map_rc.borrow().len() as i64)),
                        _ => Err(AlphaError::runtime(
                            format!("Unknown method '{}' on map", method),
                            *span,
                        )),
                    },

                    Value::String(s) => match method.as_str() {
                        "len" => Ok(Value::Int(s.chars().count() as i64)),
                        "empty?" => Ok(Value::Boolean(s.is_empty())),
                        _ => Err(AlphaError::runtime(
                            format!("Unknown method '{}' on string", method),
                            *span,
                        )),
                    },

                    Value::StructureInstance(inst) => {
                        let st = self.structures.get(&inst.name).cloned().ok_or_else(|| {
                            AlphaError::runtime(format!("Unknown structure '{}'", inst.name), *span)
                        })?;
                        let method_decl = st.methods.iter().find(|m| (m.name == *method || m.name.replace('?', "") == method.replace('?', "")) && !m.is_static).ok_or_else(|| {
                            AlphaError::runtime(
                                format!("Structure '{}' has no method '{}'", inst.name, method),
                                *span,
                            )
                        })?.clone();

                        if method_decl.params.len() != evaluated_args.len() {
                            return Err(AlphaError::runtime(
                                format!(
                                    "Method '{}' expects {} arguments, got {}",
                                    method,
                                    method_decl.params.len(),
                                    evaluated_args.len()
                                ),
                                *span,
                            ));
                        }

                        let prev_struct = self.current_structure.clone();
                        self.current_structure = Some(inst.name.clone());
                        self.env.enter_scope();
                        self.env.define("this".to_string(), Value::StructureInstance(inst.clone()), false);
                        for (param, arg) in method_decl.params.iter().zip(&evaluated_args) {
                            self.env.define(param.name.clone(), arg.clone(), false);
                        }

                        let signal = self.eval_block(&method_decl.body)?;
                        self.env.exit_scope();
                        self.current_structure = prev_struct;

                        match signal {
                            ControlSignal::Return(v) => Ok(v),
                            _ => Ok(Value::Nothing),
                        }
                    }

                    Value::BuiltinNamespace(name) => {
                        if let Some(struct_name) = name.strip_prefix("structure:") {
                            let st = self.structures.get(struct_name).cloned().ok_or_else(|| {
                                AlphaError::runtime(format!("Unknown structure '{}'", struct_name), *span)
                            })?;
                            let method_decl = st.methods.iter().find(|m| (m.name == *method || m.name.replace('?', "") == method.replace('?', "")) && m.is_static).ok_or_else(|| {
                                AlphaError::runtime(
                                    format!("Structure '{}' has no static method '{}'", struct_name, method),
                                    *span,
                                )
                            })?.clone();

                            if method_decl.params.len() != evaluated_args.len() {
                                return Err(AlphaError::runtime(
                                    format!(
                                        "Static method '{}' expects {} arguments, got {}",
                                        method,
                                        method_decl.params.len(),
                                        evaluated_args.len()
                                    ),
                                    *span,
                                ));
                            }

                            let prev_struct = self.current_structure.clone();
                            self.current_structure = Some(struct_name.to_string());
                            self.env.enter_scope();
                            for (param, arg) in method_decl.params.iter().zip(&evaluated_args) {
                                self.env.define(param.name.clone(), arg.clone(), false);
                            }

                            let signal = self.eval_block(&method_decl.body)?;
                            self.env.exit_scope();
                            self.current_structure = prev_struct;

                            return match signal {
                                ControlSignal::Return(v) => Ok(v),
                                _ => Ok(Value::Nothing),
                            };
                        }

                        if let Some(dir_path_str) = name.strip_prefix("dir.file:") {
                            if method == "create" {
                                let file_name = evaluated_args.first().map(|a| a.to_string()).unwrap_or_default();
                                let file_path = Path::new(dir_path_str).join(file_name);
                                fs::write(&file_path, "").map_err(|e| {
                                    AlphaError::runtime(format!("Failed to create file {:?}: {}", file_path, e), *span)
                                })?;
                                return Ok(Value::Nothing);
                            }
                        }

                        if name == "Io" {
                            if method == "open" {
                                let path_str = evaluated_args.first().map(|a| a.to_string()).unwrap_or_default();
                                let p = PathBuf::from(path_str);
                                Ok(Value::IoFile(Rc::new(RefCell::new(IoFileHandle { path: p }))))
                            } else {
                                crate::runtime::stdlib::call_io_namespace_method(method, &evaluated_args, *span)
                            }
                        } else if (name == "Fs" || name == "dir") && method == "open" {
                            let path_str = evaluated_args.first().map(|a| a.to_string()).unwrap_or_else(|| ".".to_string());
                            let p = PathBuf::from(path_str);
                            Ok(Value::FsDir(Rc::new(RefCell::new(FsDirHandle { path: p }))))
                        } else if name == "Fs" {
                            crate::runtime::stdlib::call_fs_namespace_method(method, &evaluated_args, *span)
                        } else if name == "console" && method == "out" {
                            if evaluated_args.is_empty() {
                                return Err(AlphaError::runtime("console.out() requires a command string argument", *span));
                            }
                            let cmd = evaluated_args[0].to_string();
                            let output = execute_console_command(&cmd, *span)?;
                            Ok(Value::String(output))
                        } else if name == "time" {
                            match method.as_str() {
                                "now" => Ok(Value::Int(crate::runtime::stdlib::time_now())),
                                "now_ms" => Ok(Value::Int(crate::runtime::stdlib::time_now_ms())),
                                "sleep" => {
                                    let ms = evaluated_args.first().and_then(|a| a.as_int()).unwrap_or(0).max(0) as u64;
                                    crate::runtime::stdlib::time_sleep(ms);
                                    Ok(Value::Nothing)
                                }
                                "elapsed" => {
                                    let start = evaluated_args.first().and_then(|a| a.as_int()).unwrap_or(0);
                                    Ok(Value::Int(crate::runtime::stdlib::time_elapsed(start)))
                                }
                                _ => Err(AlphaError::runtime(format!("Unknown method '{}' on time", method), *span)),
                            }
                        } else if name == "http" {
                            match method.as_str() {
                                "get" => {
                                    if evaluated_args.is_empty() {
                                        return Err(AlphaError::runtime("http.get(url) requires url", *span));
                                    }
                                    let url = evaluated_args[0].to_string();
                                    let res = crate::runtime::stdlib::http_get(&url, *span)?;
                                    Ok(Value::String(res))
                                }
                                "post" => {
                                    if evaluated_args.is_empty() {
                                        return Err(AlphaError::runtime("http.post(url, body) requires url", *span));
                                    }
                                    let url = evaluated_args[0].to_string();
                                    let body = evaluated_args.get(1).map(|a| a.to_string()).unwrap_or_default();
                                    let res = crate::runtime::stdlib::http_post(&url, &body, *span)?;
                                    Ok(Value::String(res))
                                }
                                "download" => {
                                    if evaluated_args.len() < 2 {
                                        return Err(AlphaError::runtime("http.download(url, filename) requires url and filename", *span));
                                    }
                                    let url = evaluated_args[0].to_string();
                                    let file = evaluated_args[1].to_string();
                                    let ok = crate::runtime::stdlib::http_download(&url, &file, *span)?;
                                    Ok(Value::Boolean(ok))
                                }
                                _ => Err(AlphaError::runtime(format!("Unknown method '{}' on http", method), *span)),
                            }
                        } else if name == "json" {
                            match method.as_str() {
                                "parse" => {
                                    if evaluated_args.is_empty() {
                                        return Err(AlphaError::runtime("json.parse(str) requires string", *span));
                                    }
                                    let s = evaluated_args[0].to_string();
                                    crate::runtime::stdlib::json_parse(&s, *span)
                                }
                                "stringify" => {
                                    if evaluated_args.is_empty() {
                                        return Err(AlphaError::runtime("json.stringify(val) requires value", *span));
                                    }
                                    let s = crate::runtime::stdlib::json_stringify(&evaluated_args[0], *span)?;
                                    Ok(Value::String(s))
                                }
                                _ => Err(AlphaError::runtime(format!("Unknown method '{}' on json", method), *span)),
                            }
                        } else if name == "random" {
                            let m = method.replace('?', "");
                            match m.as_str() {
                                "int" => {
                                    match evaluated_args.len() {
                                        0 => Ok(Value::Int(crate::runtime::stdlib::rng_int_range(0, i64::MAX))),
                                        1 => {
                                            let max = evaluated_args[0].as_int().ok_or_else(|| {
                                                AlphaError::runtime("random.int(max) expects integer argument", *span)
                                            })?;
                                            Ok(Value::Int(crate::runtime::stdlib::rng_int_range(0, max)))
                                        }
                                        _ => {
                                            let min = evaluated_args[0].as_int().ok_or_else(|| {
                                                AlphaError::runtime("random.int(min, max) expects integer min", *span)
                                            })?;
                                            let max = evaluated_args[1].as_int().ok_or_else(|| {
                                                AlphaError::runtime("random.int(min, max) expects integer max", *span)
                                            })?;
                                            Ok(Value::Int(crate::runtime::stdlib::rng_int_range(min, max)))
                                        }
                                    }
                                }
                                "float" => {
                                    if evaluated_args.len() >= 2 {
                                        let min = evaluated_args[0].as_float().unwrap_or(0.0);
                                        let max = evaluated_args[1].as_float().unwrap_or(1.0);
                                        Ok(Value::Float(crate::runtime::stdlib::rng_float_range(min, max)))
                                    } else {
                                        Ok(Value::Float(crate::runtime::stdlib::rng_float_range(0.0, 1.0)))
                                    }
                                }
                                "bool" | "boolean" => Ok(Value::Boolean(crate::runtime::stdlib::rng_bool())),
                                "choice" => {
                                    if evaluated_args.is_empty() {
                                        return Err(AlphaError::runtime("random.choice() requires a vector argument", *span));
                                    }
                                    match &evaluated_args[0] {
                                        Value::Vect(rc) => {
                                            let v = rc.borrow();
                                            if v.is_empty() {
                                                Ok(Value::Nothing)
                                            } else {
                                                let idx = crate::runtime::stdlib::rng_int_range(0, (v.len() - 1) as i64) as usize;
                                                Ok(v[idx].clone())
                                            }
                                        }
                                        _ => Err(AlphaError::runtime("random.choice() expects a vect argument", *span)),
                                    }
                                }
                                "string" => {
                                    let len = if !evaluated_args.is_empty() {
                                        evaluated_args[0].as_int().unwrap_or(16).max(0) as usize
                                    } else {
                                        16
                                    };
                                    Ok(Value::String(crate::runtime::stdlib::rng_string(len)))
                                }
                                "seed" => {
                                    if !evaluated_args.is_empty() {
                                        let s = evaluated_args[0].as_int().unwrap_or(0);
                                        crate::runtime::stdlib::rng_seed(s);
                                    }
                                    Ok(Value::Nothing)
                                }
                                _ => Err(AlphaError::runtime(format!("Unknown method '{}' on random", method), *span)),
                            }
                        } else if name == "cli" {
                            match method.as_str() {
                                "arguments" => {
                                    let args_vals: Vec<Value> = self.cli_args.iter().map(|s| Value::String(s.clone())).collect();
                                    Ok(Value::Vect(Rc::new(RefCell::new(args_vals))))
                                }
                                "paint" => {
                                    if evaluated_args.len() < 2 {
                                        return Err(AlphaError::runtime("cli.paint(text, hexcolor) requires 2 arguments", *span));
                                    }
                                    let text = evaluated_args[0].to_string();
                                    let hex = match &evaluated_args[1] {
                                        Value::HexColor(h) => h.clone(),
                                        Value::String(s) => s.clone(),
                                        other => return Err(AlphaError::runtime(format!("cli.paint expects hexcolor as 2nd argument, found '{}'", other.type_name()), *span)),
                                    };
                                    Ok(Value::String(crate::runtime::stdlib::format_hex_paint(&text, &hex)))
                                }
                                _ => Err(AlphaError::runtime(format!("Unknown method '{}' on cli", method), *span)),
                            }
                        } else if name.starts_with("module:") {
                            let mod_name = &name["module:".len()..];
                            let qname = format!("{}.{}", mod_name, method);
                            if let Some(sf) = self.lookup_function(&qname) {
                                return self.call_function(&sf.decl, &evaluated_args, *span);
                            } else {
                                return Err(AlphaError::runtime(format!("Module '{}' has no function '{}'", mod_name, method), *span));
                            }
                        } else {
                            Err(AlphaError::runtime(
                                format!("Unknown method '{}' on namespace '{}'", method, name),
                                *span,
                            ))
                        }
                    }

                    Value::IoFile(handle) => call_io_file_method(handle, method, &evaluated_args, *span),

                    Value::FsDir(handle) => call_fs_dir_method(handle, method, &evaluated_args, *span),

                    _ => Err(AlphaError::runtime(
                        format!("Type '{}' has no method '{}'", obj.type_name(), method),
                        *span,
                    )),
                }
            }

            Expr::MemberAccess { object, member, span } => {
                if let Expr::Variable(name, _) = &**object {
                    if let Some(en) = self.enums.get(name) {
                        if let Some(ordinal) = en.variants.iter().position(|v| v == member) {
                            return Ok(Value::EnumVariant {
                                enum_name: name.clone(),
                                variant: member.clone(),
                                ordinal: ordinal + 1,
                            });
                        } else {
                            return Err(AlphaError::runtime(
                                format!("Enum '{}' has no variant '{}'", name, member),
                                *span,
                            ));
                        }
                    }
                }

                let obj = self.eval_expr(object)?;

                match &obj {
                    Value::BuiltinNamespace(ns) => {
                        if ns.starts_with("enum:") {
                            let enum_name = &ns["enum:".len()..];
                            if let Some(en) = self.enums.get(enum_name).cloned() {
                                if let Some(ordinal) = en.variants.iter().position(|v| v == member) {
                                    return Ok(Value::EnumVariant {
                                        enum_name: enum_name.to_string(),
                                        variant: member.clone(),
                                        ordinal: ordinal + 1,
                                    });
                                } else {
                                    return Err(AlphaError::runtime(
                                        format!("Enum '{}' has no variant '{}'", enum_name, member),
                                        *span,
                                    ));
                                }
                            }
                        }
                        if ns == "env" {
                            if member == "current" {
                                let cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                return Ok(Value::String(cur.to_string_lossy().to_string()));
                            }
                            if member == "exec_dir" {
                                let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
                                let dir = exe.parent().unwrap_or(Path::new("."));
                                return Ok(Value::String(dir.to_string_lossy().to_string()));
                            }
                        }
                        Err(AlphaError::runtime(
                            format!("Unknown property '{}' on namespace '{}'", member, ns),
                            *span,
                        ))
                    }
                    Value::FsDir(handle) => {
                        if member == "file" {
                            // Returns a special helper for dir.file.create(...)
                            let path_str = handle.borrow().path.to_string_lossy().to_string();
                            Ok(Value::BuiltinNamespace(format!("dir.file:{}", path_str)))
                        } else {
                            Err(AlphaError::runtime(format!("Unknown member '{}' on Fs.Dir", member), *span))
                        }
                    }
                    Value::StructureInstance(inst) => {
                        let fields = inst.fields.borrow();
                        let found_val = fields.get(member)
                            .or_else(|| fields.keys().find(|k| k.replace('?', "") == member.replace('?', "")).and_then(|k| fields.get(k)));
                        if let Some(val) = found_val {
                            Ok(val.clone())
                        } else {
                            Err(AlphaError::runtime(
                                format!("Structure '{}' has no field '{}'", inst.name, member),
                                *span,
                            ))
                        }
                    }
                    _ => Err(AlphaError::runtime(
                        format!("Cannot access property '{}' on type '{}'", member, obj.type_name()),
                        *span,
                    )),
                }
            }

            Expr::Index { object, index, span } => {
                let obj = self.eval_expr(object)?;
                let idx = self.eval_expr(index)?;

                match obj {
                    Value::Vect(vec_rc) => {
                        if let Value::Int(i) = idx {
                            let v = vec_rc.borrow();
                            if i < 1 || (i as usize) > v.len() {
                                Err(AlphaError::runtime(
                                    format!("Vector index out of bounds: 1-based index {} but length is {}", i, v.len()),
                                    *span,
                                ))
                            } else {
                                Ok(v[(i - 1) as usize].clone())
                            }
                        } else {
                            Err(AlphaError::runtime("Vector index must be an integer", *span))
                        }
                    }
                    Value::Map(map_rc) => {
                        let m = map_rc.borrow();
                        let key = idx.to_key().map_err(|e| AlphaError::runtime(e, *span))?;
                        if let Some(val) = m.get(&key) {
                            Ok(val.clone())
                        } else if let Value::Int(i) = idx {
                            if i >= 1 && (i as usize) <= m.len() {
                                let val = m.values().nth((i - 1) as usize).unwrap();
                                Ok(val.clone())
                            } else {
                                Ok(Value::Nothing)
                            }
                        } else {
                            Ok(Value::Nothing)
                        }
                    }
                    Value::Tuple(elements) => {
                        if let Value::Int(i) = idx {
                            if i < 1 || (i as usize) > elements.len() {
                                Err(AlphaError::runtime(
                                    format!("Tuple index out of bounds: 1-based index {} but length is {}", i, elements.len()),
                                    *span,
                                ))
                            } else {
                                Ok(elements[(i - 1) as usize].clone())
                            }
                        } else {
                            Err(AlphaError::runtime("Tuple index must be an integer", *span))
                        }
                    }
                    Value::String(s) => {
                        if let Value::Int(i) = idx {
                            let chars: Vec<char> = s.chars().collect();
                            if i < 1 || (i as usize) > chars.len() {
                                Err(AlphaError::runtime(
                                    format!("String index out of bounds: 1-based index {} but length is {}", i, chars.len()),
                                    *span,
                                ))
                            } else {
                                Ok(Value::Char(chars[(i - 1) as usize]))
                            }
                        } else {
                            Err(AlphaError::runtime("String index must be an integer", *span))
                        }
                    }
                    _ => Err(AlphaError::runtime(
                        format!("Type '{}' does not support indexing", obj.type_name()),
                        *span,
                    )),
                }
            }

            Expr::InterpolatedString(raw, span) => self.eval_interpolated_string(raw, *span),
            Expr::MultilineString(s, _) => Ok(Value::String(s.clone())),
            Expr::InterpolatedMultilineString(raw, span) => self.eval_interpolated_string(raw, *span),

            Expr::This(span) => {
                if let Some(val) = self.env.get("this") {
                    Ok(val)
                } else {
                    Err(AlphaError::runtime("'this' can only be used inside structure methods", *span))
                }
            }

            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                span: _,
            } => {
                let cond_val = self.eval_expr(condition)?;
                if cond_val.is_truthy() {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }

            Expr::Cast {
                expr,
                target_type,
                span,
            } => {
                let val = self.eval_expr(expr)?;
                match target_type {
                    Type::Int => match &val {
                        Value::Int(i) => Ok(Value::Int(*i)),
                        Value::Float(f) => Ok(Value::Int(*f as i64)),
                        Value::String(s) => s.trim().parse::<i64>().map(Value::Int).map_err(|_| {
                            AlphaError::runtime(format!("Cannot cast string \"{}\" to int", s), *span)
                        }),
                        Value::Char(c) => Ok(Value::Int(*c as i64)),
                        Value::Boolean(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                        _ => Err(AlphaError::runtime(format!("Cannot cast '{}' to int", val.type_name()), *span)),
                    },
                    Type::Float => match &val {
                        Value::Float(f) => Ok(Value::Float(*f)),
                        Value::Int(i) => Ok(Value::Float(*i as f64)),
                        Value::String(s) => s.trim().parse::<f64>().map(Value::Float).map_err(|_| {
                            AlphaError::runtime(format!("Cannot cast string \"{}\" to float", s), *span)
                        }),
                        _ => Err(AlphaError::runtime(format!("Cannot cast '{}' to float", val.type_name()), *span)),
                    },
                    Type::String => Ok(Value::String(val.to_string())),
                    Type::Char => match &val {
                        Value::Char(c) => Ok(Value::Char(*c)),
                        Value::Int(i) => Ok(Value::Char((*i as u8) as char)),
                        Value::String(s) => {
                            if let Some(c) = s.chars().next() {
                                Ok(Value::Char(c))
                            } else {
                                Err(AlphaError::runtime("Cannot cast empty string to char", *span))
                            }
                        }
                        _ => Err(AlphaError::runtime(format!("Cannot cast '{}' to char", val.type_name()), *span)),
                    },
                    Type::Boolean => Ok(Value::Boolean(val.is_truthy())),
                    _ => Ok(val),
                }
            }

            Expr::ForceUnwrap { expr, span } => {
                let val = self.eval_expr(expr)?;
                if val.is_nothing() {
                    return Err(AlphaError::runtime(
                        "Attempted to unwrap 'nothing' value with '!?'",
                        *span,
                    )
                    .with_hint("Check that the value is not 'nothing' before force-unwrapping, or use 'check' / 'or' operator"));
                }
                Ok(val)
            }

            Expr::Check { target, arms, span: _ } => {
                let target_val = self.eval_expr(target)?;
                for arm in arms {
                    let mut matched = false;
                    let mut bound_val = None;
                    match &arm.pattern {
                        CheckPattern::Something => {
                            if !target_val.is_nothing() {
                                matched = true;
                                bound_val = Some(target_val.clone());
                            }
                        }
                        CheckPattern::Nothing => {
                            if target_val.is_nothing() {
                                matched = true;
                            }
                        }
                        CheckPattern::Else => {
                            matched = true;
                        }
                        CheckPattern::Expr(pat_expr) => {
                            let pat_val = self.eval_expr(pat_expr)?;
                            if pat_val == target_val {
                                matched = true;
                            }
                        }
                    }

                    if matched {
                        self.env.enter_scope();
                        if let (Some(binding), Some(val)) = (&arm.binding, bound_val) {
                            self.env.define(binding.clone(), val, false);
                        }
                        let res = match &arm.body {
                            CheckBody::Block(b) => {
                                let signal = self.eval_block(b)?;
                                match signal {
                                    ControlSignal::Return(v) => v,
                                    _ => Value::Nothing,
                                }
                            }
                            CheckBody::Expr(e) => self.eval_expr(e)?,
                        };
                        self.env.exit_scope();
                        return Ok(res);
                    }
                }
                Ok(Value::Nothing)
            }

            Expr::Make {
                structure_name,
                fields,
                span,
            } => {
                let struct_name = if let Some(name) = structure_name {
                    name.clone()
                } else if let Some(curr) = &self.current_structure {
                    curr.clone()
                } else {
                    return Err(AlphaError::runtime(
                        "Cannot use 'make this' outside of a structure method",
                        *span,
                    ));
                };

                let mut field_map = HashMap::new();
                for (f_name, f_expr) in fields {
                    let f_val = self.eval_expr(f_expr)?;
                    field_map.insert(f_name.clone(), f_val);
                }

                if let Some(st) = self.structures.get(&struct_name).cloned() {
                    for (f_name, f_type) in &st.fields {
                        if !field_map.contains_key(f_name) {
                            let default_val = match f_type {
                                Type::Int => Value::Int(0),
                                Type::Float => Value::Float(0.0),
                                Type::String => Value::String(String::new()),
                                Type::Char => Value::Char('\0'),
                                Type::Boolean => Value::Boolean(false),
                                Type::Vect(_) => Value::Vect(Rc::new(RefCell::new(Vec::new()))),
                                Type::Map(_, _) => Value::Map(Rc::new(RefCell::new(HashMap::new()))),
                                _ => Value::Nothing,
                            };
                            field_map.insert(f_name.clone(), default_val);
                        }
                    }
                }

                Ok(Value::StructureInstance(StructureInstance {
                    name: struct_name,
                    fields: Rc::new(RefCell::new(field_map)),
                }))
            }

            Expr::VectNew(_) => Ok(Value::Vect(Rc::new(RefCell::new(Vec::new())))),
            Expr::MapNew(_) => Ok(Value::Map(Rc::new(RefCell::new(HashMap::new())))),
            Expr::HexColor(h, _) => Ok(Value::HexColor(h.clone())),

            Expr::Tuple(elements, _) => {
                let mut vals = Vec::new();
                for el in elements {
                    vals.push(self.eval_expr(el)?);
                }
                Ok(Value::Tuple(vals))
            }

            Expr::Slice { object, start, end, inclusive, span } => {
                let obj = self.eval_expr(object)?;
                let start_idx = if let Some(s) = start {
                    let v = self.eval_expr(s)?;
                    Some(v.as_int().ok_or_else(|| AlphaError::runtime("Slice start must be an integer", *span))?)
                } else {
                    None
                };
                let end_idx = if let Some(e) = end {
                    let v = self.eval_expr(e)?;
                    Some(v.as_int().ok_or_else(|| AlphaError::runtime("Slice end must be an integer", *span))?)
                } else {
                    None
                };

                match obj {
                    Value::String(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        let from = match start_idx {
                            Some(s) if s >= 1 => ((s - 1) as usize).min(len as usize),
                            Some(s) if s < 0 => (s + len).clamp(0, len) as usize,
                            Some(_) => 0,
                            None => 0,
                        };

                        let mut to = match end_idx {
                            Some(mut e) => {
                                if e >= 1 {
                                    e -= 1;
                                    if *inclusive { e += 1; }
                                    e.clamp(0, len) as usize
                                } else if e < 0 {
                                    e += len;
                                    if *inclusive { e += 1; }
                                    e.clamp(0, len) as usize
                                } else {
                                    0
                                }
                            }
                            None => len as usize,
                        };
                        if to < from { to = from; }
                        let slice_chars = &chars[from..to];
                        let result: String = slice_chars.iter().collect();
                        Ok(Value::String(result))
                    }
                    Value::Vect(vec_rc) => {
                        let v = vec_rc.borrow();
                        let len = v.len() as i64;
                        let from = match start_idx {
                            Some(s) if s >= 1 => ((s - 1) as usize).min(len as usize),
                            Some(s) if s < 0 => (s + len).clamp(0, len) as usize,
                            Some(_) => 0,
                            None => 0,
                        };

                        let mut to = match end_idx {
                            Some(mut e) => {
                                if e >= 1 {
                                    e -= 1;
                                    if *inclusive { e += 1; }
                                    e.clamp(0, len) as usize
                                } else if e < 0 {
                                    e += len;
                                    if *inclusive { e += 1; }
                                    e.clamp(0, len) as usize
                                } else {
                                    0
                                }
                            }
                            None => len as usize,
                        };
                        if to < from { to = from; }
                        let slice_items = v[from..to].to_vec();
                        Ok(Value::Vect(Rc::new(RefCell::new(slice_items))))
                    }
                    _ => Err(AlphaError::runtime(
                        format!("Slicing is only supported on string and vect, found '{}'", obj.type_name()),
                        *span,
                    )),
                }
            }
        }
    }

    fn eval_interpolated_string(&mut self, text: &str, span: Span) -> Result<Value, AlphaError> {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '{' || next_ch == '}' {
                        result.push(next_ch);
                        chars.next();
                        continue;
                    }
                }
                result.push(ch);
            } else if ch == '{' {
                let mut expr_code = String::new();
                let mut brace_depth = 1;
                while let Some(inner_ch) = chars.next() {
                    if inner_ch == '{' {
                        brace_depth += 1;
                        expr_code.push(inner_ch);
                    } else if inner_ch == '}' {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            break;
                        } else {
                            expr_code.push(inner_ch);
                        }
                    } else {
                        expr_code.push(inner_ch);
                    }
                }

                if brace_depth != 0 {
                    return Err(AlphaError::runtime("Unclosed '{' in interpolated string", span));
                }

                let mut lexer = Lexer::new(&expr_code);
                let tokens = lexer.tokenize()?;
                let mut parser = Parser::new(tokens);
                let expr = parser.parse_expr()?;
                let val = self.eval_expr(&expr)?;
                result.push_str(&val.to_string());
            } else {
                result.push(ch);
            }
        }

        Ok(Value::String(result))
    }

    fn call_function(
        &mut self,
        decl: &FunctionDecl,
        args: &[Value],
        span: Span,
    ) -> Result<Value, AlphaError> {
        if args.len() != decl.params.len() {
            return Err(AlphaError::runtime(
                format!(
                    "Function '{}' expects {} arguments, got {}",
                    decl.name,
                    decl.params.len(),
                    args.len()
                ),
                span,
            ));
        }

        self.env.enter_scope();
        for (param, arg) in decl.params.iter().zip(args) {
            self.env.define(param.name.clone(), arg.clone(), false);
        }

        let signal = self.eval_block(&decl.body)?;
        self.env.exit_scope();

        match signal {
            ControlSignal::Return(v) => Ok(v),
            _ => Ok(Value::Nothing),
        }
    }

    fn add_values(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x + y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 + y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x + *y as f64)),
            (Value::String(x), y) => Ok(Value::String(format!("{}{}", x, y))),
            (x, Value::String(y)) => Ok(Value::String(format!("{}{}", x, y))),
            _ => Err(AlphaError::runtime(
                format!("Cannot add '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn sub_values(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x - y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x - y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 - y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x - *y as f64)),
            _ => Err(AlphaError::runtime(
                format!("Cannot subtract '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn mul_values(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x * y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x * y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 * y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x * *y as f64)),
            _ => Err(AlphaError::runtime(
                format!("Cannot multiply '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn div_values(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                if *y == 0 {
                    return Err(AlphaError::runtime("Division by zero", span)
                        .with_hint("Denominator cannot be 0 in division"));
                }
                Ok(Value::Int(x / y))
            }
            (Value::Float(x), Value::Float(y)) => {
                if *y == 0.0 {
                    return Err(AlphaError::runtime("Division by zero", span)
                        .with_hint("Denominator cannot be 0 in division"));
                }
                Ok(Value::Float(x / y))
            }
            (Value::Int(x), Value::Float(y)) => {
                if *y == 0.0 {
                    return Err(AlphaError::runtime("Division by zero", span)
                        .with_hint("Denominator cannot be 0 in division"));
                }
                Ok(Value::Float(*x as f64 / y))
            }
            (Value::Float(x), Value::Int(y)) => {
                if *y == 0 {
                    return Err(AlphaError::runtime("Division by zero", span)
                        .with_hint("Denominator cannot be 0 in division"));
                }
                Ok(Value::Float(x / *y as f64))
            }
            _ => Err(AlphaError::runtime(
                format!("Cannot divide '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn mod_values(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                if *y == 0 {
                    return Err(AlphaError::runtime("Modulo by zero", span)
                        .with_hint("Denominator cannot be 0 in modulo operation"));
                }
                Ok(Value::Int(x % y))
            }
            _ => Err(AlphaError::runtime(
                format!("Modulo not supported on '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }

    fn cmp_less(&self, a: &Value, b: &Value, span: Span) -> Result<Value, AlphaError> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Boolean(x < y)),
            (Value::Float(x), Value::Float(y)) => Ok(Value::Boolean(x < y)),
            (Value::Int(x), Value::Float(y)) => Ok(Value::Boolean((*x as f64) < *y)),
            (Value::Float(x), Value::Int(y)) => Ok(Value::Boolean(*x < (*y as f64))),
            (Value::String(x), Value::String(y)) => Ok(Value::Boolean(x < y)),
            _ => Err(AlphaError::runtime(
                format!("Cannot compare '<' on '{}' and '{}'", a.type_name(), b.type_name()),
                span,
            )),
        }
    }
}
