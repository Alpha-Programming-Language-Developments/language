#![allow(dead_code)]
// Token definitions for the Alpha programming language.
use crate::diagnostics::error::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Var,
    Const,
    Function,
    Public,
    Here,
    Return,
    If,
    ElseIf,
    Else,
    Not,
    And,
    Or,
    For,
    Each,
    In,
    Import,
    Web,
    Break,
    Continue,
    Try,
    Catch,
    Check,
    Structure,
    Attach,
    Static,
    Nonstatic,
    This,
    Make,
    Schedule,
    Verify,
    System,
    Enum,
    Module,
    AtStrict,
    HashTypechecked,

    // Literals
    IntLit(i64),
    FloatLit(f64),
    HexColorLit(String),
    StringLit(String),
    InterpolatedString(String),
    MultilineString(String),
    InterpolatedMultilineString(String),
    CharLit(char),
    True,
    False,
    Nothing,

    // Builtin Type Identifiers
    TypeInt,
    TypeUnt,
    TypeFloat,
    TypeString,
    TypeChar,
    TypeBoolean,
    TypeHexColor,
    TypeSomething,
    TypeEverything,
    TypeNothing,
    TypeVect,
    TypeMap,

    // Identifier (can end with '?' such as is_valid?, nothing?)
    Ident(String),

    // Operators & Punctuation
    Plus,          // +
    Minus,         // -
    Star,          // *
    Slash,         // /
    Percent,       // %
    PlusPlus,      // ++
    MinusMinus,    // --
    PlusEqual,     // +=
    MinusEqual,    // -=
    StarEqual,     // *=
    SlashEqual,    // /=
    PercentEqual,  // %=
    Equal,         // =
    EqualEqual,    // ==
    FatArrow,      // =>
    BangEqual,     // !=
    Less,          // <
    LessEqual,     // <=
    Greater,       // >
    GreaterEqual,  // >=
    ColonColon,    // ::
    Dot,           // .
    DotDot,        // ..
    DotDotDot,     // ...
    Comma,         // ,
    Colon,         // :
    Semicolon,     // ;
    Question,      // ?
    Bang,          // !
    BangQuestion,  // !?
    AndAnd,        // &&
    OrOr,          // ||
    Arrow,         // ->
    LParen,        // (
    RParen,        // )
    LBrace,        // {
    RBrace,        // }
    LBracket,      // [
    RBracket,      // ]

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span, text: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            text: text.into(),
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Var => write!(f, "var"),
            TokenKind::Const => write!(f, "const"),
            TokenKind::Function => write!(f, "function"),
            TokenKind::Public => write!(f, "public"),
            TokenKind::Here => write!(f, "here"),
            TokenKind::Return => write!(f, "return"),
            TokenKind::If => write!(f, "if"),
            TokenKind::ElseIf => write!(f, "elseif"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::Not => write!(f, "not"),
            TokenKind::And => write!(f, "and"),
            TokenKind::Or => write!(f, "or"),
            TokenKind::For => write!(f, "for"),
            TokenKind::Each => write!(f, "each"),
            TokenKind::In => write!(f, "in"),
            TokenKind::Import => write!(f, "import"),
            TokenKind::Web => write!(f, "web"),
            TokenKind::Break => write!(f, "break"),
            TokenKind::Continue => write!(f, "continue"),
            TokenKind::Try => write!(f, "try"),
            TokenKind::Catch => write!(f, "catch"),
            TokenKind::Check => write!(f, "check"),
            TokenKind::Structure => write!(f, "structure"),
            TokenKind::Attach => write!(f, "attach"),
            TokenKind::Static => write!(f, "static"),
            TokenKind::Nonstatic => write!(f, "nonstatic"),
            TokenKind::This => write!(f, "this"),
            TokenKind::Make => write!(f, "make"),
            TokenKind::Schedule => write!(f, "schedule"),
            TokenKind::Verify => write!(f, "verify"),
            TokenKind::System => write!(f, "system"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Module => write!(f, "module"),
            TokenKind::AtStrict => write!(f, "@strict"),
            TokenKind::HashTypechecked => write!(f, "#typechecked"),
            TokenKind::IntLit(n) => write!(f, "{}", n),
            TokenKind::FloatLit(n) => write!(f, "{}", n),
            TokenKind::HexColorLit(h) => write!(f, "#({})", h),
            TokenKind::StringLit(s) => write!(f, "{:?}", s),
            TokenKind::InterpolatedString(s) => write!(f, "$\"{}\"", s),
            TokenKind::MultilineString(s) => write!(f, "\"\"\"{}\"\"\"", s),
            TokenKind::InterpolatedMultilineString(s) => write!(f, "$\"\"\"{}\"\"\"", s),
            TokenKind::CharLit(c) => write!(f, "'{}'", c),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Nothing => write!(f, "nothing"),
            TokenKind::TypeInt => write!(f, "int"),
            TokenKind::TypeUnt => write!(f, "unt"),
            TokenKind::TypeFloat => write!(f, "float"),
            TokenKind::TypeString => write!(f, "string"),
            TokenKind::TypeChar => write!(f, "char"),
            TokenKind::TypeBoolean => write!(f, "boolean"),
            TokenKind::TypeHexColor => write!(f, "hexcolor"),
            TokenKind::TypeSomething => write!(f, "something"),
            TokenKind::TypeEverything => write!(f, "everything"),
            TokenKind::TypeNothing => write!(f, "nothing"),
            TokenKind::TypeVect => write!(f, "vect"),
            TokenKind::TypeMap => write!(f, "map"),
            TokenKind::Ident(id) => write!(f, "{}", id),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::PlusPlus => write!(f, "++"),
            TokenKind::MinusMinus => write!(f, "--"),
            TokenKind::PlusEqual => write!(f, "+="),
            TokenKind::MinusEqual => write!(f, "-="),
            TokenKind::StarEqual => write!(f, "*="),
            TokenKind::SlashEqual => write!(f, "/="),
            TokenKind::PercentEqual => write!(f, "%="),
            TokenKind::Equal => write!(f, "="),
            TokenKind::EqualEqual => write!(f, "=="),
            TokenKind::FatArrow => write!(f, "=>"),
            TokenKind::BangEqual => write!(f, "!="),
            TokenKind::Less => write!(f, "<"),
            TokenKind::LessEqual => write!(f, "<="),
            TokenKind::Greater => write!(f, ">"),
            TokenKind::GreaterEqual => write!(f, ">="),
            TokenKind::ColonColon => write!(f, "::"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::DotDot => write!(f, ".."),
            TokenKind::DotDotDot => write!(f, "..."),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::Semicolon => write!(f, ";"),
            TokenKind::Question => write!(f, "?"),
            TokenKind::Bang => write!(f, "!"),
            TokenKind::BangQuestion => write!(f, "!?"),
            TokenKind::AndAnd => write!(f, "&&"),
            TokenKind::OrOr => write!(f, "||"),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::Eof => write!(f, "<EOF>"),
        }
    }
}
