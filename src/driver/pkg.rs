// Alpha Language Package Manager (`alpha pkg`)
// Handles downloading, discovering, and registering remote Alpha packages in packages.alpaks.

use std::fs;
use std::path::Path;
use std::process::Command;
use crate::error::AlphaError;

/// Main entry point for `alpha pkg` commands.
pub fn handle_pkg_command(args: &[String], cur_dir: &Path) -> Result<(), AlphaError> {
    if args.is_empty() {
        print_pkg_help();
        return Ok(());
    }

    match args[0].as_str() {
        "add" => {
            if args.len() < 2 {
                eprintln!("[Alpha Error] Missing package URL for 'alpha pkg add <url>'\n");
                print_pkg_help();
                return Ok(());
            }
            let custom_alias = args.get(2).map(|s| s.as_str());
            add_package(&args[1], custom_alias, cur_dir)
        }
        "list" => list_packages(cur_dir),
        "remove" | "rm" => {
            if args.len() < 2 {
                eprintln!("[Alpha Error] Missing package name for 'alpha pkg remove <name>'\n");
                return Ok(());
            }
            remove_package(&args[1], cur_dir)
        }
        "--help" | "-h" | "help" => {
            print_pkg_help();
            Ok(())
        }
        url => {
            // Default: `alpha pkg <url> [alias]`
            let custom_alias = args.get(1).map(|s| s.as_str());
            add_package(url, custom_alias, cur_dir)
        }
    }
}

fn print_pkg_help() {
    println!(
        r#"Alpha Package Manager (`alpha pkg`)

USAGE:
    alpha pkg <URL> [alias]          Download, discover entry and add package
    alpha pkg add <URL> [alias]      Explicitly add remote package
    alpha pkg list                   List configured packages in packages.alpaks
    alpha pkg remove <name>          Remove a package from packages.alpaks

EXAMPLES:
    alpha pkg https://github.com/Alpha-Programming-Language-Developments/webalph
    alpha pkg add https://github.com/org/repo my_lib
"#
    );
}

/// Resolves a remote repository, discovers its alias and entrypoint, and adds it to packages.alpaks.
fn add_package(raw_url: &str, custom_alias: Option<&str>, cur_dir: &Path) -> Result<(), AlphaError> {
    println!("[Alpha PKG] Resolving package: '{}'...", raw_url);

    let (owner, repo, explicit_branch) = parse_github_coords(raw_url)?;

    let branches_to_try = if let Some(b) = explicit_branch {
        vec![b]
    } else {
        vec!["main".to_string(), "master".to_string()]
    };

    let mut resolved_branch = None;
    let mut resolved_alias = None;
    let mut resolved_entry = None;

    // 1. Probe remote repository branches and locate alpha-project.toml / entry files
    for branch in &branches_to_try {
        // Try to fetch alpha-project.toml or Alpha.toml to get package name
        let config_names = ["alpha-project.toml", "Alpha.toml", "alpha.toml"];
        for cfg_name in config_names {
            let cfg_url = format!("https://raw.githubusercontent.com/{}/{}/{}/{}", owner, repo, branch, cfg_name);
            if let Ok(content) = fetch_url_text(&cfg_url) {
                if let Some(name) = extract_toml_name(&content) {
                    resolved_alias = Some(name);
                    resolved_branch = Some(branch.clone());
                    break;
                }
            }
        }

        // Try entrypoint candidates in order: src/lib.alpha, src/main.alpha, lib.alpha, main.alpha
        let entry_candidates = ["src/lib.alpha", "src/main.alpha", "lib.alpha", "main.alpha"];
        for entry in entry_candidates {
            let entry_url = format!("https://raw.githubusercontent.com/{}/{}/{}/{}", owner, repo, branch, entry);
            if let Ok(content) = fetch_url_text(&entry_url) {
                if !content.trim().is_empty() {
                    resolved_entry = Some(entry.to_string());
                    resolved_branch = Some(branch.clone());
                    break;
                }
            }
        }

        if resolved_entry.is_some() {
            break;
        }
    }

    let branch = resolved_branch.unwrap_or_else(|| "main".to_string());
    let entry_file = resolved_entry.unwrap_or_else(|| "src/lib.alpha".to_string());
    let alias = custom_alias
        .map(|s| s.to_string())
        .or(resolved_alias)
        .unwrap_or_else(|| repo.clone());

    // Construct import url in standard GitHub blob format
    let import_url = format!(
        "https://github.com/{}/{}/blob/{}/{}",
        owner, repo, branch, entry_file
    );

    println!("[Alpha PKG] Package alias: '{}'", alias);
    println!("[Alpha PKG] Entry point:   '{}' (branch: {})", entry_file, branch);

    // Save into packages.alpaks in current project root
    let root = crate::driver::project::find_project_root(cur_dir).unwrap_or_else(|| cur_dir.to_path_buf());
    let alpaks_path = root.join("packages.alpaks");

    update_packages_alpaks(&alpaks_path, &alias, &import_url)?;

    println!(
        "[Alpha PKG] Successfully added to {:?}:\n    \"{}\" = import \"{}\"",
        alpaks_path.file_name().unwrap_or_default(),
        alias,
        import_url
    );

    Ok(())
}

fn list_packages(cur_dir: &Path) -> Result<(), AlphaError> {
    let root = crate::driver::project::find_project_root(cur_dir).unwrap_or_else(|| cur_dir.to_path_buf());
    let alpaks_path = root.join("packages.alpaks");

    if !alpaks_path.exists() {
        println!("[Alpha PKG] No 'packages.alpaks' file found in {:?}", root);
        return Ok(());
    }

    let content = fs::read_to_string(&alpaks_path).map_err(|e| {
        AlphaError::runtime(format!("Failed to read packages.alpaks: {}", e), Default::default())
    })?;

    println!("[Alpha PKG] Configured packages in {:?}:", alpaks_path);
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//") {
            println!("  {}", trimmed);
            count += 1;
        }
    }

    if count == 0 {
        println!("  (No packages defined yet)");
    }

    Ok(())
}

fn remove_package(name_to_remove: &str, cur_dir: &Path) -> Result<(), AlphaError> {
    let root = crate::driver::project::find_project_root(cur_dir).unwrap_or_else(|| cur_dir.to_path_buf());
    let alpaks_path = root.join("packages.alpaks");

    if !alpaks_path.exists() {
        println!("[Alpha PKG] No packages.alpaks found in {:?}", root);
        return Ok(());
    }

    let content = fs::read_to_string(&alpaks_path).map_err(|e| {
        AlphaError::runtime(format!("Failed to read packages.alpaks: {}", e), Default::default())
    })?;

    let mut new_lines = Vec::new();
    let mut removed = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let target_matches = trimmed.starts_with(&format!("\"{}\"", name_to_remove))
            || trimmed.starts_with(&format!("{} =", name_to_remove))
            || trimmed.starts_with(&format!("{}=", name_to_remove));

        if target_matches {
            removed = true;
        } else {
            new_lines.push(line);
        }
    }

    if removed {
        fs::write(&alpaks_path, new_lines.join("\n") + "\n").map_err(|e| {
            AlphaError::runtime(format!("Failed to update packages.alpaks: {}", e), Default::default())
        })?;
        println!("[Alpha PKG] Removed package '{}' from packages.alpaks", name_to_remove);
    } else {
        println!("[Alpha PKG] Package '{}' not found in packages.alpaks", name_to_remove);
    }

    Ok(())
}

fn update_packages_alpaks(path: &Path, alias: &str, import_url: &str) -> Result<(), AlphaError> {
    let entry_line = format!("\"{}\" = import \"{}\"", alias, import_url);

    let content = if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::from("# packages.alpaks\n# Remote dependencies for this Alpha project\n\n")
    };

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut replaced = false;

    for line in lines.iter_mut() {
        let trimmed = line.trim();
        let target_matches = trimmed.starts_with(&format!("\"{}\"", alias))
            || trimmed.starts_with(&format!("{} =", alias))
            || trimmed.starts_with(&format!("{}=", alias));

        if target_matches {
            *line = entry_line.clone();
            replaced = true;
            break;
        }
    }

    if !replaced {
        lines.push(entry_line);
    }

    let output = lines.join("\n") + "\n";
    fs::write(path, output).map_err(|e| {
        AlphaError::runtime(format!("Failed to write packages.alpaks: {}", e), Default::default())
    })?;

    Ok(())
}

fn parse_github_coords(raw: &str) -> Result<(String, String, Option<String>), AlphaError> {
    let mut clean = raw.trim().trim_end_matches('/').trim_end_matches(".git").to_string();
    if clean.starts_with("https://github.com/") {
        clean = clean["https://github.com/".len()..].to_string();
    } else if clean.starts_with("http://github.com/") {
        clean = clean["http://github.com/".len()..].to_string();
    } else if clean.starts_with("github.com/") {
        clean = clean["github.com/".len()..].to_string();
    }

    let parts: Vec<&str> = clean.split('/').collect();
    if parts.len() < 2 {
        return Err(AlphaError::runtime(
            format!("Invalid GitHub URL '{}'. Expected format: 'https://github.com/owner/repo'", raw),
            Default::default(),
        ));
    }

    let owner = parts[0].to_string();
    let repo = parts[1].to_string();

    let branch = if parts.len() >= 4 && (parts[2] == "tree" || parts[2] == "blob") {
        Some(parts[3].to_string())
    } else {
        None
    };

    Ok((owner, repo, branch))
}

fn fetch_url_text(url: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let res = Command::new("curl.exe")
        .args(["-s", "-L", "-f", "-A", "Mozilla/5.0", url])
        .output();

    #[cfg(not(target_os = "windows"))]
    let res = Command::new("curl")
        .args(["-s", "-L", "-f", "-A", "Mozilla/5.0", url])
        .output();

    match res {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout).to_string();
            if body.trim().is_empty() {
                Err("Empty response".to_string())
            } else {
                Ok(body)
            }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(format!("Curl failed: {}", err))
        }
        Err(e) => Err(format!("Could not run curl: {}", e)),
    }
}

fn extract_toml_name(toml_content: &str) -> Option<String> {
    for line in toml_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") {
            if let Some((_, right)) = trimmed.split_once('=') {
                let name = right.trim().trim_matches('"').trim_matches('\'').trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}
