#![allow(dead_code)]
// Abstract Syntax Tree (AST) definitions for Alpha.
use crate::diagnostics::error::Span;
use crate::semantics::types::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Here,
    Public,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportTarget {
    Local(String), // e.g. "tools/wifi.alpha", "file_name.alpha"
    Core(String),  // e.g. "Core/Basic.alpha"
    Web(String),   // e.g. web::"url"
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub target: ImportTarget,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AttachedMethod {
    pub name: String,
    pub is_static: bool,
    pub visibility: Visibility,
    pub return_type: Type,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructureDecl {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub methods: Vec<AttachedMethod>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub functions: Vec<FunctionDecl>,
    pub structures: Vec<StructureDecl>,
    pub enums: Vec<EnumDecl>,
    pub variables: Vec<VarDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub visibility: Visibility,
    pub return_type: Type,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub is_const: bool,
    pub ty: Type,
    pub init: Expr,
    pub has_explicit_type: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Nothing,
}

#[derive(Debug, Clone)]
pub enum CheckPattern {
    Something,
    Nothing,
    Else,
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum CheckBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct CheckArm {
    pub pattern: CheckPattern,
    pub binding: Option<String>,
    pub body: CheckBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(LiteralValue, Span),
    InterpolatedString(String, Span),
    MultilineString(String, Span),
    InterpolatedMultilineString(String, Span),
    Variable(String, Span),
    This(Span),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    Cast {
        expr: Box<Expr>,
        target_type: Type,
        span: Span,
    },
    ForceUnwrap {
        expr: Box<Expr>,
        span: Span,
    },
    Check {
        target: Box<Expr>,
        arms: Vec<CheckArm>,
        span: Span,
    },
    Make {
        structure_name: Option<String>,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    VectNew(Span),
    MapNew(Span),
    Tuple(Vec<Expr>, Span),
    HexColor(String, Span),
    Block(Block),
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s) => *s,
            Expr::InterpolatedString(_, s) => *s,
            Expr::MultilineString(_, s) => *s,
            Expr::InterpolatedMultilineString(_, s) => *s,
            Expr::Variable(_, s) => *s,
            Expr::This(s) => *s,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Ternary { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::ForceUnwrap { span, .. } => *span,
            Expr::Check { span, .. } => *span,
            Expr::Make { span, .. } => *span,
            Expr::VectNew(s) => *s,
            Expr::MapNew(s) => *s,
            Expr::Tuple(_, s) => *s,
            Expr::HexColor(_, s) => *s,
            Expr::Block(b) => b.span,
            Expr::Slice { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    Var(VarDecl),
    VarTuple {
        names: Vec<String>,
        types: Vec<Option<Type>>,
        overall_type: Option<Type>,
        init: Expr,
        is_const: bool,
        span: Span,
    },
    Structure(StructureDecl),
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
        span: Span,
    },
    IncDec {
        target: Expr,
        is_inc: bool,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Block,
        elseif_branches: Vec<(Expr, Block)>,
        else_branch: Option<Block>,
        span: Span,
    },
    Check {
        target: Expr,
        arms: Vec<CheckArm>,
        span: Span,
    },
    ForInfinite {
        body: Block,
        span: Span,
    },
    ForCount {
        init: Option<Box<Statement>>,
        condition: Option<Expr>,
        step: Option<Box<Statement>>,
        body: Block,
        span: Span,
    },
    ForRange {
        var_name: Option<String>,
        start: Expr,
        end: Expr,
        inclusive: bool,
        body: Block,
        span: Span,
    },
    ForEach {
        item_var: String,
        key_var: Option<String>,
        collection: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    TryCatch {
        try_block: Block,
        err_var: String,
        catch_block: Block,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    System {
        target: String,
        body: Block,
        span: Span,
    },
    Block(Block),
    Defer {
        statement: Box<Statement>,
        span: Span,
    },
    Verify {
        condition: Expr,
        message: Option<Expr>,
        span: Span,
    },
    Enum(EnumDecl),
    Module(ModuleDecl),
    StrictDirective(Span),
    Expr(Expr),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::Var(v) => v.span,
            Statement::VarTuple { span, .. } => *span,
            Statement::Structure(st) => st.span,
            Statement::Assign { span, .. } => *span,
            Statement::IncDec { span, .. } => *span,
            Statement::If { span, .. } => *span,
            Statement::Check { span, .. } => *span,
            Statement::ForInfinite { span, .. } => *span,
            Statement::ForCount { span, .. } => *span,
            Statement::ForRange { span, .. } => *span,
            Statement::ForEach { span, .. } => *span,
            Statement::Return { span, .. } => *span,
            Statement::TryCatch { span, .. } => *span,
            Statement::Break(s) => *s,
            Statement::Continue(s) => *s,
            Statement::System { span, .. } => *span,
            Statement::Block(b) => b.span,
            Statement::Defer { span, .. } => *span,
            Statement::Verify { span, .. } => *span,
            Statement::Enum(e) => e.span,
            Statement::Module(m) => m.span,
            Statement::StrictDirective(s) => *s,
            Statement::Expr(e) => e.span(),
        }
    }
}

/// Checks if the target platform matches the current operating system.
///
/// Supported targets:
/// - `win`, `windows` -> Windows
/// - `posix`, `unix` -> Any POSIX / Unix-like OS (Linux, macOS, BSD, etc.)
/// - `linux` -> Linux
/// - `macos`, `darwin`, `osx` -> macOS
/// - `bsd` -> FreeBSD, NetBSD, OpenBSD
/// - `android` -> Android
/// - `ios` -> iOS
/// - `wasm`, `web` -> WebAssembly
///
/// Can be overridden via the `ALPHA_TARGET_OS` environment variable for testing and cross-compilation.
pub fn matches_current_system(target: &str) -> bool {
    let t = target.trim().to_lowercase();
    if let Ok(override_os) = std::env::var("ALPHA_TARGET_OS") {
        let o = override_os.trim().to_lowercase();
        return match t.as_str() {
            "win" | "windows" => o == "win" || o == "windows",
            "posix" | "unix" => {
                o == "posix" || o == "unix" || o == "linux" || o == "macos" || o == "darwin"
            }
            "linux" => o == "linux",
            "macos" | "darwin" | "osx" => o == "macos" || o == "darwin" || o == "osx",
            "bsd" => o == "bsd" || o == "freebsd" || o == "netbsd" || o == "openbsd",
            "android" => o == "android",
            "ios" => o == "ios",
            "wasm" | "web" => o == "wasm" || o == "web",
            other => o == other,
        };
    }

    match t.as_str() {
        "win" | "windows" => cfg!(windows),
        "posix" | "unix" => cfg!(unix),
        "linux" => cfg!(target_os = "linux"),
        "macos" | "darwin" | "osx" => cfg!(target_os = "macos"),
        "bsd" => cfg!(any(
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )),
        "android" => cfg!(target_os = "android"),
        "ios" => cfg!(target_os = "ios"),
        "wasm" | "web" => cfg!(target_arch = "wasm32"),
        other => std::env::consts::OS.eq_ignore_ascii_case(other),
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub statements: Vec<Statement>,
    pub functions: Vec<FunctionDecl>,
    pub structures: Vec<StructureDecl>,
    pub enums: Vec<EnumDecl>,
    pub modules: Vec<ModuleDecl>,
}
