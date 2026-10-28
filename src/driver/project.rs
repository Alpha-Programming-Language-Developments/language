#![allow(dead_code)]
// Project management for the Alpha language (alpha here, alpha build).
use crate::diagnostics::error::{AlphaError, Span};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectSection,
}

impl ProjectConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            project: ProjectSection {
                name: name.into(),
                version: "0.0.1".to_string(),
            },
        }
    }
}

/// Initialize a new Alpha project in the specified directory (`alpha here`).
pub fn init_project(target_dir: &Path, project_name: Option<String>) -> Result<(), AlphaError> {
    let toml_path = target_dir.join("alpha-project.toml");
    if toml_path.exists() {
        return Err(AlphaError::runtime(
            "Project already exists in this directory (alpha-project.toml found)",
            Span::default(),
        ));
    }

    let default_name = target_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("alpha_project")
        .to_string();

    let name = project_name.unwrap_or(default_name);
    let toml_content = format!(
        "[project]\nname = \"{}\"\nversion = \"0.0.1\"\n",
        name
    );

    fs::write(&toml_path, toml_content).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to write alpha-project.toml: {}", e),
            Span::default(),
        )
    })?;

    let src_dir = target_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to create src/ directory: {}", e),
            Span::default(),
        )
    })?;

    let main_alpha_path = src_dir.join("main.alpha");
    if !main_alpha_path.exists() {
        let template = "function main() [here, nothing] {\n\n}\n";
        fs::write(&main_alpha_path, template).map_err(|e| {
            AlphaError::runtime(
                format!("Failed to create src/main.alpha: {}", e),
                Span::default(),
            )
        })?;
    }

    println!("[Alpha] Initialized project '{}' successfully.", name);
    println!("  - Created: alpha-project.toml");
    println!("  - Created: src/main.alpha");
    Ok(())
}

/// Find and load `alpha-project.toml` by searching upwards from the given directory.
pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        if current.join("alpha-project.toml").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Parse `alpha-project.toml`.
pub fn load_project_config(project_dir: &Path) -> Result<ProjectConfig, AlphaError> {
    let toml_path = project_dir.join("alpha-project.toml");
    if !toml_path.exists() {
        return Err(AlphaError::runtime(
            "No 'alpha-project.toml' found in directory. Use 'alpha here' to initialize a project.",
            Span::default(),
        ));
    }

    let content = fs::read_to_string(&toml_path).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to read alpha-project.toml: {}", e),
            Span::default(),
        )
    })?;

    toml::from_str::<ProjectConfig>(&content).map_err(|e| {
        AlphaError::runtime(
            format!("Failed to parse alpha-project.toml: {}", e),
            Span::default(),
        )
    })
}
