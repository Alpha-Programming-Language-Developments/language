#![allow(dead_code)]
// Static type checker and semantic analyzer for the Alpha language.
use crate::diagnostics::error::{AlphaError, Span};
use crate::frontend::ast::*;
use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::semantics::types::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub ty: Type,
    pub is_const: bool,
}

#[derive(Debug, Clone)]
pub struct FuncSignature {
    pub name: String,
    pub params: Vec<Param>,
    pub visibility: Visibility,
    pub return_type: Type,
    pub defined_in: Option<String>,
}

#[derive(Default)]
pub struct Scope {
    symbols: HashMap<String, SymbolInfo>,
}

pub struct TypeChecker {
    scopes: Vec<Scope>,
    functions: HashMap<String, FuncSignature>,
    pub structures: HashMap<String, StructureDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub modules: HashMap<String, ModuleDecl>,
    current_structure: Option<String>,
    current_return_type: Option<Type>,
    current_file: Option<String>,
    loaded_files: HashSet<String>,
    working_dir: PathBuf,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut tc = Self {
            scopes: vec![Scope::default()],
            functions: HashMap::new(),
            structures: HashMap::new(),
            enums: HashMap::new(),
            modules: HashMap::new(),
            current_structure: None,
            current_return_type: None,
            current_file: None,
            loaded_files: HashSet::new(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        tc.register_builtins();
        tc
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.current_file = Some(file.into());
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = dir;
        self
    }

    fn register_builtins(&mut self) {
        // Builtins from Core/Basic.alpha
        self.functions.insert(
            "print".to_string(),
            FuncSignature {
                name: "print".to_string(),
                params: vec![Param {
                    name: "value".to_string(),
                    ty: Type::Something,
                    span: Span::default(),
                }],
                visibility: Visibility::Public,
                return_type: Type::Nothing,
                defined_in: None,
            },
        );

        // Core namespaces (Io, Fs, dir, env) as variables in global scope
        self.scopes[0].symbols.insert(
            "Io".to_string(),
            SymbolInfo {
                ty: Type::Custom("Io".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "Fs".to_string(),
            SymbolInfo {
                ty: Type::Custom("Fs".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "dir".to_string(),
            SymbolInfo {
                ty: Type::Custom("Fs.Dir".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "env".to_string(),
            SymbolInfo {
                ty: Type::Custom("Env".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "console".to_string(),
            SymbolInfo {
                ty: Type::Custom("console".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "random".to_string(),
            SymbolInfo {
                ty: Type::Custom("random".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "time".to_string(),
            SymbolInfo {
                ty: Type::Custom("time".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "http".to_string(),
            SymbolInfo {
                ty: Type::Custom("http".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "json".to_string(),
            SymbolInfo {
                ty: Type::Custom("json".to_string()),
                is_const: true,
            },
        );
        self.scopes[0].symbols.insert(
            "cli".to_string(),
            SymbolInfo {
                ty: Type::Custom("cli".to_string()),
                is_const: true,
            },
        );

        self.functions.insert(
            "paint".to_string(),
            FuncSignature {
                name: "paint".to_string(),
                params: vec![
                    Param {
                        name: "text".to_string(),
                        ty: Type::Something,
                        span: Span::default(),
                    },
                    Param {
                        name: "color".to_string(),
                        ty: Type::HexColor,
                        span: Span::default(),
                    },
                ],
                visibility: Visibility::Public,
                return_type: Type::String,
                defined_in: None,
            },
        );
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), AlphaError> {
        // 0. First pass: process all imports
        for import in &program.imports {
            self.check_import(import)?;
        }

        // 1. Second pass: register all structures, enums, modules
        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
            self.scopes[0].symbols.insert(
                st.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(st.name.clone()),
                    is_const: true,
                },
            );
        }

        for en in &program.enums {
            self.enums.insert(en.name.clone(), en.clone());
            self.scopes[0].symbols.insert(
                en.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(en.name.clone()),
                    is_const: true,
                },
            );
        }

        for m in &program.modules {
            self.modules.insert(m.name.clone(), m.clone());
            self.scopes[0].symbols.insert(
                m.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(m.name.clone()),
                    is_const: true,
                },
            );
            for func in &m.functions {
                let qname = format!("{}.{}", m.name, func.name);
                self.functions.insert(
                    qname.clone(),
                    FuncSignature {
                        name: qname,
                        params: func.params.clone(),
                        visibility: func.visibility,
                        return_type: func.return_type.clone(),
                        defined_in: self.current_file.clone(),
                    },
                );
            }
        }

        // Also register structures, enums, modules found in statements
        for stmt in &program.statements {
            if let Statement::Structure(st) = stmt {
                self.structures.insert(st.name.clone(), st.clone());
                self.scopes[0].symbols.insert(
                    st.name.clone(),
                    SymbolInfo {
                        ty: Type::Custom(st.name.clone()),
                        is_const: true,
                    },
                );
            } else if let Statement::Enum(en) = stmt {
                self.enums.insert(en.name.clone(), en.clone());
                self.scopes[0].symbols.insert(
                    en.name.clone(),
                    SymbolInfo {
                        ty: Type::Custom(en.name.clone()),
                        is_const: true,
                    },
                );
            } else if let Statement::Module(m) = stmt {
                self.modules.insert(m.name.clone(), m.clone());
                self.scopes[0].symbols.insert(
                    m.name.clone(),
                    SymbolInfo {
                        ty: Type::Custom(m.name.clone()),
                        is_const: true,
                    },
                );
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname.clone(),
                        FuncSignature {
                            name: qname,
                            params: func.params.clone(),
                            visibility: func.visibility,
                            return_type: func.return_type.clone(),
                            defined_in: self.current_file.clone(),
                        },
                    );
                }
            }
        }

        // 2. Third pass: register all function signatures
        for func in &program.functions {
            if self.functions.contains_key(&func.name) {
                return Err(AlphaError::type_error(
                    format!("Duplicate function definition: '{}'", func.name),
                    func.span,
                ));
            }

            self.functions.insert(
                func.name.clone(),
                FuncSignature {
                    name: func.name.clone(),
                    params: func.params.clone(),
                    visibility: func.visibility,
                    return_type: func.return_type.clone(),
                    defined_in: self.current_file.clone(),
                },
            );
        }

        // 3. Fourth pass: check top-level statements
        for stmt in &program.statements {
            self.check_statement(stmt).map_err(|e| {
                if let Some(f) = &self.current_file {
                    e.with_file(f)
                } else {
                    e
                }
            })?;
        }

        // 4. Fifth pass: check function bodies
        for func in &program.functions {
            self.enter_scope();
            self.current_return_type = Some(func.return_type.clone());

            for param in &func.params {
                self.declare_variable(&param.name, param.ty.clone(), false, param.span)?;
            }

            self.check_block(&func.body).map_err(|e| {
                if let Some(f) = &self.current_file {
                    e.with_file(f)
                } else {
                    e
                }
            })?;

            self.current_return_type = None;
            self.exit_scope();
        }

        // 5. Sixth pass: check structure methods
        let all_structures = self.structures.values().cloned().collect::<Vec<_>>();
        for st in all_structures {
            self.current_structure = Some(st.name.clone());
            for method in &st.methods {
                self.enter_scope();
                self.current_return_type = Some(method.return_type.clone());

                if !method.is_static {
                    self.declare_variable("this", Type::Custom(st.name.clone()), false, method.span)?;
                }

                for param in &method.params {
                    self.declare_variable(&param.name, param.ty.clone(), false, param.span)?;
                }

                self.check_block(&method.body).map_err(|e| {
                    if let Some(f) = &self.current_file {
                        e.with_file(f)
                    } else {
                        e
                    }
                })?;

                self.current_return_type = None;
                self.exit_scope();
            }
            self.current_structure = None;
        }

        // 6. Seventh pass: check module functions
        let all_modules = self.modules.values().cloned().collect::<Vec<_>>();
        for m in all_modules {
            for func in &m.functions {
                self.enter_scope();
                self.current_return_type = Some(func.return_type.clone());
                for param in &func.params {
                    self.declare_variable(&param.name, param.ty.clone(), false, param.span)?;
                }
                self.check_block(&func.body).map_err(|e| {
                    if let Some(f) = &self.current_file {
                        e.with_file(f)
                    } else {
                        e
                    }
                })?;
                self.current_return_type = None;
                self.exit_scope();
            }
        }

        Ok(())
    }

    fn resolve_import_path(&self, target: &ImportTarget) -> Result<PathBuf, AlphaError> {
        match target {
            ImportTarget::Local(path_str) => {
                let candidate1 = if let Some(cur) = &self.current_file {
                    Path::new(cur).parent().unwrap_or(Path::new("")).join(path_str)
                } else {
                    self.working_dir.join(path_str)
                };
                let candidate2 = self.working_dir.join(path_str);
                let candidate3 = self.working_dir.join("src").join(path_str);

                if candidate1.exists() {
                    Ok(candidate1)
                } else if candidate2.exists() {
                    Ok(candidate2)
                } else if candidate3.exists() {
                    Ok(candidate3)
                } else {
                    Err(AlphaError::compile(
                        format!("Imported file '{}' not found at {:?}", path_str, candidate1),
                        Span::default(),
                    )
                    .with_hint("Check the relative path to the imported file"))
                }
            }
            ImportTarget::Core(core_path) => {
                // Search order:
                // 1. working_dir/src/Core/module.alpha
                // 2. working_dir/Core/module.alpha
                // 3. current_file/../Core/module.alpha
                // 4. Builtin standard library
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

                if candidate1.exists() {
                    Ok(candidate1)
                } else if candidate2.exists() {
                    Ok(candidate2)
                } else if candidate3.exists() {
                    Ok(candidate3)
                } else if candidate4.exists() {
                    Ok(candidate4)
                } else if candidate5.exists() {
                    Ok(candidate5)
                } else {
                    if core_path == "Core/Basic.alpha"
                        || core_path == "Core/Io.alpha"
                        || core_path == "Core/Fs.alpha"
                        || core_path == "Core/Env.alpha"
                        || core_path == "Core/Math.alpha"
                        || core_path == "Core/Machine.alpha"
                        || core_path == "Core/Random.alpha"
                        || core_path == "Core/Time.alpha"
                        || core_path == "Core/Http.alpha"
                        || core_path == "Core/Json.alpha"
                        || core_path == "Core/Cli.alpha"
                    {
                        return Ok(PathBuf::from("__builtin__"));
                    }
                    Err(AlphaError::compile(
                        format!("Core module '{}' not found", core_path),
                        Span::default(),
                    )
                    .with_hint(format!("Place your custom module inside the 'src/{}' directory", core_path)))
                }
            }
            ImportTarget::Web(_) => Ok(PathBuf::from("__web__")),
        }
    }

    fn check_import(&mut self, import: &ImportDecl) -> Result<(), AlphaError> {
        if let ImportTarget::Local(path_str) = &import.target {
            let candidates = [
                self.working_dir.join("packages.alpaks"),
                PathBuf::from("packages.alpaks"),
            ];
            for candidate in &candidates {
                if candidate.exists() {
                    if let Ok(content) = fs::read_to_string(candidate) {
                        let map = crate::runtime::stdlib::parse_packages_alpaks(&content);
                        let raw = path_str.trim_end_matches(".alpha");
                        if let Some(url) = map.get(path_str).or_else(|| map.get(raw)) {
                            return self.check_web_module(url, import.span);
                        }
                    }
                }
            }
        }

        if let ImportTarget::Web(url) = &import.target {
            return self.check_web_module(url, import.span);
        }

        let path = self.resolve_import_path(&import.target)?;
        if path == Path::new("__builtin__") {
            return Ok(());
        }

        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let path_key = canonical.to_string_lossy().to_string();

        if self.loaded_files.contains(&path_key) {
            return Ok(());
        }
        self.loaded_files.insert(path_key.clone());

        let source = fs::read_to_string(&path).map_err(|e| {
            AlphaError::compile(
                format!("Failed to read imported module {:?}: {}", path, e),
                import.span,
            )
        })?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| e.with_file(&path_key))?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| e.with_file(&path_key))?;

        let old_file = self.current_file.clone();
        self.current_file = Some(path_key.clone());

        for sub_import in &program.imports {
            self.check_import(sub_import)?;
        }

        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
            self.scopes[0].symbols.insert(
                st.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(st.name.clone()),
                    is_const: true,
                },
            );
        }

        for func in &program.functions {
            self.functions.insert(
                func.name.clone(),
                FuncSignature {
                    name: func.name.clone(),
                    params: func.params.clone(),
                    visibility: func.visibility,
                    return_type: func.return_type.clone(),
                    defined_in: Some(path_key.clone()),
                },
            );
        }

        for stmt in &program.statements {
            let _ = self.check_statement(stmt);
        }

        self.current_file = old_file;
        Ok(())
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn check_web_module(&mut self, url: &str, span: Span) -> Result<(), AlphaError> {
        if self.loaded_files.contains(url) {
            return Ok(());
        }
        self.loaded_files.insert(url.to_string());

        let source = crate::runtime::stdlib::fetch_web_module(url)
            .map_err(|e| AlphaError::compile(e, span))?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| e.with_file(url))?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().map_err(|e| e.with_file(url))?;

        for sub_import in &program.imports {
            self.check_import(sub_import)?;
        }

        for st in &program.structures {
            self.structures.insert(st.name.clone(), st.clone());
            self.scopes[0].symbols.insert(
                st.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(st.name.clone()),
                    is_const: true,
                },
            );
        }

        for en in &program.enums {
            self.enums.insert(en.name.clone(), en.clone());
            self.scopes[0].symbols.insert(
                en.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(en.name.clone()),
                    is_const: true,
                },
            );
        }

        for m in &program.modules {
            self.modules.insert(m.name.clone(), m.clone());
            self.scopes[0].symbols.insert(
                m.name.clone(),
                SymbolInfo {
                    ty: Type::Custom(m.name.clone()),
                    is_const: true,
                },
            );
            for func in &m.functions {
                let qname = format!("{}.{}", m.name, func.name);
                self.functions.insert(
                    qname.clone(),
                    FuncSignature {
                        name: qname,
                        params: func.params.clone(),
                        visibility: func.visibility,
                        return_type: func.return_type.clone(),
                        defined_in: Some(url.to_string()),
                    },
                );
            }
        }

        for func in &program.functions {
            self.functions.insert(
                func.name.clone(),
                FuncSignature {
                    name: func.name.clone(),
                    params: func.params.clone(),
                    visibility: func.visibility,
                    return_type: func.return_type.clone(),
                    defined_in: Some(url.to_string()),
                },
            );
        }

        for stmt in &program.statements {
            let _ = self.check_statement(stmt);
        }
        Ok(())
    }

    fn declare_variable(
        &mut self,
        name: &str,
        ty: Type,
        is_const: bool,
        span: Span,
    ) -> Result<(), AlphaError> {
        let current_scope = self.scopes.last_mut().expect("At least one scope");
        if current_scope.symbols.contains_key(name) {
            return Err(AlphaError::type_error(
                format!("Variable '{}' already declared in this scope", name),
                span,
            ));
        }

        current_scope.symbols.insert(
            name.to_string(),
            SymbolInfo { ty, is_const },
        );
        Ok(())
    }

    fn lookup_variable(&self, name: &str) -> Option<&SymbolInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.symbols.get(name) {
                return Some(info);
            }
        }
        let clean = name.replace('?', "");
        if !clean.is_empty() {
            for scope in self.scopes.iter().rev() {
                if let Some(info) = scope.symbols.get(&clean) {
                    return Some(info);
                }
                for (k, info) in &scope.symbols {
                    if k.replace('?', "") == clean {
                        return Some(info);
                    }
                }
            }
        }
        None
    }

    fn lookup_function(&self, name: &str) -> Option<&FuncSignature> {
        if let Some(func) = self.functions.get(name) {
            return Some(func);
        }
        let clean = name.replace('?', "");
        if !clean.is_empty() {
            if let Some(func) = self.functions.get(&clean) {
                return Some(func);
            }
            for (k, func) in &self.functions {
                if k.replace('?', "") == clean {
                    return Some(func);
                }
            }
        }
        None
    }

    fn check_block(&mut self, block: &Block) -> Result<(), AlphaError> {
        self.enter_scope();
        for stmt in &block.statements {
            self.check_statement(stmt)?;
        }
        self.exit_scope();
        Ok(())
    }

    fn check_statement(&mut self, stmt: &Statement) -> Result<(), AlphaError> {
        match stmt {
            Statement::Var(decl) => {
                let init_ty = self.check_expr(&decl.init)?;
                let final_ty = if matches!(decl.init, Expr::VectNew(_)) && matches!(decl.ty, Type::Vect(_)) {
                    decl.ty.clone()
                } else if matches!(decl.init, Expr::MapNew(_)) && matches!(decl.ty, Type::Map(_, _)) {
                    decl.ty.clone()
                } else {
                    init_ty
                };

                if !final_ty.is_assignable_to(&decl.ty) {
                    return Err(AlphaError::type_error(
                        format!(
                            "Cannot initialize variable '{}' of type '{}' with value of type '{}'",
                            decl.name, decl.ty, final_ty
                        ),
                        decl.span,
                    )
                    .with_hint(format!("Make sure the value matches the declared type '{}'", decl.ty)));
                }

                self.declare_variable(&decl.name, decl.ty.clone(), decl.is_const, decl.span)?;
                Ok(())
            }

            Statement::VarTuple {
                names,
                types,
                overall_type,
                init,
                is_const,
                span,
            } => {
                let init_ty = self.check_expr(init)?;
                if let Some(ov) = overall_type {
                    if !init_ty.is_assignable_to(ov) {
                        return Err(AlphaError::type_error(
                            format!("Cannot assign '{}' to tuple of type '{}'", init_ty, ov),
                            *span,
                        ));
                    }
                }
                let elem_types: Vec<Type> = match &init_ty {
                    Type::Tuple(items) => items.clone(),
                    _ => {
                        if let Some(Type::Tuple(items)) = overall_type {
                            items.clone()
                        } else {
                            vec![Type::Something; names.len()]
                        }
                    }
                };

                for (i, name) in names.iter().enumerate() {
                    let declared_ty = types.get(i).and_then(|t| t.clone());
                    let final_ty = if let Some(dt) = declared_ty {
                        if let Some(et) = elem_types.get(i) {
                            if !et.is_assignable_to(&dt) {
                                return Err(AlphaError::type_error(
                                    format!("Tuple element {} of type '{}' cannot be assigned to '{}'", i, et, dt),
                                    *span,
                                ));
                            }
                        }
                        dt
                    } else if let Some(et) = elem_types.get(i) {
                        et.clone()
                    } else {
                        Type::Something
                    };

                    self.declare_variable(name, final_ty, *is_const, *span)?;
                }
                Ok(())
            }

            Statement::Assign {
                target,
                op: _,
                value,
                span,
            } => {
                let target_ty = self.check_expr(target)?;
                let val_ty = self.check_expr(value)?;

                if let Expr::Variable(name, _) = target {
                    if let Some(info) = self.lookup_variable(name) {
                        if info.is_const {
                            return Err(AlphaError::type_error(
                                format!("Cannot reassign constant variable '{}'", name),
                                *span,
                            )
                            .with_hint("Variables declared with 'const var' are immutable and cannot be reassigned"));
                        }
                    }
                } else if let Expr::Tuple(targets, _) = target {
                    for t in targets {
                        if let Expr::Variable(name, _) = t {
                            if let Some(info) = self.lookup_variable(name) {
                                if info.is_const {
                                    return Err(AlphaError::type_error(
                                        format!("Cannot reassign constant variable '{}'", name),
                                        *span,
                                    ));
                                }
                            }
                        }
                    }
                }

                if !val_ty.is_assignable_to(&target_ty) {
                    return Err(AlphaError::type_error(
                        format!(
                            "Cannot assign expression of type '{}' to target of type '{}'",
                            val_ty, target_ty
                        ),
                        *span,
                    )
                    .with_hint(format!("Target expects type '{}', but expression is '{}'", target_ty, val_ty)));
                }
                Ok(())
            }

            Statement::IncDec { target, span, .. } => {
                let ty = self.check_expr(target)?;
                if ty != Type::Int && ty != Type::Float {
                    return Err(AlphaError::type_error(
                        format!("Increment/decrement is only supported on numeric types, found '{}'", ty),
                        *span,
                    ));
                }
                if let Expr::Variable(name, _) = target {
                    if let Some(info) = self.lookup_variable(name) {
                        if info.is_const {
                            return Err(AlphaError::type_error(
                                format!("Cannot mutate constant '{}'", name),
                                *span,
                            ));
                        }
                    }
                }
                Ok(())
            }

            Statement::If {
                condition,
                then_branch,
                elseif_branches,
                else_branch,
                ..
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Boolean && cond_ty != Type::Something {
                    return Err(AlphaError::type_error(
                        format!("If condition must evaluate to 'boolean', found '{}'", cond_ty),
                        condition.span(),
                    ));
                }
                self.check_block(then_branch)?;

                for (elif_cond, elif_block) in elseif_branches {
                    let elif_ty = self.check_expr(elif_cond)?;
                    if elif_ty != Type::Boolean && elif_ty != Type::Something {
                        return Err(AlphaError::type_error(
                            format!("Elseif condition must evaluate to 'boolean', found '{}'", elif_ty),
                            elif_cond.span(),
                        ));
                    }
                    self.check_block(elif_block)?;
                }

                if let Some(else_b) = else_branch {
                    self.check_block(else_b)?;
                }
                Ok(())
            }

            Statement::ForInfinite { body, .. } => {
                self.check_block(body)?;
                Ok(())
            }

            Statement::ForCount {
                init,
                condition,
                step,
                body,
                ..
            } => {
                self.enter_scope();
                if let Some(init_s) = init {
                    if let Statement::Assign { target: Expr::Variable(name, _), value, .. } = &**init_s {
                        if self.lookup_variable(name).is_none() {
                            let ty = self.check_expr(value)?;
                            self.declare_variable(name, ty, false, Span::default())?;
                        } else {
                            self.check_statement(init_s)?;
                        }
                    } else {
                        self.check_statement(init_s)?;
                    }
                }
                if let Some(c) = condition {
                    let cond_ty = self.check_expr(c)?;
                    if cond_ty != Type::Boolean && cond_ty != Type::Something {
                        return Err(AlphaError::type_error(
                            format!("For condition must evaluate to 'boolean', found '{}'", cond_ty),
                            c.span(),
                        ));
                    }
                }
                if let Some(step_s) = step {
                    self.check_statement(step_s)?;
                }
                self.check_block(body)?;
                self.exit_scope();
                Ok(())
            }

            Statement::ForEach {
                item_var,
                key_var,
                collection,
                body,
                span,
            } => {
                let coll_ty = self.check_expr(collection)?;
                self.enter_scope();

                match &coll_ty {
                    Type::Vect(inner) => {
                        if key_var.is_some() {
                            return Err(AlphaError::type_error(
                                "Cannot unpack key and value from a vector; use 'for each item in vec'",
                                *span,
                            ));
                        }
                        self.declare_variable(item_var, *inner.clone(), false, *span)?;
                    }
                    Type::Map(k, v) => {
                        if let Some(k_var) = key_var {
                            self.declare_variable(k_var, *k.clone(), false, *span)?;
                            self.declare_variable(item_var, *v.clone(), false, *span)?;
                        } else {
                            // If only one var on map, it's the key or entry
                            self.declare_variable(item_var, *k.clone(), false, *span)?;
                        }
                    }
                    Type::Something => {
                        self.declare_variable(item_var, Type::Something, false, *span)?;
                        if let Some(k_var) = key_var {
                            self.declare_variable(k_var, Type::Something, false, *span)?;
                        }
                    }
                    _ => {
                        return Err(AlphaError::type_error(
                            format!("Cannot iterate over type '{}', expected vector or map", coll_ty),
                            collection.span(),
                        ));
                    }
                }

                self.check_block(body)?;
                self.exit_scope();
                Ok(())
            }

            Statement::Return { value, span } => {
                let expected = self
                    .current_return_type
                    .clone()
                    .unwrap_or(Type::Nothing);

                let actual = match value {
                    Some(e) => self.check_expr(e)?,
                    None => Type::Nothing,
                };

                if !actual.is_assignable_to(&expected) {
                    return Err(AlphaError::type_error(
                        format!(
                            "Invalid return type: expected '{}', got '{}'",
                            expected, actual
                        ),
                        *span,
                    )
                    .with_hint(format!("Function was declared to return '{}'", expected)));
                }
                Ok(())
            }

            Statement::TryCatch {
                try_block,
                err_var,
                catch_block,
                span,
            } => {
                self.check_block(try_block)?;
                self.enter_scope();
                self.declare_variable(err_var, Type::String, false, *span)?;
                self.check_block(catch_block)?;
                self.exit_scope();
                Ok(())
            }

            Statement::Break(_) | Statement::Continue(_) => Ok(()),

            Statement::System { target, body, .. } => {
                if matches_current_system(target) {
                    self.check_block(body)?;
                }
                Ok(())
            }

            Statement::Structure(st) => {
                self.structures.insert(st.name.clone(), st.clone());
                self.scopes.last_mut().unwrap().symbols.insert(
                    st.name.clone(),
                    SymbolInfo {
                        ty: Type::Custom(st.name.clone()),
                        is_const: true,
                    },
                );
                self.current_structure = Some(st.name.clone());
                for method in &st.methods {
                    self.enter_scope();
                    self.current_return_type = Some(method.return_type.clone());
                    if !method.is_static {
                        self.declare_variable("this", Type::Custom(st.name.clone()), false, method.span)?;
                    }
                    for param in &method.params {
                        self.declare_variable(&param.name, param.ty.clone(), false, param.span)?;
                    }
                    self.check_block(&method.body)?;
                    self.current_return_type = None;
                    self.exit_scope();
                }
                self.current_structure = None;
                Ok(())
            }

            Statement::Check {
                target,
                arms,
                span: _,
            } => {
                let target_ty = self.check_expr(target)?;
                for arm in arms {
                    self.enter_scope();
                    match &arm.pattern {
                        CheckPattern::Something => {
                            let unwrapped = target_ty.unwrapped_non_nothing().clone();
                            if let Some(binding) = &arm.binding {
                                self.declare_variable(binding, unwrapped, false, arm.span)?;
                            }
                        }
                        CheckPattern::Nothing | CheckPattern::Else => {}
                        CheckPattern::Expr(pat_expr) => {
                            self.check_expr(pat_expr)?;
                        }
                    }
                    match &arm.body {
                        CheckBody::Block(block) => self.check_block(block)?,
                        CheckBody::Expr(e) => {
                            self.check_expr(e)?;
                        }
                    }
                    self.exit_scope();
                }
                Ok(())
            }

            Statement::ForRange {
                var_name,
                start,
                end,
                inclusive: _,
                body,
                span,
            } => {
                let start_ty = self.check_expr(start)?;
                let end_ty = self.check_expr(end)?;
                if start_ty != Type::Int || end_ty != Type::Int {
                    return Err(AlphaError::type_error(
                        format!("Range loop bounds must be integers, got '{}' and '{}'", start_ty, end_ty),
                        *span,
                    ));
                }
                self.enter_scope();
                self.declare_variable(var_name, Type::Int, false, *span)?;
                self.check_block(body)?;
                self.exit_scope();
                Ok(())
            }

            Statement::Expr(e) => {
                self.check_expr(e)?;
                Ok(())
            }

            Statement::Block(b) => {
                self.enter_scope();
                self.check_block(b)?;
                self.exit_scope();
                Ok(())
            }

            Statement::Defer { statement, .. } => {
                self.check_statement(statement)?;
                Ok(())
            }

            Statement::Verify { condition, message, span } => {
                let cond_ty = self.check_expr(condition)?;
                if !cond_ty.is_assignable_to(&Type::Boolean) && cond_ty != Type::Something {
                    return Err(AlphaError::type_error(
                        format!("Verification condition must be boolean, got '{}'", cond_ty),
                        *span,
                    ));
                }
                if let Some(msg) = message {
                    self.check_expr(msg)?;
                }
                Ok(())
            }

            Statement::Enum(en) => {
                self.enums.insert(en.name.clone(), en.clone());
                if self.lookup_variable(&en.name).is_none() {
                    self.declare_variable(&en.name, Type::Custom(en.name.clone()), true, en.span)?;
                }
                Ok(())
            }

            Statement::Module(m) => {
                self.modules.insert(m.name.clone(), m.clone());
                if self.lookup_variable(&m.name).is_none() {
                    self.declare_variable(&m.name, Type::Custom(m.name.clone()), true, m.span)?;
                }
                for func in &m.functions {
                    let qname = format!("{}.{}", m.name, func.name);
                    self.functions.insert(
                        qname.clone(),
                        FuncSignature {
                            name: qname,
                            params: func.params.clone(),
                            visibility: func.visibility,
                            return_type: func.return_type.clone(),
                            defined_in: self.current_file.clone(),
                        },
                    );
                    self.enter_scope();
                    self.current_return_type = Some(func.return_type.clone());
                    for param in &func.params {
                        self.declare_variable(&param.name, param.ty.clone(), false, param.span)?;
                    }
                    self.check_block(&func.body)?;
                    self.current_return_type = None;
                    self.exit_scope();
                }
                Ok(())
            }
        }
    }

    pub fn check_expr(&mut self, expr: &Expr) -> Result<Type, AlphaError> {
        match expr {
            Expr::Literal(lit, _) => match lit {
                LiteralValue::Int(_) => Ok(Type::Int),
                LiteralValue::Float(_) => Ok(Type::Float),
                LiteralValue::String(_) => Ok(Type::String),
                LiteralValue::Char(_) => Ok(Type::Char),
                LiteralValue::Bool(_) => Ok(Type::Boolean),
                LiteralValue::Nothing => Ok(Type::Nothing),
            },

            Expr::Variable(name, span) => {
                if let Some(info) = self.lookup_variable(name) {
                    Ok(info.ty.clone())
                } else if self.lookup_function(name).is_some() {
                    Ok(Type::Something)
                } else {
                    Err(AlphaError::type_error(
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
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;

                let l_unwrapped = left_ty.unwrapped_non_nothing();
                let r_unwrapped = right_ty.unwrapped_non_nothing();

                match op {
                    BinaryOp::Add => {
                        if *l_unwrapped == Type::Int && *r_unwrapped == Type::Int {
                            Ok(Type::Int)
                        } else if *l_unwrapped == Type::Float && *r_unwrapped == Type::Float {
                            Ok(Type::Float)
                        } else if *l_unwrapped == Type::String || *r_unwrapped == Type::String {
                            Ok(Type::String)
                        } else if *l_unwrapped == Type::Something || *r_unwrapped == Type::Something {
                            Ok(Type::Something)
                        } else {
                            Err(AlphaError::type_error(
                                format!("Cannot add '{}' and '{}'", left_ty, right_ty),
                                *span,
                            ))
                        }
                    }
                    BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        if *l_unwrapped == Type::Int && *r_unwrapped == Type::Int {
                            Ok(Type::Int)
                        } else if *l_unwrapped == Type::Float && *r_unwrapped == Type::Float {
                            Ok(Type::Float)
                        } else if *l_unwrapped == Type::Something || *r_unwrapped == Type::Something {
                            Ok(Type::Something)
                        } else {
                            Err(AlphaError::type_error(
                                format!("Arithmetic operator not supported for '{}' and '{}'", left_ty, right_ty),
                                *span,
                            ))
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => Ok(Type::Boolean),
                    BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                        Ok(Type::Boolean)
                    }
                    BinaryOp::And => Ok(Type::Boolean),
                    BinaryOp::Or => {
                        if left_ty.allows_nothing() {
                            let unwrapped = left_ty.unwrapped_non_nothing().clone();
                            if right_ty.is_assignable_to(&unwrapped) {
                                Ok(unwrapped)
                            } else {
                                Ok(right_ty)
                            }
                        } else if left_ty == Type::Boolean && right_ty == Type::Boolean {
                            Ok(Type::Boolean)
                        } else {
                            Ok(right_ty)
                        }
                    }
                }
            }

            Expr::Unary { op, expr, span } => {
                let inner_ty = self.check_expr(expr)?;
                let u_inner = inner_ty.unwrapped_non_nothing();
                match op {
                    UnaryOp::Not => Ok(Type::Boolean),
                    UnaryOp::Neg | UnaryOp::Pos => {
                        if *u_inner == Type::Int {
                            Ok(Type::Int)
                        } else if *u_inner == Type::Float {
                            Ok(Type::Float)
                        } else if *u_inner == Type::Something {
                            Ok(Type::Something)
                        } else {
                            Err(AlphaError::type_error(
                                format!("Unary operator requires numeric type, found '{}'", inner_ty),
                                *span,
                            ))
                        }
                    }
                }
            }

            Expr::Call { callee, args, span } => {
                if let Expr::Variable(name, _) = &**callee {
                    if name == "printf" || name == "format" || name == "error" {
                        // Builtin variadic formatting functions
                        for arg in args {
                            self.check_expr(arg)?;
                        }
                        return if name == "format" {
                            Ok(Type::String)
                        } else {
                            Ok(Type::Nothing)
                        };
                    }

                    if name == "random" || name == "rand" {
                        for arg in args {
                            self.check_expr(arg)?;
                        }
                        return if args.is_empty() {
                            Ok(Type::Float)
                        } else {
                            Ok(Type::Int)
                        };
                    }

                    if let Some(func) = self.lookup_function(name).cloned() {
                        // Check visibility: here vs public
                        if func.visibility == Visibility::Here {
                            if let (Some(def_file), Some(cur_file)) = (&func.defined_in, &self.current_file) {
                                if def_file != cur_file {
                                    return Err(AlphaError::type_error(
                                        format!("Function '{}' is private ('here') to '{}'", name, def_file),
                                        *span,
                                    )
                                    .with_hint(format!("Mark function '{}' as '[public, ...]' to use it from other modules", name)));
                                }
                            }
                        }

                        if args.len() != func.params.len() {
                            return Err(AlphaError::type_error(
                                format!(
                                    "Function '{}' expects {} arguments, received {}",
                                    name,
                                    func.params.len(),
                                    args.len()
                                ),
                                *span,
                            ));
                        }

                        for (arg, param) in args.iter().zip(&func.params) {
                            let arg_ty = self.check_expr(arg)?;
                            if !arg_ty.is_assignable_to(&param.ty) {
                                return Err(AlphaError::type_error(
                                    format!(
                                        "Argument for parameter '{}' expects '{}', got '{}'",
                                        param.name, param.ty, arg_ty
                                    ),
                                    arg.span(),
                                ));
                            }
                        }

                        return Ok(func.return_type.clone());
                    }
                }

                // Generic call
                let callee_ty = self.check_expr(callee)?;
                for a in args {
                    self.check_expr(a)?;
                }
                if callee_ty == Type::Something {
                    Ok(Type::Something)
                } else {
                    Ok(Type::Nothing)
                }
            }

            Expr::MethodCall {
                object,
                method,
                args,
                span,
            } => {
                // Special check for .nothing?() method which exists on every type
                if method == "nothing?" {
                    self.check_expr(object)?;
                    return Ok(Type::Boolean);
                }

                let obj_ty = self.check_expr(object)?;

                match &obj_ty {
                    Type::Vect(inner) => match method.as_str() {
                        "push" => {
                            if args.len() != 1 {
                                return Err(AlphaError::type_error("push() expects 1 argument", *span));
                            }
                            let arg_ty = self.check_expr(&args[0])?;
                            if !arg_ty.is_assignable_to(inner) {
                                return Err(AlphaError::type_error(
                                    format!("Cannot push value of type '{}' into vector of '{}'", arg_ty, inner),
                                    args[0].span(),
                                ));
                            }
                            Ok(Type::Nothing)
                        }
                        "len" => Ok(Type::Int),
                        "get" => Ok(*inner.clone()),
                        "pop" => Ok(*inner.clone()),
                        _ => Ok(Type::Something),
                    },
                    Type::Map(k, v) => match method.as_str() {
                        "insert" => {
                            if args.len() != 2 {
                                return Err(AlphaError::type_error("insert() expects 2 arguments", *span));
                            }
                            let k_ty = self.check_expr(&args[0])?;
                            let v_ty = self.check_expr(&args[1])?;
                            if !k_ty.is_assignable_to(k) || !v_ty.is_assignable_to(v) {
                                return Err(AlphaError::type_error(
                                    format!("Key or value type mismatch in insert"),
                                    *span,
                                ));
                            }
                            Ok(Type::Nothing)
                        }
                        "contains" => {
                            if args.len() != 1 {
                                return Err(AlphaError::type_error("contains() expects 1 argument", *span));
                            }
                            let k_ty = self.check_expr(&args[0])?;
                            if !k_ty.is_assignable_to(k) {
                                return Err(AlphaError::type_error(
                                    format!("Map key type mismatch in contains()"),
                                    *span,
                                ));
                            }
                            Ok(Type::Boolean)
                        }
                        "get" => {
                            if args.len() != 1 {
                                return Err(AlphaError::type_error("get() expects 1 argument", *span));
                            }
                            // Returns V or nothing
                            Ok(Type::Union(vec![*v.clone(), Type::Nothing]))
                        }
                        _ => Ok(Type::Something),
                    },
                    Type::String => match method.as_str() {
                        "len" => Ok(Type::Int),
                        "empty?" => Ok(Type::Boolean),
                        _ => Ok(Type::Something),
                    },
                    Type::Custom(custom_name) => {
                        // Handle structures
                        let found_st = self.structures.get(custom_name)
                            .or_else(|| self.structures.keys().find(|k| k.replace('?', "") == custom_name.replace('?', "")).and_then(|k| self.structures.get(k)));
                        if let Some(st) = found_st.cloned() {
                            if let Some(m) = st.methods.iter().find(|m| m.name == *method || m.name.replace('?', "") == method.replace('?', "")) {
                                for a in args {
                                    self.check_expr(a)?;
                                }
                                return Ok(m.return_type.clone());
                            }
                        }

                        // Handle Io.File, Fs.Dir, Io, Fs
                        if custom_name == "console" && method == "out" {
                            if args.len() != 1 {
                                return Err(AlphaError::type_error("console.out() expects 1 argument", *span));
                            }
                            let arg_ty = self.check_expr(&args[0])?;
                            if !arg_ty.is_assignable_to(&Type::String) {
                                return Err(AlphaError::type_error(
                                    format!("console.out() argument must be string, found '{}'", arg_ty),
                                    args[0].span(),
                                    ));
                            }
                            return Ok(Type::String);
                        } else if custom_name == "random" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            let m = method.replace('?', "");
                            return match m.as_str() {
                                "int" => Ok(Type::Int),
                                "float" => Ok(Type::Float),
                                "bool" | "boolean" => Ok(Type::Boolean),
                                "choice" => Ok(Type::Something),
                                "string" => Ok(Type::String),
                                "seed" => Ok(Type::Nothing),
                                _ => Ok(Type::Something),
                            };
                        } else if custom_name == "time" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            return match method.as_str() {
                                "now" | "now_ms" | "elapsed" => Ok(Type::Int),
                                "sleep" => Ok(Type::Nothing),
                                _ => Ok(Type::Something),
                            };
                        } else if custom_name == "http" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            return match method.as_str() {
                                "get" | "post" => Ok(Type::String),
                                "download" => Ok(Type::Boolean),
                                _ => Ok(Type::Something),
                            };
                        } else if custom_name == "json" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            return match method.as_str() {
                                "parse" => Ok(Type::Something),
                                "stringify" => Ok(Type::String),
                                _ => Ok(Type::Something),
                            };
                        } else if custom_name == "Io" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            match method.as_str() {
                                "open" => Ok(Type::Custom("Io.File".to_string())),
                                "read" => Ok(Type::String),
                                "write" | "copy" | "move" | "rename" | "delete" => Ok(Type::Nothing),
                                "size" => Ok(Type::Int),
                                "exists" | "exists?" | "file?" | "is_file" => Ok(Type::Boolean),
                                _ => Ok(Type::Something),
                            }
                        } else if custom_name == "Fs" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            match method.as_str() {
                                "open" => Ok(Type::Custom("Fs.Dir".to_string())),
                                "join" | "extension" | "ext" | "filename" | "name" | "stem" | "parent" | "abs" | "absolute" => Ok(Type::String),
                                "exists" | "exists?" | "file?" | "is_file" | "dir?" | "is_dir" => Ok(Type::Boolean),
                                _ => Ok(Type::Something),
                            }
                        } else if custom_name == "Fs.Dir" && method == "open" {
                            Ok(Type::Custom("Fs.Dir".to_string()))
                        } else if custom_name == "Io.File" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            match method.as_str() {
                                "content" => Ok(Type::String),
                                "reset" => Ok(Type::Nothing),
                                "write" => Ok(Type::Nothing),
                                "something?" => Ok(Type::Boolean),
                                "empty?" => Ok(Type::Boolean),
                                "exists" | "exists?" => Ok(Type::Boolean),
                                "linelen" => Ok(Type::Int),
                                "symlen" => Ok(Type::Int),
                                "bytelen" | "size" => Ok(Type::Int),
                                "delete" => Ok(Type::Nothing),
                                _ => Ok(Type::Something),
                            }
                        } else if custom_name == "Fs.Dir" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            match method.as_str() {
                                "open" => Ok(Type::Custom("Fs.Dir".to_string())),
                                "delete" => Ok(Type::Nothing),
                                "create" => Ok(Type::Nothing),
                                "rename" => Ok(Type::Nothing),
                                "reset" => Ok(Type::Nothing),
                                "get_children" | "files" | "dirs" | "walk" => Ok(Type::Vect(Box::new(Type::String))),
                                "nothing?" => Ok(Type::Boolean),
                                "empty?" => Ok(Type::Boolean),
                                "exists" | "exists?" => Ok(Type::Boolean),
                                "join" => Ok(Type::String),
                                _ => Ok(Type::Something),
                            }
                        } else if custom_name == "cli" {
                            for a in args {
                                self.check_expr(a)?;
                            }
                            match method.as_str() {
                                "arguments" => Ok(Type::Vect(Box::new(Type::String))),
                                "paint" => Ok(Type::String),
                                _ => Ok(Type::Something),
                            }
                        } else {
                            let qname = format!("{}.{}", custom_name, method);
                            if let Some(func) = self.functions.get(&qname).cloned() {
                                if func.visibility == Visibility::Here && func.defined_in != self.current_file {
                                    return Err(AlphaError::type_error(
                                        format!("Cannot call private 'here' function '{}' from outside its file", qname),
                                        *span,
                                    ));
                                }
                                for a in args {
                                    self.check_expr(a)?;
                                }
                                Ok(func.return_type)
                            } else {
                                Ok(Type::Something)
                            }
                        }
                    }
                    _ => Ok(Type::Something),
                }
            }

            Expr::MemberAccess { object, member, span } => {
                let obj_ty = self.check_expr(object)?;
                if let Type::Custom(name) = &obj_ty {
                    if let Some(en) = self.enums.get(name) {
                        if en.variants.contains(member) {
                            return Ok(Type::Custom(name.clone()));
                        } else {
                            return Err(AlphaError::type_error(
                                format!("Variant '{}' not found in enum '{}'", member, name),
                                *span,
                            ));
                        }
                    }
                    if let Some(st) = self.structures.get(name) {
                        if let Some((_, f_ty)) = st.fields.iter().find(|(n, _)| n == member) {
                            return Ok(f_ty.clone());
                        }
                    }
                    if name == "Env" {
                        if member == "current" || member == "exec_dir" {
                            return Ok(Type::String);
                        }
                    } else if name == "Fs.Dir" && member == "file" {
                        return Ok(Type::Custom("Fs.DirFile".to_string()));
                    }
                }
                Ok(Type::Something)
            }

            Expr::Index { object, index, span } => {
                let obj_ty = self.check_expr(object)?;
                self.check_expr(index)?;
                match obj_ty {
                    Type::Vect(inner) => Ok(*inner),
                    Type::Map(_, v) => Ok(*v),
                    Type::Tuple(elements) => {
                        if let Expr::Literal(LiteralValue::Int(idx), _) = index.as_ref() {
                            let idx = *idx as usize;
                            if idx >= 1 && idx <= elements.len() {
                                Ok(elements[idx - 1].clone())
                            } else {
                                Err(AlphaError::type_error(
                                    format!("Tuple index {} is out of bounds (1-based, size {})", idx, elements.len()),
                                    *span,
                                ))
                            }
                        } else {
                            Ok(Type::Something)
                        }
                    }
                    _ => Ok(Type::Something),
                }
            }

            Expr::HexColor(_, _) => Ok(Type::HexColor),

            Expr::InterpolatedString(_, _)
            | Expr::MultilineString(_, _)
            | Expr::InterpolatedMultilineString(_, _) => Ok(Type::String),

            Expr::This(span) => {
                if let Some(info) = self.lookup_variable("this") {
                    Ok(info.ty.clone())
                } else if let Some(curr) = &self.current_structure {
                    Ok(Type::Custom(curr.clone()))
                } else {
                    Err(AlphaError::type_error(
                        "'this' can only be used inside structure methods",
                        *span,
                    ))
                }
            }

            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                span: _,
            } => {
                self.check_expr(condition)?;
                let then_ty = self.check_expr(then_expr)?;
                let else_ty = self.check_expr(else_expr)?;
                if then_ty == else_ty {
                    Ok(then_ty)
                } else if then_ty.is_assignable_to(&else_ty) {
                    Ok(else_ty)
                } else if else_ty.is_assignable_to(&then_ty) {
                    Ok(then_ty)
                } else {
                    Ok(Type::Union(vec![then_ty, else_ty]))
                }
            }

            Expr::Cast {
                expr,
                target_type,
                span: _,
            } => {
                self.check_expr(expr)?;
                Ok(target_type.clone())
            }

            Expr::ForceUnwrap { expr, span: _ } => {
                let ty = self.check_expr(expr)?;
                Ok(ty.unwrapped_non_nothing().clone())
            }

            Expr::Check {
                target,
                arms,
                span: _,
            } => {
                let target_ty = self.check_expr(target)?;
                let mut arm_types = Vec::new();
                for arm in arms {
                    self.enter_scope();
                    match &arm.pattern {
                        CheckPattern::Something => {
                            let unwrapped = target_ty.unwrapped_non_nothing().clone();
                            if let Some(binding) = &arm.binding {
                                self.declare_variable(binding, unwrapped, false, arm.span)?;
                            }
                        }
                        CheckPattern::Nothing | CheckPattern::Else => {}
                        CheckPattern::Expr(pat_expr) => {
                            self.check_expr(pat_expr)?;
                        }
                    }
                    let arm_ty = match &arm.body {
                        CheckBody::Block(b) => {
                            self.check_block(b)?;
                            Type::Something
                        }
                        CheckBody::Expr(e) => self.check_expr(e)?,
                    };
                    arm_types.push(arm_ty);
                    self.exit_scope();
                }
                if arm_types.is_empty() {
                    Ok(Type::Nothing)
                } else {
                    let first = &arm_types[0];
                    if arm_types.iter().all(|t| t == first) {
                        Ok(first.clone())
                    } else {
                        Ok(Type::Something)
                    }
                }
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
                    return Err(AlphaError::type_error(
                        "Cannot use 'make this' outside of a structure method",
                        *span,
                    ));
                };

                if let Some(st) = self.structures.get(&struct_name).cloned() {
                    for (f_name, f_expr) in fields {
                        let expr_ty = self.check_expr(f_expr)?;
                        if let Some((_, expected_ty)) = st.fields.iter().find(|(n, _)| n == f_name) {
                            if !expr_ty.is_assignable_to(expected_ty) {
                                return Err(AlphaError::type_error(
                                    format!(
                                        "Field '{}' of structure '{}' expected type '{}', got '{}'",
                                        f_name, struct_name, expected_ty, expr_ty
                                    ),
                                    f_expr.span(),
                                ));
                            }
                        }
                    }
                }
                Ok(Type::Custom(struct_name))
            }

            Expr::VectNew(_) => Ok(Type::Vect(Box::new(Type::Something))),
            Expr::MapNew(_) => Ok(Type::Map(Box::new(Type::Something), Box::new(Type::Something))),

            Expr::Tuple(elements, _) => {
                let mut tys = Vec::new();
                for el in elements {
                    tys.push(self.check_expr(el)?);
                }
                Ok(Type::Tuple(tys))
            }

            Expr::Slice { object, start, end, inclusive: _, span } => {
                let obj_ty = self.check_expr(object)?;
                if let Some(s) = start {
                    let sty = self.check_expr(s)?;
                    if sty != Type::Int && sty != Type::Something {
                        return Err(AlphaError::type_error("Slice start index must be an integer", *span));
                    }
                }
                if let Some(e) = end {
                    let ety = self.check_expr(e)?;
                    if ety != Type::Int && ety != Type::Something {
                        return Err(AlphaError::type_error("Slice end index must be an integer", *span));
                    }
                }
                match obj_ty {
                    Type::String => Ok(Type::String),
                    Type::Vect(inner) => Ok(Type::Vect(inner)),
                    Type::Something => Ok(Type::Something),
                    _ => Err(AlphaError::type_error(
                        format!("Slicing is only supported on string and vect, found '{}'", obj_ty),
                        *span,
                    )),
                }
            }
        }
    }
}
