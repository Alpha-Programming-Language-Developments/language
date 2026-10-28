// Alpha Programming Language Library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod backend;
pub mod diagnostics;
pub mod driver;
pub mod frontend;
pub mod runtime;
pub mod semantics;

// Convenience re-exports matching top-level names for easy access
pub use backend::compiler;
pub use diagnostics::error;
pub use driver::{cli, project};
pub use frontend::{ast, lexer, parser, token};
pub use runtime::{env, interpreter, stdlib, value};
pub use semantics::{typechecker, types};
