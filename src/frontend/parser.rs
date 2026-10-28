// Recursive-descent & Pratt parser for the Alpha language.
use crate::diagnostics::error::AlphaError;
use crate::diagnostics::Span;
use crate::frontend::ast::*;
use crate::frontend::token::{Token, TokenKind};
use crate::semantics::types::Type;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    /// Parse a complete Alpha program / file.
    pub fn parse_program(&mut self) -> Result<Program, AlphaError> {
        let mut imports = Vec::new();
        let mut statements = Vec::new();
        let mut functions = Vec::new();
        let mut structures = Vec::new();
        let mut enums = Vec::new();
        let mut modules = Vec::new();

        self.skip_semicolons();

        while !self.is_at_end() {
            if self.check(&TokenKind::Import) {
                imports.push(self.parse_import()?);
            } else if self.check(&TokenKind::Function) {
                functions.push(self.parse_function()?);
            } else if self.check(&TokenKind::Structure) {
                let st = self.parse_structure()?;
                structures.push(st.clone());
                statements.push(Statement::Structure(st));
            } else if self.check(&TokenKind::Enum) {
                let en = self.parse_enum()?;
                enums.push(en.clone());
                statements.push(Statement::Enum(en));
            } else if self.check(&TokenKind::Module) {
                let m = self.parse_module()?;
                modules.push(m.clone());
                statements.push(Statement::Module(m));
            } else if self.is_system_block() {
                self.parse_top_level_system_block(
                    &mut imports,
                    &mut functions,
                    &mut structures,
                    &mut statements,
                )?;
            } else {
                statements.push(self.parse_statement()?);
            }
            self.skip_semicolons();
        }

        Ok(Program {
            imports,
            statements,
            functions,
            structures,
            enums,
            modules,
        })
    }

    fn is_system_block(&self) -> bool {
        self.check(&TokenKind::Bang)
            && (self.check_next(&TokenKind::System)
                || matches!(self.tokens.get(self.current + 1).map(|t| &t.kind), Some(TokenKind::Ident(s)) if s == "system"))
    }

    fn parse_top_level_system_block(
        &mut self,
        imports: &mut Vec<ImportDecl>,
        functions: &mut Vec<FunctionDecl>,
        structures: &mut Vec<StructureDecl>,
        statements: &mut Vec<Statement>,
    ) -> Result<(), AlphaError> {
        self.expect(&TokenKind::Bang)?;
        self.advance(); // consume 'system'
        let target = if self.match_token(&TokenKind::LParen) {
            let t = self.expect_ident()?.text;
            self.expect(&TokenKind::RParen)?;
            t
        } else {
            self.expect_ident()?.text
        };

        self.expect(&TokenKind::LBrace)?;
        let is_active = matches_current_system(&target);

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            self.skip_semicolons();
            if self.check(&TokenKind::RBrace) {
                break;
            }
            if self.check(&TokenKind::Import) {
                let imp = self.parse_import()?;
                if is_active {
                    imports.push(imp);
                }
            } else if self.check(&TokenKind::Function) {
                let func = self.parse_function()?;
                if is_active {
                    functions.push(func);
                }
            } else if self.check(&TokenKind::Structure) {
                let st = self.parse_structure()?;
                if is_active {
                    structures.push(st.clone());
                    statements.push(Statement::Structure(st));
                }
            } else if self.is_system_block() {
                self.parse_top_level_system_block(imports, functions, structures, statements)?;
            } else {
                let stmt = self.parse_statement()?;
                if is_active {
                    statements.push(stmt);
                }
            }
            self.skip_semicolons();
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(())
    }

    // -------------------------------------------------------------
    // Imports
    // -------------------------------------------------------------
    fn parse_import(&mut self) -> Result<ImportDecl, AlphaError> {
        let start_span = self.expect(&TokenKind::Import)?.span;

        // Check for web::"url" - now restricted to packages.alpaks
        if self.check(&TokenKind::Web) && self.check_next(&TokenKind::ColonColon) {
            self.advance(); // consume 'web'
            self.advance(); // consume '::'
            let token = self.advance();
            if let TokenKind::StringLit(url) = token.kind {
                let span = start_span.merge(token.span);
                return Err(AlphaError::parse(
                    format!("Direct 'import web::\"{}\"' in source files is not allowed. Web packages must be declared in 'packages.alpaks' (e.g. \"my_pkg\" = import web::\"{}\") and imported via 'import my_pkg'.", url, url),
                    span,
                )
                .with_hint("Declare your package in packages.alpaks at the project root"));
            } else {
                return Err(AlphaError::parse(
                    "Expected string literal after 'web::'",
                    token.span,
                ));
            }
        }

        // Check for "package_name", (file_name).alpha or path/to/file.alpha or Core/Basic.alpha
        let mut path_str = String::new();
        let mut last_span = start_span;

        if let TokenKind::StringLit(s) = self.peek().kind.clone() {
            let tok = self.advance();
            path_str = s;
            last_span = tok.span;
        } else if self.match_token(&TokenKind::LParen) {
            // (file_name).alpha
            while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                let tok = self.advance();
                path_str.push_str(&tok.text);
                last_span = tok.span;
            }
            self.expect(&TokenKind::RParen)?;
            if self.match_token(&TokenKind::Dot) {
                path_str.push('.');
                let ext = self.expect_identifier_or_text()?;
                path_str.push_str(&ext);
            }
        } else {
            // path like Core/Basic.alpha or tools/wifi.alpha
            while !self.is_at_end() {
                let tok = self.peek();
                match &tok.kind {
                    TokenKind::Ident(_)
                    | TokenKind::Slash
                    | TokenKind::Dot
                    | TokenKind::Minus
                    | TokenKind::Schedule
                    | TokenKind::Verify
                    | TokenKind::System
                    | TokenKind::Enum
                    | TokenKind::Module
                    | TokenKind::Nothing
                    | TokenKind::TypeInt
            | TokenKind::TypeUnt
                    | TokenKind::TypeFloat
                    | TokenKind::TypeString
                    | TokenKind::TypeChar
                    | TokenKind::TypeBoolean
                    | TokenKind::TypeHexColor
                    | TokenKind::TypeSomething
                    | TokenKind::TypeNothing
                    | TokenKind::TypeVect
                    | TokenKind::TypeMap => {
                        let t = self.advance();
                        path_str.push_str(&t.text);
                        last_span = t.span;
                    }
                    _ => break,
                }
            }
        }

        if path_str.is_empty() {
            return Err(AlphaError::parse(
                "Expected import path",
                self.peek().span,
            ));
        }

        let target = if path_str.starts_with("Core/") || path_str.starts_with("Core\\") {
            ImportTarget::Core(path_str)
        } else {
            ImportTarget::Local(path_str)
        };

        Ok(ImportDecl {
            target,
            span: start_span.merge(last_span),
        })
    }

    // -------------------------------------------------------------
    // Variable Declarations
    // -------------------------------------------------------------
    fn parse_var_decl(&mut self, is_const: bool, start_span: Span) -> Result<VarDecl, AlphaError> {
        let name_tok = self.expect_ident()?;
        let (ty, has_explicit_type) = if self.match_token(&TokenKind::Colon) {
            (self.parse_type()?, true)
        } else {
            (Type::Something, false)
        };

        self.expect(&TokenKind::Equal)?;
        let init = self.parse_expr()?;
        let end_span = init.span();

        Ok(VarDecl {
            name: name_tok.text,
            is_const,
            ty,
            init,
            has_explicit_type,
            span: start_span.merge(end_span),
        })
    }

    fn parse_var_tuple_decl(&mut self, is_const: bool, start_span: Span) -> Result<Statement, AlphaError> {
        self.expect(&TokenKind::LParen)?;
        let mut names = Vec::new();
        let mut types = Vec::new();

        if !self.check(&TokenKind::RParen) {
            loop {
                let name = self.expect_ident()?.text;
                let elem_type = if self.match_token(&TokenKind::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                names.push(name);
                types.push(elem_type);

                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen)?;

        let overall_type = if self.match_token(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&TokenKind::Equal)?;
        let init = self.parse_expr()?;
        let span = start_span.merge(init.span());

        Ok(Statement::VarTuple {
            names,
            types,
            overall_type,
            init,
            is_const,
            span,
        })
    }

    // -------------------------------------------------------------
    // Function Declarations
    // -------------------------------------------------------------
    fn parse_function(&mut self) -> Result<FunctionDecl, AlphaError> {
        let start_span = self.expect(&TokenKind::Function)?.span;

        let name = self.expect_ident()?.text;
        self.expect(&TokenKind::LParen)?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let p_tok = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let p_ty = self.parse_type()?;
                let p_span = p_tok.span.merge(self.previous().span);
                params.push(Param {
                    name: p_tok.text,
                    ty: p_ty,
                    span: p_span,
                });

                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen)?;

        // Metadata brackets: [visibility, return_type] or [visibility] or [return_type]
        let mut visibility = Visibility::Here;
        let mut return_type = Type::Nothing;

        if self.match_token(&TokenKind::LBracket) {
            // First item could be visibility (here/public) or a type
            if self.match_token(&TokenKind::Here) {
                visibility = Visibility::Here;
                if self.match_token(&TokenKind::Comma) {
                    return_type = self.parse_type()?;
                }
            } else if self.match_token(&TokenKind::Public) {
                visibility = Visibility::Public;
                if self.match_token(&TokenKind::Comma) {
                    return_type = self.parse_type()?;
                }
            } else {
                // Must be return type directly
                return_type = self.parse_type()?;
            }
            self.expect(&TokenKind::RBracket)?;
        }

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(FunctionDecl {
            name,
            params,
            visibility,
            return_type,
            body,
            span: start_span.merge(end_span),
        })
    }

    // -------------------------------------------------------------
    // Structures
    // -------------------------------------------------------------
    fn parse_structure(&mut self) -> Result<StructureDecl, AlphaError> {
        let start_span = self.expect(&TokenKind::Structure)?.span;
        let name_tok = self.expect_ident()?;
        let name = name_tok.text;

        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(&TokenKind::Attach) {
                self.expect(&TokenKind::LBrace)?;
                self.skip_semicolons();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    methods.push(self.parse_attached_method(&name)?);
                    self.skip_semicolons();
                }
                self.expect(&TokenKind::RBrace)?;
            } else if self.check(&TokenKind::Function) {
                methods.push(self.parse_attached_method(&name)?);
            } else {
                // Field declaration: name: type
                let f_name = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let f_ty = self.parse_type()?;
                fields.push((f_name.text, f_ty));
                self.match_token(&TokenKind::Comma);
            }
            self.skip_semicolons();
        }
        let end_span = self.expect(&TokenKind::RBrace)?.span;

        Ok(StructureDecl {
            name,
            fields,
            methods,
            span: start_span.merge(end_span),
        })
    }

    fn parse_attached_method(&mut self, _struct_name: &str) -> Result<AttachedMethod, AlphaError> {
        let start_span = self.expect(&TokenKind::Function)?.span;
        let name_tok = self.expect_ident_or_keyword_name()?;
        let name = name_tok.text;

        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let p_tok = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let p_ty = self.parse_type()?;
                let p_span = p_tok.span.merge(self.previous().span);
                params.push(Param {
                    name: p_tok.text,
                    ty: p_ty,
                    span: p_span,
                });

                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen)?;

        // Method metadata: [(static/nonstatic), (public/here), (return_type)]
        let mut is_static = false;
        let mut visibility = Visibility::Public;
        let mut return_type = Type::Nothing;

        if self.match_token(&TokenKind::LBracket) {
            while !self.check(&TokenKind::RBracket) && !self.is_at_end() {
                if self.match_token(&TokenKind::Static) {
                    is_static = true;
                } else if self.match_token(&TokenKind::Nonstatic) {
                    is_static = false;
                } else if self.match_token(&TokenKind::Public) {
                    visibility = Visibility::Public;
                } else if self.match_token(&TokenKind::Here) {
                    visibility = Visibility::Here;
                } else {
                    return_type = self.parse_type()?;
                }
                self.match_token(&TokenKind::Comma);
            }
            self.expect(&TokenKind::RBracket)?;
        }

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(AttachedMethod {
            name,
            is_static,
            visibility,
            return_type,
            params,
            body,
            span: start_span.merge(end_span),
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, AlphaError> {
        let start_span = self.expect(&TokenKind::Enum)?.span;
        let name = self.expect_ident()?.text;
        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let v_name = self.expect_ident()?.text;
            variants.push(v_name);
            let _ = self.match_token(&TokenKind::Comma);
            self.skip_semicolons();
        }
        let end_span = self.expect(&TokenKind::RBrace)?.span;
        Ok(EnumDecl {
            name,
            variants,
            span: start_span.merge(end_span),
        })
    }

    fn parse_module(&mut self) -> Result<ModuleDecl, AlphaError> {
        let start_span = self.expect(&TokenKind::Module)?.span;
        let name = self.expect_ident()?.text;
        self.expect(&TokenKind::LBrace)?;
        let mut functions = Vec::new();
        let mut structures = Vec::new();
        let mut enums = Vec::new();
        let mut variables = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Function) {
                functions.push(self.parse_function()?);
            } else if self.check(&TokenKind::Structure) {
                structures.push(self.parse_structure()?);
            } else if self.check(&TokenKind::Enum) {
                enums.push(self.parse_enum()?);
            } else if self.check(&TokenKind::Var) || (self.check(&TokenKind::Const) && self.check_next(&TokenKind::Var)) {
                let is_const = self.match_token(&TokenKind::Const);
                let var_span = self.expect(&TokenKind::Var)?.span;
                variables.push(self.parse_var_decl(is_const, var_span)?);
            } else {
                return Err(AlphaError::parse(
                    format!("Unexpected token in module '{}': '{}'", name, self.peek().text),
                    self.peek().span,
                ));
            }
            self.skip_semicolons();
        }
        let end_span = self.expect(&TokenKind::RBrace)?.span;
        Ok(ModuleDecl {
            name,
            functions,
            structures,
            enums,
            variables,
            span: start_span.merge(end_span),
        })
    }

    // -------------------------------------------------------------
    // Types
    // -------------------------------------------------------------
    pub fn parse_type(&mut self) -> Result<Type, AlphaError> {
        // Union type starting with '<', e.g. <string, int>, <int, nothing>
        if self.match_token(&TokenKind::Less) {
            let mut union_types = Vec::new();
            loop {
                union_types.push(self.parse_type()?);
                if !self.match_token(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Greater)?;
            return Ok(Type::Union(union_types));
        }

        // Tuple type starting with '(', e.g. (int, string)
        if self.match_token(&TokenKind::LParen) {
            let mut tuple_types = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    tuple_types.push(self.parse_type()?);
                    if !self.match_token(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(&TokenKind::RParen)?;
            return Ok(Type::Tuple(tuple_types));
        }

        let tok = self.advance();
        match tok.kind {
            TokenKind::TypeInt => Ok(Type::Int),
            TokenKind::TypeUnt => Ok(Type::Unt),
            TokenKind::TypeFloat => Ok(Type::Float),
            TokenKind::TypeString => Ok(Type::String),
            TokenKind::TypeChar => Ok(Type::Char),
            TokenKind::TypeBoolean => Ok(Type::Boolean),
            TokenKind::TypeHexColor => Ok(Type::HexColor),
            TokenKind::TypeSomething => Ok(Type::Something),
            TokenKind::TypeNothing | TokenKind::Nothing => Ok(Type::Nothing),
            TokenKind::TypeVect => {
                self.expect(&TokenKind::Less)?;
                let inner = self.parse_type()?;
                self.expect(&TokenKind::Greater)?;
                Ok(Type::Vect(Box::new(inner)))
            }
            TokenKind::TypeMap => {
                self.expect(&TokenKind::Less)?;
                let k = self.parse_type()?;
                self.expect(&TokenKind::Comma)?;
                let v = self.parse_type()?;
                self.expect(&TokenKind::Greater)?;
                Ok(Type::Map(Box::new(k), Box::new(v)))
            }
            TokenKind::Ident(name) => {
                // Check if it's qualified type like Io.File, Fs.Dir
                if self.match_token(&TokenKind::Dot) {
                    let sub = self.expect_ident()?;
                    Ok(Type::Custom(format!("{}.{}", name, sub.text)))
                } else if name == "IoFile" {
                    Ok(Type::Custom("Io.File".to_string()))
                } else if name == "FsDir" {
                    Ok(Type::Custom("Fs.Dir".to_string()))
                } else {
                    Ok(Type::Custom(name))
                }
            }
            _ => Err(AlphaError::parse(
                format!("Expected type, found '{}'", tok.text),
                tok.span,
            )),
        }
    }

    // -------------------------------------------------------------
    // Blocks and Statements
    // -------------------------------------------------------------
    fn parse_block(&mut self) -> Result<Block, AlphaError> {
        let start_span = self.expect(&TokenKind::LBrace)?.span;
        let mut stmts = Vec::new();

        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_statement()?);
            self.skip_semicolons();
        }

        let end_span = self.expect(&TokenKind::RBrace)?.span;
        Ok(Block {
            statements: stmts,
            span: start_span.merge(end_span),
        })
    }

    fn parse_system_statement(&mut self) -> Result<Statement, AlphaError> {
        let start_span = self.expect(&TokenKind::Bang)?.span;
        self.advance(); // consume 'system'
        let target = if self.match_token(&TokenKind::LParen) {
            let t = self.expect_ident()?.text;
            self.expect(&TokenKind::RParen)?;
            t
        } else {
            self.expect_ident()?.text
        };
        let body = self.parse_block()?;
        let span = start_span.merge(body.span);
        Ok(Statement::System {
            target,
            body,
            span,
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, AlphaError> {
        if self.is_system_block() {
            return self.parse_system_statement();
        }

        if self.match_token(&TokenKind::AtStrict) {
            return Ok(Statement::StrictDirective(self.previous().span));
        }

        if self.check(&TokenKind::Structure) {
            let st = self.parse_structure()?;
            return Ok(Statement::Structure(st));
        }

        if self.check(&TokenKind::Enum) {
            let en = self.parse_enum()?;
            return Ok(Statement::Enum(en));
        }

        if self.check(&TokenKind::Module) {
            let m = self.parse_module()?;
            return Ok(Statement::Module(m));
        }

        if self.check(&TokenKind::Check) {
            return self.parse_check_statement();
        }

        if self.check(&TokenKind::Var) || (self.check(&TokenKind::Const) && self.check_next(&TokenKind::Var)) {
            let is_const = self.match_token(&TokenKind::Const);
            let var_span = self.expect(&TokenKind::Var)?.span;
            if self.check(&TokenKind::LParen) {
                return self.parse_var_tuple_decl(is_const, var_span);
            }
            let decl = self.parse_var_decl(is_const, var_span)?;
            return Ok(Statement::Var(decl));
        }

        if self.check(&TokenKind::If) {
            return self.parse_if();
        }

        if self.check(&TokenKind::Try) {
            return self.parse_try_catch();
        }

        if self.check(&TokenKind::For) {
            return self.parse_for();
        }

        if self.match_token(&TokenKind::Return) {
            let start_span = self.previous().span;
            if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RBrace) || self.is_at_end() {
                return Ok(Statement::Return {
                    value: None,
                    span: start_span,
                });
            }
            let val = self.parse_expr()?;
            let span = start_span.merge(val.span());
            return Ok(Statement::Return {
                value: Some(val),
                span,
            });
        }

        let is_schedule = if self.match_token(&TokenKind::Defer) {
            Some(self.previous().span)
        } else if self.check(&TokenKind::Make) && self.check_next(&TokenKind::Schedule) {
            let make_span = self.advance().span;
            let sched_span = self.advance().span;
            Some(make_span.merge(sched_span))
        } else {
            None
        };

        if let Some(start_span) = is_schedule {
            let deferred = if self.check(&TokenKind::LBrace) {
                let block = self.parse_block()?;
                Statement::Block(block)
            } else {
                self.parse_statement()?
            };
            let span = start_span.merge(deferred.span());
            return Ok(Statement::Defer {
                statement: Box::new(deferred),
                span,
            });
        }

        if self.match_token(&TokenKind::Verify) {
            let start_span = self.previous().span;
            let condition = self.parse_expr()?;
            let message = if self.match_token(&TokenKind::Comma) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let end_span = message.as_ref().map(|m| m.span()).unwrap_or_else(|| condition.span());
            return Ok(Statement::Verify {
                condition,
                message,
                span: start_span.merge(end_span),
            });
        }

        if self.check(&TokenKind::LBrace) {
            let block = self.parse_block()?;
            return Ok(Statement::Block(block));
        }

        if self.match_token(&TokenKind::Break) {
            return Ok(Statement::Break(self.previous().span));
        }

        if self.match_token(&TokenKind::Continue) {
            return Ok(Statement::Continue(self.previous().span));
        }

        // Could be Assignment, Inc/Dec, or Expression Statement
        let expr = self.parse_expr()?;

        // Check for assignments
        let assign_op = if self.match_token(&TokenKind::Equal) {
            Some(AssignOp::Assign)
        } else if self.match_token(&TokenKind::PlusEqual) {
            Some(AssignOp::AddAssign)
        } else if self.match_token(&TokenKind::MinusEqual) {
            Some(AssignOp::SubAssign)
        } else if self.match_token(&TokenKind::StarEqual) {
            Some(AssignOp::MulAssign)
        } else if self.match_token(&TokenKind::SlashEqual) {
            Some(AssignOp::DivAssign)
        } else if self.match_token(&TokenKind::PercentEqual) {
            Some(AssignOp::ModAssign)
        } else {
            None
        };

        if let Some(op) = assign_op {
            let val = self.parse_expr()?;
            let span = expr.span().merge(val.span());
            return Ok(Statement::Assign {
                target: expr,
                op,
                value: val,
                span,
            });
        }

        // Check for ++ or --
        if self.match_token(&TokenKind::PlusPlus) {
            let span = expr.span().merge(self.previous().span);
            return Ok(Statement::IncDec {
                target: expr,
                is_inc: true,
                span,
            });
        }
        if self.match_token(&TokenKind::MinusMinus) {
            let span = expr.span().merge(self.previous().span);
            return Ok(Statement::IncDec {
                target: expr,
                is_inc: false,
                span,
            });
        }

        Ok(Statement::Expr(expr))
    }

    fn parse_try_catch(&mut self) -> Result<Statement, AlphaError> {
        let start_span = self.expect(&TokenKind::Try)?.span;
        let try_block = self.parse_block()?;

        self.expect(&TokenKind::Catch).map_err(|e| {
            e.with_hint("A 'try' block must be followed by 'catch <error_variable> { ... }'")
        })?;
        let err_var = self.expect_ident().map_err(|e| {
            e.with_hint("Specify variable name to receive the error message, e.g. 'catch err { ... }'")
        })?.text;
        let catch_block = self.parse_block()?;
        let span = start_span.merge(catch_block.span);

        Ok(Statement::TryCatch {
            try_block,
            err_var,
            catch_block,
            span,
        })
    }

    fn parse_check_statement(&mut self) -> Result<Statement, AlphaError> {
        let start_span = self.expect(&TokenKind::Check)?.span;
        let target = self.parse_expr()?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            arms.push(self.parse_check_arm()?);
            self.match_token(&TokenKind::Comma);
            self.skip_semicolons();
        }
        let end_span = self.expect(&TokenKind::RBrace)?.span;

        Ok(Statement::Check {
            target,
            arms,
            span: start_span.merge(end_span),
        })
    }

    fn parse_check_expr(&mut self) -> Result<Expr, AlphaError> {
        let start_span = self.expect(&TokenKind::Check)?.span;
        let target = self.parse_or()?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        self.skip_semicolons();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            arms.push(self.parse_check_arm()?);
            self.match_token(&TokenKind::Comma);
            self.skip_semicolons();
        }
        let end_span = self.expect(&TokenKind::RBrace)?.span;

        Ok(Expr::Check {
            target: Box::new(target),
            arms,
            span: start_span.merge(end_span),
        })
    }

    fn parse_check_arm(&mut self) -> Result<CheckArm, AlphaError> {
        let start_pos = self.peek().span;
        let mut binding = None;

        let pattern = if self.match_token(&TokenKind::Else) || matches!(&self.peek().kind, TokenKind::Ident(s) if s == "_") {
            if matches!(&self.peek().kind, TokenKind::Ident(s) if s == "_") {
                self.advance();
            }
            CheckPattern::Else
        } else if self.match_token(&TokenKind::Nothing) || self.match_token(&TokenKind::TypeNothing) {
            CheckPattern::Nothing
        } else if self.match_token(&TokenKind::TypeSomething) {
            CheckPattern::Something
        } else if self.match_token(&TokenKind::LParen) {
            // e.g. (something) or (expr)
            if self.match_token(&TokenKind::TypeSomething) {
                self.expect(&TokenKind::RParen)?;
                CheckPattern::Something
            } else if matches!(&self.peek().kind, TokenKind::Ident(s) if s == "something") {
                self.advance();
                self.expect(&TokenKind::RParen)?;
                CheckPattern::Something
            } else {
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                CheckPattern::Expr(e)
            }
        } else if matches!(&self.peek().kind, TokenKind::Ident(s) if s == "something") {
            self.advance();
            CheckPattern::Something
        } else {
            let e = self.parse_equality()?;
            CheckPattern::Expr(e)
        };

        self.expect(&TokenKind::FatArrow)?;

        let body = if matches!(pattern, CheckPattern::Something)
            && matches!(&self.peek().kind, TokenKind::Ident(_))
            && (self.check_next(&TokenKind::Comma) || self.check_next(&TokenKind::RBrace))
        {
            let b_tok = self.advance();
            binding = Some(b_tok.text.clone());
            CheckBody::Expr(Expr::Variable(b_tok.text, b_tok.span))
        } else if self.check(&TokenKind::LBrace) {
            CheckBody::Block(self.parse_block()?)
        } else {
            CheckBody::Expr(self.parse_expr()?)
        };

        let span = match &body {
            CheckBody::Block(b) => start_pos.merge(b.span),
            CheckBody::Expr(e) => start_pos.merge(e.span()),
        };

        Ok(CheckArm {
            pattern,
            binding,
            body,
            span,
        })
    }

    fn parse_if(&mut self) -> Result<Statement, AlphaError> {
        let start_span = self.expect(&TokenKind::If)?.span;
        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;

        let mut elseif_branches = Vec::new();
        while self.match_token(&TokenKind::ElseIf) || (self.check(&TokenKind::Else) && self.check_next(&TokenKind::If)) {
            if self.previous().kind != TokenKind::ElseIf {
                self.advance(); // consume 'else'
                self.advance(); // consume 'if'
            }
            let elif_cond = self.parse_expr()?;
            let elif_block = self.parse_block()?;
            elseif_branches.push((elif_cond, elif_block));
        }

        let else_branch = if self.match_token(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let end_span = else_branch
            .as_ref()
            .map(|b| b.span)
            .or_else(|| elseif_branches.last().map(|(_, b)| b.span))
            .unwrap_or(then_branch.span);

        Ok(Statement::If {
            condition,
            then_branch,
            elseif_branches,
            else_branch,
            span: start_span.merge(end_span),
        })
    }

    fn parse_for(&mut self) -> Result<Statement, AlphaError> {
        let start_span = self.expect(&TokenKind::For)?.span;

        // Case 1: Infinite loop: 'for {'
        if self.check(&TokenKind::LBrace) {
            let body = self.parse_block()?;
            return Ok(Statement::ForInfinite {
                span: start_span.merge(body.span),
                body,
            });
        }

        // Case 2: For each: 'for each entry in variable {' or 'for each key, value in variable {'
        if self.match_token(&TokenKind::Each) {
            let first_var = self.expect_ident()?.text;
            let (item_var, key_var) = if self.match_token(&TokenKind::Comma) {
                let second_var = self.expect_ident()?.text;
                (second_var, Some(first_var))
            } else {
                (first_var, None)
            };

            self.expect(&TokenKind::In)?;
            let collection = self.parse_expr()?;
            let body = self.parse_block()?;

            return Ok(Statement::ForEach {
                item_var,
                key_var,
                collection,
                span: start_span.merge(body.span),
                body,
            });
        }

        // Case 2.5: Range loop or collection loop: 'for n in 1...3 {' or 'for x in list {'
        if (matches!(self.peek().kind, TokenKind::Ident(_)) || self.check(&TokenKind::In)) && self.check_next(&TokenKind::In) {
            let var_name = self.expect_ident()?.text;
            self.expect(&TokenKind::In)?;
            let start = self.parse_expr()?;
            if self.match_token(&TokenKind::DotDotDot) || self.match_token(&TokenKind::DotDot) {
                let inclusive = self.previous().kind == TokenKind::DotDotDot;
                let end = self.parse_expr()?;
                let body = self.parse_block()?;
                let body_span = body.span;
                return Ok(Statement::ForRange {
                    var_name: if var_name == "_" { None } else { Some(var_name) },
                    start,
                    end,
                    inclusive,
                    body,
                    span: start_span.merge(body_span),
                });
            } else {
                let body = self.parse_block()?;
                let body_span = body.span;
                return Ok(Statement::ForEach {
                    item_var: var_name,
                    key_var: None,
                    collection: start,
                    body,
                    span: start_span.merge(body_span),
                });
            }
        }

        // Case 2.6: Repeat range loop without variable: 'for 1...3 {' or 'for 1..3 {'
        if self.check_range_ahead_before_brace_or_sep() {
            let start = self.parse_expr()?;
            if self.match_token(&TokenKind::DotDotDot) || self.match_token(&TokenKind::DotDot) {
                let inclusive = self.previous().kind == TokenKind::DotDotDot;
                let end = self.parse_expr()?;
                let body = self.parse_block()?;
                let body_span = body.span;
                return Ok(Statement::ForRange {
                    var_name: None,
                    start,
                    end,
                    inclusive,
                    body,
                    span: start_span.merge(body_span),
                });
            }
        }

        // Case 3: Count loop: 'for i = 0, i < 5, i++ {' or 'for i = 0; i < 5; i++ {'
        // Can be separated by commas or semicolons
        let init_stmt = if self.check(&TokenKind::Comma) || self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(Box::new(self.parse_statement()?))
        };

        if self.match_token(&TokenKind::Comma) || self.match_token(&TokenKind::Semicolon) {
            // ok
        } else {
            return Err(AlphaError::parse(
                "Expected ',' or ';' after for-loop initializer",
                self.peek().span,
            ));
        }

        let condition = if self.check(&TokenKind::Comma) || self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        if self.match_token(&TokenKind::Comma) || self.match_token(&TokenKind::Semicolon) {
            // ok
        } else {
            return Err(AlphaError::parse(
                "Expected ',' or ';' after for-loop condition",
                self.peek().span,
            ));
        }

        let step_stmt = if self.check(&TokenKind::LBrace) {
            None
        } else {
            Some(Box::new(self.parse_statement()?))
        };

        let body = self.parse_block()?;

        Ok(Statement::ForCount {
            init: init_stmt,
            condition,
            step: step_stmt,
            body: body.clone(),
            span: start_span.merge(body.span),
        })
    }

    // -------------------------------------------------------------
    // Expressions (Pratt Parsing)
    // -------------------------------------------------------------
    pub fn parse_expr(&mut self) -> Result<Expr, AlphaError> {
        if self.check(&TokenKind::If) {
            let start_span = self.advance().span; // consume 'if'
            let condition = self.parse_or()?;
            self.expect(&TokenKind::Colon)?;
            let then_expr = self.parse_expr()?;
            self.expect(&TokenKind::Else)?;
            let else_expr = self.parse_expr()?;
            let span = start_span.merge(else_expr.span());
            return Ok(Expr::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            });
        }

        if self.check(&TokenKind::Check) {
            return self.parse_check_expr();
        }

        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_and()?;
        while self.match_token(&TokenKind::Or) || self.match_token(&TokenKind::OrOr) {
            let op = BinaryOp::Or;
            let right = self.parse_and()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_equality()?;
        while self.match_token(&TokenKind::And) || self.match_token(&TokenKind::AndAnd) {
            let op = BinaryOp::And;
            let right = self.parse_equality()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_comparison()?;
        while let Some(op) = self.match_equality_op() {
            let right = self.parse_comparison()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn match_equality_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&TokenKind::EqualEqual) {
            Some(BinaryOp::Equal)
        } else if self.match_token(&TokenKind::BangEqual) {
            Some(BinaryOp::NotEqual)
        } else {
            None
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_term()?;
        while let Some(op) = self.match_comparison_op() {
            let right = self.parse_term()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn match_comparison_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&TokenKind::Less) {
            Some(BinaryOp::Less)
        } else if self.match_token(&TokenKind::LessEqual) {
            Some(BinaryOp::LessEqual)
        } else if self.match_token(&TokenKind::Greater) {
            Some(BinaryOp::Greater)
        } else if self.match_token(&TokenKind::GreaterEqual) {
            Some(BinaryOp::GreaterEqual)
        } else {
            None
        }
    }

    fn parse_term(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_factor()?;
        while let Some(op) = self.match_term_op() {
            let right = self.parse_factor()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn match_term_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&TokenKind::Plus) {
            Some(BinaryOp::Add)
        } else if self.match_token(&TokenKind::Minus) {
            Some(BinaryOp::Sub)
        } else {
            None
        }
    }

    fn parse_factor(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_unary()?;
        while let Some(op) = self.match_factor_op() {
            let right = self.parse_unary()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn match_factor_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&TokenKind::Star) {
            Some(BinaryOp::Mul)
        } else if self.match_token(&TokenKind::Slash) {
            Some(BinaryOp::Div)
        } else if self.match_token(&TokenKind::Percent) {
            Some(BinaryOp::Mod)
        } else {
            None
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, AlphaError> {
        if self.match_token(&TokenKind::Not) || self.match_token(&TokenKind::Bang) {
            let start_span = self.previous().span;
            let expr = self.parse_unary()?;
            let span = start_span.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span,
            });
        }
        if self.match_token(&TokenKind::Minus) {
            let start_span = self.previous().span;
            let expr = self.parse_unary()?;
            let span = start_span.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
                span,
            });
        }
        if self.match_token(&TokenKind::Plus) {
            let start_span = self.previous().span;
            let expr = self.parse_unary()?;
            let span = start_span.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Pos,
                expr: Box::new(expr),
                span,
            });
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, AlphaError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&TokenKind::BangQuestion) {
                let span = expr.span().merge(self.previous().span);
                expr = Expr::ForceUnwrap {
                    expr: Box::new(expr),
                    span,
                };
                continue;
            }

            if self.match_token(&TokenKind::Dot) {
                // Member access or method call
                // Note: The method/field name can be an identifier, or 'nothing?', 'something?', 'empty?'
                let member_tok = self.expect_ident_or_keyword_name()?;
                let member = member_tok.text;

                if self.match_token(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end_span = self.expect(&TokenKind::RParen)?.span;
                    let span = expr.span().merge(end_span);

                    expr = Expr::MethodCall {
                        object: Box::new(expr),
                        method: member,
                        args,
                        span,
                    };
                } else {
                    let span = expr.span().merge(member_tok.span);
                    expr = Expr::MemberAccess {
                        object: Box::new(expr),
                        member,
                        span,
                    };
                }
            } else if self.check(&TokenKind::LParen) {
                if self.peek().span.line > expr.span().line || matches!(expr, Expr::Tuple(_, _) | Expr::Literal(_, _)) {
                    break;
                }
                self.advance();
                // Function call
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end_span = self.expect(&TokenKind::RParen)?.span;
                let span = expr.span().merge(end_span);

                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else if self.match_token(&TokenKind::LBracket) {
                // Slices: [..end], [start..end], [start..], or normal index: [idx]
                if self.match_token(&TokenKind::DotDotDot) || self.match_token(&TokenKind::DotDot) {
                    let inclusive = self.previous().kind == TokenKind::DotDotDot;
                    let end = if self.check(&TokenKind::RBracket) {
                        None
                    } else {
                        Some(Box::new(self.parse_expr()?))
                    };
                    let end_span = self.expect(&TokenKind::RBracket)?.span;
                    let span = expr.span().merge(end_span);
                    expr = Expr::Slice {
                        object: Box::new(expr),
                        start: None,
                        end,
                        inclusive,
                        span,
                    };
                } else {
                    let first = self.parse_expr()?;
                    if self.match_token(&TokenKind::DotDotDot) || self.match_token(&TokenKind::DotDot) {
                        let inclusive = self.previous().kind == TokenKind::DotDotDot;
                        let end = if self.check(&TokenKind::RBracket) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        let end_span = self.expect(&TokenKind::RBracket)?.span;
                        let span = expr.span().merge(end_span);
                        expr = Expr::Slice {
                            object: Box::new(expr),
                            start: Some(Box::new(first)),
                            end,
                            inclusive,
                            span,
                        };
                    } else {
                        let end_span = self.expect(&TokenKind::RBracket)?.span;
                        let span = expr.span().merge(end_span);
                        expr = Expr::Index {
                            object: Box::new(expr),
                            index: Box::new(first),
                            span,
                        };
                    }
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, AlphaError> {
        if self.check(&TokenKind::LBrace) {
            let block = self.parse_block()?;
            return Ok(Expr::Block(block));
        }

        let tok = self.advance();
        match tok.kind {
            TokenKind::IntLit(n) => Ok(Expr::Literal(LiteralValue::Int(n), tok.span)),
            TokenKind::FloatLit(n) => Ok(Expr::Literal(LiteralValue::Float(n), tok.span)),
            TokenKind::StringLit(s) => Ok(Expr::Literal(LiteralValue::String(s), tok.span)),
            TokenKind::InterpolatedString(s) => Ok(Expr::InterpolatedString(s, tok.span)),
            TokenKind::MultilineString(s) => Ok(Expr::MultilineString(s, tok.span)),
            TokenKind::InterpolatedMultilineString(s) => Ok(Expr::InterpolatedMultilineString(s, tok.span)),
            TokenKind::CharLit(c) => Ok(Expr::Literal(LiteralValue::Char(c), tok.span)),
            TokenKind::HexColorLit(h) => Ok(Expr::HexColor(h, tok.span)),
            TokenKind::True => Ok(Expr::Literal(LiteralValue::Bool(true), tok.span)),
            TokenKind::False => Ok(Expr::Literal(LiteralValue::Bool(false), tok.span)),
            TokenKind::Nothing => Ok(Expr::Literal(LiteralValue::Nothing, tok.span)),
            TokenKind::This => Ok(Expr::This(tok.span)),

            TokenKind::Make => {
                let start_span = tok.span;
                let structure_name = if self.match_token(&TokenKind::This) {
                    None
                } else {
                    Some(self.expect_ident()?.text)
                };
                self.expect(&TokenKind::LBrace)?;
                let mut fields = Vec::new();
                if !self.check(&TokenKind::RBrace) {
                    loop {
                        let f_name = self.expect_ident()?.text;
                        self.expect(&TokenKind::Colon)?;
                        let f_val = self.parse_expr()?;
                        fields.push((f_name, f_val));
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end_span = self.expect(&TokenKind::RBrace)?.span;
                Ok(Expr::Make {
                    structure_name,
                    fields,
                    span: start_span.merge(end_span),
                })
            }

            // Special constructor vect.new()
            TokenKind::TypeVect => {
                if self.match_token(&TokenKind::Dot) {
                    let next = self.expect_ident()?;
                    if next.text == "new" {
                        self.expect(&TokenKind::LParen)?;
                        let end_span = self.expect(&TokenKind::RParen)?.span;
                        return Ok(Expr::VectNew(tok.span.merge(end_span)));
                    }
                }
                Ok(Expr::Variable(tok.text, tok.span))
            }

            // Special constructor map.new()
            TokenKind::TypeMap => {
                if self.match_token(&TokenKind::Dot) {
                    let next = self.expect_ident()?;
                    if next.text == "new" {
                        self.expect(&TokenKind::LParen)?;
                        let end_span = self.expect(&TokenKind::RParen)?.span;
                        return Ok(Expr::MapNew(tok.span.merge(end_span)));
                    }
                }
                Ok(Expr::Variable(tok.text, tok.span))
            }

            TokenKind::Ident(name) => Ok(Expr::Variable(name, tok.span)),
            TokenKind::Schedule | TokenKind::Verify | TokenKind::System | TokenKind::TypeSomething | TokenKind::TypeNothing => Ok(Expr::Variable(tok.text, tok.span)),

            TokenKind::LParen => {
                let start_span = tok.span;
                if self.match_token(&TokenKind::RParen) {
                    return Ok(Expr::Tuple(Vec::new(), start_span.merge(self.previous().span)));
                }
                let expr = self.parse_expr()?;
                if self.match_token(&TokenKind::Arrow) {
                    let target_ty = self.parse_type()?;
                    let end_span = self.expect(&TokenKind::RParen)?.span;
                    return Ok(Expr::Cast {
                        expr: Box::new(expr),
                        target_type: target_ty,
                        span: start_span.merge(end_span),
                    });
                }
                if self.match_token(&TokenKind::Comma) {
                    let mut elements = vec![expr];
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            elements.push(self.parse_expr()?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end_span = self.expect(&TokenKind::RParen)?.span;
                    return Ok(Expr::Tuple(elements, start_span.merge(end_span)));
                }
                self.expect(&TokenKind::RParen)?;
                Ok(expr)
            }

            _ => Err(AlphaError::parse(
                format!("Unexpected token in expression: '{}'", tok.text),
                tok.span,
            )),
        }
    }

    // -------------------------------------------------------------
    // Helper Methods
    // -------------------------------------------------------------
    fn skip_semicolons(&mut self) {
        while self.match_token(&TokenKind::Semicolon) {}
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            &self.peek().kind == kind
        }
    }

    fn check_range_ahead_before_brace_or_sep(&self) -> bool {
        let mut idx = self.current;
        let mut has_dotdot = false;
        while idx < self.tokens.len() {
            match &self.tokens[idx].kind {
                TokenKind::DotDot | TokenKind::DotDotDot => {
                    has_dotdot = true;
                }
                TokenKind::Comma | TokenKind::Semicolon => {
                    return false;
                }
                TokenKind::LBrace => {
                    return has_dotdot;
                }
                _ => {}
            }
            idx += 1;
        }
        false
    }

    fn check_next(&self, kind: &TokenKind) -> bool {
        if self.current + 1 >= self.tokens.len() {
            false
        } else {
            &self.tokens[self.current + 1].kind == kind
        }
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<Token, AlphaError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            let hint = match kind {
                TokenKind::Colon => "Types must be preceded by a colon, e.g. 'var x: int = 10'",
                TokenKind::Equal => "Variables must be initialized with '=', e.g. 'var x: int = 10'",
                TokenKind::LBrace => "Code blocks in Alpha must start with '{'",
                TokenKind::RBrace => "Did you forget a closing brace '}' for the code block?",
                TokenKind::RParen => "Did you forget a closing parenthesis ')'?",
                TokenKind::RBracket => "Did you forget a closing bracket ']' in function metadata [visibility, return_type]?",
                _ => "Check the syntax at this position",
            };
            Err(AlphaError::parse(
                format!("Expected '{}', found '{}'", kind, tok.kind),
                tok.span,
            )
            .with_hint(hint))
        }
    }

    fn expect_ident(&mut self) -> Result<Token, AlphaError> {
        let tok = self.advance();
        match &tok.kind {
            TokenKind::Ident(_)
            | TokenKind::Schedule
            | TokenKind::Verify
            | TokenKind::System
            | TokenKind::TypeSomething
            | TokenKind::TypeNothing => Ok(tok),
            _ => Err(AlphaError::parse(
                format!("Expected identifier, found '{}'", tok.text),
                tok.span,
            )
            .with_hint("Identifiers can contain letters, numbers, underscores, and '?' anywhere (e.g. 'total_count', 'is_valid?', 'is?valid', '?check?')")),
        }
    }

    fn expect_ident_or_keyword_name(&mut self) -> Result<Token, AlphaError> {
        let tok = self.advance();
        match &tok.kind {
            TokenKind::Ident(_)
            | TokenKind::Schedule
            | TokenKind::Verify
            | TokenKind::System
            | TokenKind::Enum
            | TokenKind::Module
            | TokenKind::Nothing
            | TokenKind::TypeInt
            | TokenKind::TypeFloat
            | TokenKind::TypeString
            | TokenKind::TypeChar
            | TokenKind::TypeBoolean
            | TokenKind::TypeHexColor
            | TokenKind::TypeSomething
            | TokenKind::TypeNothing
            | TokenKind::TypeVect
            | TokenKind::TypeMap => Ok(tok),
            _ => Err(AlphaError::parse(
                format!("Expected identifier or member name, found '{}'", tok.text),
                tok.span,
            )),
        }
    }

    fn expect_identifier_or_text(&mut self) -> Result<String, AlphaError> {
        let tok = self.advance();
        Ok(tok.text)
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }

    fn previous(&self) -> Token {
        self.tokens[self.current.saturating_sub(1)].clone()
    }
}
