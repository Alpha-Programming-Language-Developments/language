// Backend module (native compilation and code generation).
pub mod compiler;
pub mod codegen_rust;
pub mod llvm;

pub use compiler::{build_file, build_project};
pub use codegen_rust::RustCodegen;
pub use llvm::LlvmCodegen;
