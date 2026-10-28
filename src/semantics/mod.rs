// Semantics module (types and static type checking).
pub mod typechecker;
pub mod types;

pub use typechecker::{FuncSignature, SymbolInfo, TypeChecker};
pub use types::Type;
