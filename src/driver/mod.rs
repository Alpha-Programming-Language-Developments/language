// Driver module (CLI interface and project handling).
pub mod cli;
pub mod project;
pub mod pkg;
pub mod test;

pub use cli::run_cli;
pub use project::{find_project_root, init_project, load_project_config, ProjectConfig};
