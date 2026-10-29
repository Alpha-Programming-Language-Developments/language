// Direct LLVM IR Code Generator for the Alpha Programming Language.
// Emits clean, highly-optimizable SSA LLVM IR (.ll) directly from the typed AST.
use crate::diagnostics::error::AlphaError;
use crate::frontend::ast::*;
use crate::semantics::types::Type;
use std::collections::HashMap;

pub struct LlvmCodegen {
    reg_counter: usize,
    block_counter: usize,
    string_constants: Vec<(String, String)>, // (label, content)
    str_counter: usize,
    variables: HashMap<String, (String, String)>, // var_name -> (alloca_reg, llvm_type)
    loop_stack: Vec<(String, String)>, // (step_label, exit_label)
    current_fn_ret_type: String,
}

impl LlvmCodegen {
    pub fn new() -> Self {
        Self {
            reg_counter: 0,
            block_counter: 0,
            string_constants: Vec::new(),
            str_counter: 0,
            variables: HashMap::new(),
            loop_stack: Vec::new(),
            current_fn_ret_type: "void".to_string(),
        }
    }

    fn new_reg(&mut self) -> String {
        let r = format!("%{}", self.reg_counter);
        self.reg_counter += 1;
        r
    }

    fn new_block_label(&mut self, prefix: &str) -> String {
        let label = format!("{}.{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }

    pub fn to_llvm_type(ty: &Type) -> String {
        match ty {
            Type::Int | Type::Unt => "i64".to_string(),
            Type::Float => "double".to_string(),
            Type::Boolean => "i1".to_string(),
            Type::Char => "i32".to_string(),
            Type::Nothing => "void".to_string(),
            Type::String | Type::Vect(_) | Type::Map(_, _) | Type::HexColor => "ptr".to_string(),
            _ => "ptr".to_string(),
        }
    }

    /// Generates full LLVM IR module from Alpha AST.
    pub fn generate_module(&mut self, program: &Program) -> Result<String, AlphaError> {
        let mut fn_bodies = String::new();

        // Generate user functions
        for func in &program.functions {
            let body = self.generate_function(func)?;
            fn_bodies.push_str(&body);
            fn_bodies.push('\n');
        }

        // Generate module functions
        for module in &program.modules {
            for func in &module.functions {
                let mut f = func.clone();
                f.name = format!("{}_{}", module.name, func.name);
                let body = self.generate_function(&f)?;
                fn_bodies.push_str(&body);
                fn_bodies.push('\n');
            }
        }

        // Assemble module header
        let mut header = String::new();
        header.push_str("; ModuleID = 'alpha_program'\n");
        header.push_str("source_filename = \"program.alpha\"\n");
        if cfg!(windows) {
            header.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
        } else {
            header.push_str("target triple = \"x86_64-unknown-linux-gnu\"\n\n");
        }

        // Declare external C library functions
        header.push_str("declare i32 @printf(ptr, ...)\n");
        header.push_str("declare ptr @malloc(i64)\n");
        header.push_str("declare void @free(ptr)\n");
        header.push_str("declare void @exit(i32)\n");
        header.push_str("declare void @alpha_rt_print(ptr)\n");
        header.push_str("declare void @alpha_rt_println(ptr)\n");
        header.push_str("declare ptr @alpha_rt_cli_paint(ptr, ptr)\n");
        header.push_str("declare ptr @alpha_rt_cli_arguments()\n");
        header.push_str("declare i1 @alpha_rt_io_exists(ptr)\n");
        header.push_str("declare ptr @alpha_rt_io_read(ptr)\n");
        header.push_str("declare i1 @alpha_rt_io_write(ptr, ptr)\n");
        header.push_str("declare i64 @alpha_rt_time_now_ms()\n");
        header.push_str("declare i64 @alpha_rt_time_benchmark(ptr)\n");
        header.push_str("declare ptr @alpha_rt_vect_new()\n");
        header.push_str("declare void @alpha_rt_vect_push(ptr, ptr)\n");
        header.push_str("declare i64 @alpha_rt_vect_len(ptr)\n");
        header.push_str("declare ptr @alpha_rt_vect_get(ptr, i64)\n");
        header.push_str("declare void @alpha_rt_vect_free(ptr)\n");
        header.push_str("declare ptr @alpha_rt_format_str(ptr, ptr)\n\n");

        // Format strings
        header.push_str("@.fmt_int = private unnamed_addr constant [5 x i8] c\"%lld\\00\", align 1\n");
        header.push_str("@.fmt_int_nl = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\", align 1\n");
        header.push_str("@.fmt_float = private unnamed_addr constant [4 x i8] c\"%lf\\00\", align 1\n");
        header.push_str("@.fmt_float_nl = private unnamed_addr constant [5 x i8] c\"%lf\\0A\\00\", align 1\n");
        header.push_str("@.fmt_str = private unnamed_addr constant [3 x i8] c\"%s\\00\", align 1\n");
        header.push_str("@.fmt_str_nl = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n");
        header.push_str("@.fmt_nl = private unnamed_addr constant [2 x i8] c\"\\0A\\00\", align 1\n\n");

        // User string constants
        for (label, content) in &self.string_constants {
            let escaped = escape_llvm_string(content);
            let len = content.len() + 1; // including null terminator
            header.push_str(&format!("@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1\n", label, len, escaped));
        }
        header.push('\n');

        let mut final_code = header;
        final_code.push_str(&fn_bodies);
        Ok(final_code)
    }

    fn generate_function(&mut self, func: &FunctionDecl) -> Result<String, AlphaError> {
        self.reg_counter = 0;
        self.variables.clear();

        let ret_ty = Self::to_llvm_type(&func.return_type);
        self.current_fn_ret_type = ret_ty.clone();

        let fn_name = if func.name == "main" { "main" } else { &func.name };

        // Construct parameter list
        let mut param_defs = Vec::new();
        let mut param_allocas = Vec::new();

        for param in &func.params {
            let llvm_ty = Self::to_llvm_type(&param.ty);
            let param_reg = self.new_reg();
            param_defs.push(format!("{} {}", llvm_ty, param_reg));

            let alloca_reg = format!("%{}.addr", param.name);
            param_allocas.push((param.name.clone(), param_reg, alloca_reg, llvm_ty));
        }

        let mut body = String::new();
        body.push_str(&format!("define {} @{}({}) {{\n", ret_ty, fn_name, param_defs.join(", ")));
        body.push_str("entry:\n");

        // Allocate memory for parameters and store incoming arguments
        for (name, param_reg, alloca_reg, ty) in param_allocas {
            body.push_str(&format!("  {} = alloca {}, align 8\n", alloca_reg, ty));
            body.push_str(&format!("  store {} {}, ptr {}, align 8\n", ty, param_reg, alloca_reg));
            self.variables.insert(name, (alloca_reg, ty));
        }

        // Generate function body statements
        let mut terminated = false;
        for stmt in &func.body.statements {
            let (stmt_code, is_term) = self.generate_statement(stmt)?;
            body.push_str(&stmt_code);
            if is_term {
                terminated = true;
                break;
            }
        }

        if !terminated {
            if ret_ty == "void" {
                body.push_str("  ret void\n");
            } else if ret_ty == "i64" {
                body.push_str("  ret i64 0\n");
            } else if ret_ty == "double" {
                body.push_str("  ret double 0.0\n");
            } else if ret_ty == "i1" {
                body.push_str("  ret i1 0\n");
            } else {
                body.push_str(&format!("  ret {} null\n", ret_ty));
            }
        }

        body.push_str("}\n");
        Ok(body)
    }

    fn generate_statement(&mut self, stmt: &Statement) -> Result<(String, bool), AlphaError> {
        let mut code = String::new();

        match stmt {
            Statement::Var(var) => {
                let ty = Self::to_llvm_type(&var.ty);
                let alloca_reg = format!("%{}.addr", var.name);
                code.push_str(&format!("  {} = alloca {}, align 8\n", alloca_reg, ty));

                let (init_code, init_reg, init_ty) = self.generate_expr(&var.init)?;
                code.push_str(&init_code);

                // Type cast if necessary
                let final_reg = if ty != init_ty {
                    if ty == "double" && init_ty == "i64" {
                        let cast_reg = self.new_reg();
                        code.push_str(&format!("  {} = sitofp i64 {} to double\n", cast_reg, init_reg));
                        cast_reg
                    } else if ty == "ptr" && init_ty == "i64" {
                        let cast_reg = self.new_reg();
                        code.push_str(&format!("  {} = inttoptr i64 {} to ptr\n", cast_reg, init_reg));
                        cast_reg
                    } else if ty == "i64" && init_ty == "ptr" {
                        let cast_reg = self.new_reg();
                        code.push_str(&format!("  {} = ptrtoint ptr {} to i64\n", cast_reg, init_reg));
                        cast_reg
                    } else {
                        init_reg
                    }
                } else {
                    init_reg
                };

                code.push_str(&format!("  store {} {}, ptr {}, align 8\n", ty, final_reg, alloca_reg));
                self.variables.insert(var.name.clone(), (alloca_reg, ty));
                Ok((code, false))
            }

            Statement::Assign { target, op, value, span } => {
                let (val_code, val_reg, _val_ty) = self.generate_expr(value)?;
                code.push_str(&val_code);

                if let Expr::Variable(name, _) = target {
                    if let Some((alloca_reg, var_ty)) = self.variables.get(name).cloned() {
                        let final_reg = match op {
                            AssignOp::Assign => val_reg,
                            AssignOp::AddAssign => {
                                let cur_reg = self.new_reg();
                                code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", cur_reg, var_ty, alloca_reg));
                                let res_reg = self.new_reg();
                                if var_ty == "double" {
                                    code.push_str(&format!("  {} = fadd double {}, {}\n", res_reg, cur_reg, val_reg));
                                } else {
                                    code.push_str(&format!("  {} = add i64 {}, {}\n", res_reg, cur_reg, val_reg));
                                }
                                res_reg
                            }
                            AssignOp::SubAssign => {
                                let cur_reg = self.new_reg();
                                code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", cur_reg, var_ty, alloca_reg));
                                let res_reg = self.new_reg();
                                if var_ty == "double" {
                                    code.push_str(&format!("  {} = fsub double {}, {}\n", res_reg, cur_reg, val_reg));
                                } else {
                                    code.push_str(&format!("  {} = sub i64 {}, {}\n", res_reg, cur_reg, val_reg));
                                }
                                res_reg
                            }
                            AssignOp::MulAssign => {
                                let cur_reg = self.new_reg();
                                code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", cur_reg, var_ty, alloca_reg));
                                let res_reg = self.new_reg();
                                if var_ty == "double" {
                                    code.push_str(&format!("  {} = fmul double {}, {}\n", res_reg, cur_reg, val_reg));
                                } else {
                                    code.push_str(&format!("  {} = mul i64 {}, {}\n", res_reg, cur_reg, val_reg));
                                }
                                res_reg
                            }
                            AssignOp::DivAssign => {
                                let cur_reg = self.new_reg();
                                code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", cur_reg, var_ty, alloca_reg));
                                let res_reg = self.new_reg();
                                if var_ty == "double" {
                                    code.push_str(&format!("  {} = fdiv double {}, {}\n", res_reg, cur_reg, val_reg));
                                } else {
                                    code.push_str(&format!("  {} = sdiv i64 {}, {}\n", res_reg, cur_reg, val_reg));
                                }
                                res_reg
                            }
                            AssignOp::ModAssign => {
                                let cur_reg = self.new_reg();
                                code.push_str(&format!("  {} = load i64, ptr {}, align 8\n", cur_reg, alloca_reg));
                                let res_reg = self.new_reg();
                                code.push_str(&format!("  {} = srem i64 {}, {}\n", res_reg, cur_reg, val_reg));
                                res_reg
                            }
                        };
                        code.push_str(&format!("  store {} {}, ptr {}, align 8\n", var_ty, final_reg, alloca_reg));
                        Ok((code, false))
                    } else {
                        Err(AlphaError::compile(format!("Undefined variable '{}' in assignment", name), *span))
                    }
                } else {
                    Err(AlphaError::compile("Complex lvalues not yet supported in LLVM backend", *span))
                }
            }

            Statement::IncDec { target, is_inc, span } => {
                if let Expr::Variable(name, _) = target {
                    if let Some((alloca_reg, var_ty)) = self.variables.get(name).cloned() {
                        let cur_reg = self.new_reg();
                        code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", cur_reg, var_ty, alloca_reg));
                        let next_reg = self.new_reg();
                        let op_str = if *is_inc { "add" } else { "sub" };
                        code.push_str(&format!("  {} = {} i64 {}, 1\n", next_reg, op_str, cur_reg));
                        code.push_str(&format!("  store i64 {}, ptr {}, align 8\n", next_reg, alloca_reg));
                        Ok((code, false))
                    } else {
                        Err(AlphaError::compile(format!("Undefined variable '{}'", name), *span))
                    }
                } else {
                    Err(AlphaError::compile("Inc/dec only supported on variables", *span))
                }
            }

            Statement::If { condition, then_branch, elseif_branches, else_branch, .. } => {
                let (cond_code, cond_reg, _) = self.generate_expr(condition)?;
                code.push_str(&cond_code);

                let then_lbl = self.new_block_label("if.then");
                let end_lbl = self.new_block_label("if.end");
                let mut next_lbl = if !elseif_branches.is_empty() || else_branch.is_some() {
                    self.new_block_label("if.else")
                } else {
                    end_lbl.clone()
                };

                code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_reg, then_lbl, next_lbl));

                // Then block
                code.push_str(&format!("{}:\n", then_lbl));
                let mut then_term = false;
                for s in &then_branch.statements {
                    let (s_code, is_t) = self.generate_statement(s)?;
                    code.push_str(&s_code);
                    if is_t { then_term = true; break; }
                }
                if !then_term {
                    code.push_str(&format!("  br label %{}\n", end_lbl));
                }

                // Else if blocks
                for (elif_cond, elif_body) in elseif_branches {
                    code.push_str(&format!("{}:\n", next_lbl));
                    let (c_code, c_reg, _) = self.generate_expr(elif_cond)?;
                    code.push_str(&c_code);

                    let sub_then = self.new_block_label("elif.then");
                    let sub_next = self.new_block_label("elif.next");

                    code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", c_reg, sub_then, sub_next));
                    code.push_str(&format!("{}:\n", sub_then));

                    let mut sub_term = false;
                    for s in &elif_body.statements {
                        let (s_code, is_t) = self.generate_statement(s)?;
                        code.push_str(&s_code);
                        if is_t { sub_term = true; break; }
                    }
                    if !sub_term {
                        code.push_str(&format!("  br label %{}\n", end_lbl));
                    }
                    next_lbl = sub_next;
                }

                // Else block
                if let Some(else_b) = else_branch {
                    code.push_str(&format!("{}:\n", next_lbl));
                    let mut else_term = false;
                    for s in &else_b.statements {
                        let (s_code, is_t) = self.generate_statement(s)?;
                        code.push_str(&s_code);
                        if is_t { else_term = true; break; }
                    }
                    if !else_term {
                        code.push_str(&format!("  br label %{}\n", end_lbl));
                    }
                }

                code.push_str(&format!("{}:\n", end_lbl));
                Ok((code, false))
            }

            Statement::ForRange { var_name, start, end, inclusive, body, .. } => {
                let (start_code, start_reg, _) = self.generate_expr(start)?;
                let (end_code, end_reg, _) = self.generate_expr(end)?;
                code.push_str(&start_code);
                code.push_str(&end_code);

                let iter_name = var_name.clone().unwrap_or_else(|| "_iter".to_string());
                let alloca_reg = format!("%{}.addr", iter_name);
                code.push_str(&format!("  {} = alloca i64, align 8\n", alloca_reg));
                code.push_str(&format!("  store i64 {}, ptr {}, align 8\n", start_reg, alloca_reg));
                self.variables.insert(iter_name, (alloca_reg.clone(), "i64".to_string()));

                let cond_lbl = self.new_block_label("for.cond");
                let body_lbl = self.new_block_label("for.body");
                let step_lbl = self.new_block_label("for.step");
                let end_lbl = self.new_block_label("for.end");

                code.push_str(&format!("  br label %{}\n", cond_lbl));

                // Condition block
                code.push_str(&format!("{}:\n", cond_lbl));
                let cur_reg = self.new_reg();
                code.push_str(&format!("  {} = load i64, ptr {}, align 8\n", cur_reg, alloca_reg));
                let cmp_reg = self.new_reg();
                let cmp_op = if *inclusive { "sle" } else { "slt" };
                code.push_str(&format!("  {} = icmp {} i64 {}, {}\n", cmp_reg, cmp_op, cur_reg, end_reg));
                code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cmp_reg, body_lbl, end_lbl));

                // Body block
                code.push_str(&format!("{}:\n", body_lbl));
                self.loop_stack.push((step_lbl.clone(), end_lbl.clone()));
                let mut body_term = false;
                for s in &body.statements {
                    let (s_code, is_t) = self.generate_statement(s)?;
                    code.push_str(&s_code);
                    if is_t { body_term = true; break; }
                }
                self.loop_stack.pop();
                if !body_term {
                    code.push_str(&format!("  br label %{}\n", step_lbl));
                }

                // Step block: i = i + 1
                code.push_str(&format!("{}:\n", step_lbl));
                let c_reg = self.new_reg();
                code.push_str(&format!("  {} = load i64, ptr {}, align 8\n", c_reg, alloca_reg));
                let next_reg = self.new_reg();
                code.push_str(&format!("  {} = add i64 {}, 1\n", next_reg, c_reg));
                code.push_str(&format!("  store i64 {}, ptr {}, align 8\n", next_reg, alloca_reg));
                code.push_str(&format!("  br label %{}\n", cond_lbl));

                code.push_str(&format!("{}:\n", end_lbl));
                Ok((code, false))
            }

            Statement::Return { value, .. } => {
                if let Some(val) = value {
                    if self.current_fn_ret_type == "void" {
                        let (v_code, _, _) = self.generate_expr(val)?;
                        code.push_str(&v_code);
                        code.push_str("  ret void\n");
                    } else {
                        let (v_code, v_reg, v_ty) = self.generate_expr(val)?;
                        code.push_str(&v_code);
                        code.push_str(&format!("  ret {} {}\n", v_ty, v_reg));
                    }
                } else {
                    code.push_str("  ret void\n");
                }
                Ok((code, true))
            }

            Statement::Break(span) => {
                if let Some((_, end_lbl)) = self.loop_stack.last() {
                    code.push_str(&format!("  br label %{}\n", end_lbl));
                    Ok((code, true))
                } else {
                    Err(AlphaError::compile("break outside loop", *span))
                }
            }

            Statement::Continue(span) => {
                if let Some((step_lbl, _)) = self.loop_stack.last() {
                    code.push_str(&format!("  br label %{}\n", step_lbl));
                    Ok((code, true))
                } else {
                    Err(AlphaError::compile("continue outside loop", *span))
                }
            }

            Statement::Expr(expr) => {
                let (e_code, _, _) = self.generate_expr(expr)?;
                code.push_str(&e_code);
                Ok((code, false))
            }

            Statement::TryCatch { try_block, .. } => {
                let mut term = false;
                for s in &try_block.statements {
                    let (s_code, is_t) = self.generate_statement(s)?;
                    code.push_str(&s_code);
                    if is_t { term = true; break; }
                }
                Ok((code, term))
            }

            _ => Ok((code, false)),
        }
    }

    fn generate_expr(&mut self, expr: &Expr) -> Result<(String, String, String), AlphaError> {
        let mut code = String::new();

        match expr {
            Expr::Literal(lit, _) => match lit {
                LiteralValue::Int(v) => Ok((code, v.to_string(), "i64".to_string())),
                LiteralValue::Float(v) => Ok((code, format!("{:.6}", v), "double".to_string())),
                LiteralValue::Bool(b) => Ok((code, if *b { "1".to_string() } else { "0".to_string() }, "i1".to_string())),
                LiteralValue::Char(c) => Ok((code, (*c as u32).to_string(), "i32".to_string())),
                LiteralValue::Nothing => Ok((code, "0".to_string(), "i64".to_string())),
                LiteralValue::String(s) => {
                    let label = format!(".str.{}", self.str_counter);
                    self.str_counter += 1;
                    self.string_constants.push((label.clone(), s.clone()));

                    let ptr_reg = self.new_reg();
                    let len = s.len() + 1;
                    code.push_str(&format!("  {} = getelementptr inbounds [{} x i8], ptr @{}, i64 0, i64 0\n", ptr_reg, len, label));
                    Ok((code, ptr_reg, "ptr".to_string()))
                }
            },

            Expr::HexColor(s, _) => {
                let label = format!(".str.{}", self.str_counter);
                self.str_counter += 1;
                self.string_constants.push((label.clone(), s.clone()));

                let ptr_reg = self.new_reg();
                let len = s.len() + 1;
                code.push_str(&format!("  {} = getelementptr inbounds [{} x i8], ptr @{}, i64 0, i64 0\n", ptr_reg, len, label));
                Ok((code, ptr_reg, "ptr".to_string()))
            }

            Expr::Variable(name, span) => {
                if let Some((alloca_reg, ty)) = self.variables.get(name).cloned() {
                    let load_reg = self.new_reg();
                    code.push_str(&format!("  {} = load {}, ptr {}, align 8\n", load_reg, ty, alloca_reg));
                    Ok((code, load_reg, ty))
                } else {
                    Err(AlphaError::compile(format!("Undefined variable '{}' in LLVM backend", name), *span))
                }
            }

            Expr::Unary { op, expr, .. } => {
                let (e_code, e_reg, ty) = self.generate_expr(expr)?;
                code.push_str(&e_code);
                let res_reg = self.new_reg();

                match op {
                    UnaryOp::Neg => {
                        if ty == "double" {
                            code.push_str(&format!("  {} = fneg double {}\n", res_reg, e_reg));
                        } else {
                            code.push_str(&format!("  {} = sub i64 0, {}\n", res_reg, e_reg));
                        }
                    }
                    UnaryOp::Pos => return Ok((code, e_reg, ty)),
                    UnaryOp::Not => {
                        code.push_str(&format!("  {} = xor i1 {}, 1\n", res_reg, e_reg));
                    }
                }
                Ok((code, res_reg, ty))
            }

            Expr::Binary { left, op, right, .. } => {
                let (l_code, l_reg, l_ty) = self.generate_expr(left)?;
                let (r_code, r_reg, r_ty) = self.generate_expr(right)?;
                code.push_str(&l_code);
                code.push_str(&r_code);

                let is_float = l_ty == "double" || r_ty == "double";
                let res_reg = self.new_reg();

                match op {
                    BinaryOp::Add => {
                        if is_float {
                            code.push_str(&format!("  {} = fadd double {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "double".to_string()))
                        } else {
                            code.push_str(&format!("  {} = add i64 {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "i64".to_string()))
                        }
                    }
                    BinaryOp::Sub => {
                        if is_float {
                            code.push_str(&format!("  {} = fsub double {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "double".to_string()))
                        } else {
                            code.push_str(&format!("  {} = sub i64 {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "i64".to_string()))
                        }
                    }
                    BinaryOp::Mul => {
                        if is_float {
                            code.push_str(&format!("  {} = fmul double {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "double".to_string()))
                        } else {
                            code.push_str(&format!("  {} = mul i64 {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "i64".to_string()))
                        }
                    }
                    BinaryOp::Div => {
                        if is_float {
                            code.push_str(&format!("  {} = fdiv double {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "double".to_string()))
                        } else {
                            code.push_str(&format!("  {} = sdiv i64 {}, {}\n", res_reg, l_reg, r_reg));
                            Ok((code, res_reg, "i64".to_string()))
                        }
                    }
                    BinaryOp::Mod => {
                        code.push_str(&format!("  {} = srem i64 {}, {}\n", res_reg, l_reg, r_reg));
                        Ok((code, res_reg, "i64".to_string()))
                    }
                    BinaryOp::Equal => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp oeq double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp eq i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::NotEqual => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp one double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp ne i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::Less => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp olt double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp slt i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::LessEqual => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp ole double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp sle i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::Greater => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp ogt double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp sgt i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::GreaterEqual => {
                        if is_float {
                            code.push_str(&format!("  {} = fcmp oge double {}, {}\n", res_reg, l_reg, r_reg));
                        } else {
                            code.push_str(&format!("  {} = icmp sge i64 {}, {}\n", res_reg, l_reg, r_reg));
                        }
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::And => {
                        code.push_str(&format!("  {} = and i1 {}, {}\n", res_reg, l_reg, r_reg));
                        Ok((code, res_reg, "i1".to_string()))
                    }
                    BinaryOp::Or => {
                        code.push_str(&format!("  {} = or i1 {}, {}\n", res_reg, l_reg, r_reg));
                        Ok((code, res_reg, "i1".to_string()))
                    }
                }
            }

            Expr::Call { callee, args, .. } => {
                if let Expr::Variable(name, _) = &**callee {
                    if name == "format" {
                        if args.len() >= 2 {
                            let (f_code, f_reg, _) = self.generate_expr(&args[0])?;
                            let (a_code, a_reg, _) = self.generate_expr(&args[1])?;
                            code.push_str(&f_code);
                            code.push_str(&a_code);
                            let res_reg = self.new_reg();
                            code.push_str(&format!("  {} = call ptr @alpha_rt_format_str(ptr {}, ptr {})\n", res_reg, f_reg, a_reg));
                            return Ok((code, res_reg, "ptr".to_string()));
                        }
                    }

                    if name == "print" || name == "println" {
                        let is_nl = name == "println";
                        if let Some(arg) = args.first() {
                            let (a_code, a_reg, a_ty) = self.generate_expr(arg)?;
                            code.push_str(&a_code);

                            if a_ty == "ptr" {
                                if is_nl {
                                    code.push_str(&format!("  call void @alpha_rt_println(ptr {})\n", a_reg));
                                } else {
                                    code.push_str(&format!("  call void @alpha_rt_print(ptr {})\n", a_reg));
                                }
                                return Ok((code, "0".to_string(), "i64".to_string()));
                            }

                            let fmt_ptr = if a_ty == "i64" {
                                if is_nl { "@.fmt_int_nl" } else { "@.fmt_int" }
                            } else if a_ty == "double" {
                                if is_nl { "@.fmt_float_nl" } else { "@.fmt_float" }
                            } else {
                                if is_nl { "@.fmt_str_nl" } else { "@.fmt_str" }
                            };

                            let call_reg = self.new_reg();
                            code.push_str(&format!("  {} = call i32 (ptr, ...) @printf(ptr {}, {} {})\n", call_reg, fmt_ptr, a_ty, a_reg));
                            return Ok((code, "0".to_string(), "i64".to_string()));
                        } else if is_nl {
                            let call_reg = self.new_reg();
                            code.push_str(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.fmt_nl)\n", call_reg));
                            return Ok((code, "0".to_string(), "i64".to_string()));
                        }
                    }

                    // User function call
                    let mut arg_parts = Vec::new();
                    for a in args {
                        let (a_code, a_reg, a_ty) = self.generate_expr(a)?;
                        code.push_str(&a_code);
                        arg_parts.push(format!("{} {}", a_ty, a_reg));
                    }

                    let call_reg = self.new_reg();
                    code.push_str(&format!("  {} = call i64 @{}({})\n", call_reg, name, arg_parts.join(", ")));
                    return Ok((code, call_reg, "i64".to_string()));
                }

                Ok((code, "0".to_string(), "i64".to_string()))
            }

            Expr::Index { object, index, .. } => {
                let (obj_code, obj_reg, _) = self.generate_expr(object)?;
                let (idx_code, idx_reg, _) = self.generate_expr(index)?;
                code.push_str(&obj_code);
                code.push_str(&idx_code);
                let res_reg = self.new_reg();
                code.push_str(&format!("  {} = call ptr @alpha_rt_vect_get(ptr {}, i64 {})\n", res_reg, obj_reg, idx_reg));
                Ok((code, res_reg, "ptr".to_string()))
            }

            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                let (c_code, c_reg, _) = self.generate_expr(condition)?;
                let (t_code, t_reg, t_ty) = self.generate_expr(then_expr)?;
                let (e_code, e_reg, _) = self.generate_expr(else_expr)?;

                code.push_str(&c_code);
                code.push_str(&t_code);
                code.push_str(&e_code);

                let res_reg = self.new_reg();
                code.push_str(&format!("  {} = select i1 {}, {} {}, {} {}\n", res_reg, c_reg, t_ty, t_reg, t_ty, e_reg));
                Ok((code, res_reg, t_ty))
            }

            Expr::MethodCall { object, method, args, span } => {
                let is_static_mod = if let Expr::Variable(name, _) = &**object {
                    name == "cli" || name == "Io" || name == "Time" || name == "time"
                } else {
                    false
                };

                let (obj_code, obj_reg) = if is_static_mod {
                    (String::new(), "null".to_string())
                } else {
                    let (c, r, _) = self.generate_expr(object)?;
                    (c, r)
                };
                code.push_str(&obj_code);

                match method.as_str() {
                    "arguments" => {
                        let res_reg = self.new_reg();
                        code.push_str(&format!("  {} = call ptr @alpha_rt_cli_arguments()\n", res_reg));
                        Ok((code, res_reg, "ptr".to_string()))
                    }
                    "paint" => {
                        if args.len() >= 2 {
                            let (t_code, t_reg, _) = self.generate_expr(&args[0])?;
                            let (c_code, c_reg, _) = self.generate_expr(&args[1])?;
                            code.push_str(&t_code);
                            code.push_str(&c_code);
                            let res_reg = self.new_reg();
                            code.push_str(&format!("  {} = call ptr @alpha_rt_cli_paint(ptr {}, ptr {})\n", res_reg, t_reg, c_reg));
                            Ok((code, res_reg, "ptr".to_string()))
                        } else {
                            Ok((code, obj_reg, "ptr".to_string()))
                        }
                    }
                    "len" => {
                        let res_reg = self.new_reg();
                        code.push_str(&format!("  {} = call i64 @alpha_rt_vect_len(ptr {})\n", res_reg, obj_reg));
                        Ok((code, res_reg, "i64".to_string()))
                    }
                    "exists?" | "exists" => {
                        if let Some(arg) = args.first() {
                            let (a_code, a_reg, _) = self.generate_expr(arg)?;
                            code.push_str(&a_code);
                            let res_reg = self.new_reg();
                            code.push_str(&format!("  {} = call i1 @alpha_rt_io_exists(ptr {})\n", res_reg, a_reg));
                            Ok((code, res_reg, "i1".to_string()))
                        } else {
                            Ok((code, "0".to_string(), "i1".to_string()))
                        }
                    }
                    "read" => {
                        if let Some(arg) = args.first() {
                            let (a_code, a_reg, _) = self.generate_expr(arg)?;
                            code.push_str(&a_code);
                            let res_reg = self.new_reg();
                            code.push_str(&format!("  {} = call ptr @alpha_rt_io_read(ptr {})\n", res_reg, a_reg));
                            Ok((code, res_reg, "ptr".to_string()))
                        } else {
                            Ok((code, "null".to_string(), "ptr".to_string()))
                        }
                    }
                    _ => Err(AlphaError::compile(format!("Method '{}' not supported in direct LLVM backend", method), *span)),
                }
            }

            _ => Ok((code, "0".to_string(), "i64".to_string())),
        }
    }
}

fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b == b'\n' {
            out.push_str("\\0A");
        } else if b == b'\t' {
            out.push_str("\\09");
        } else if b == b'\\' {
            out.push_str("\\5C");
        } else if b == b'"' {
            out.push_str("\\22");
        } else if (32..127).contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("\\{:02X}", b));
        }
    }
    out
}
