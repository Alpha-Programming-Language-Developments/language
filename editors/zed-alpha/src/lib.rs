// Alpha language extension for Zed editor
use zed_extension_api::{self as zed, Result};

struct AlphaExtension;

impl zed::Extension for AlphaExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("alpha-lsp")
            .or_else(|| worktree.which("alpha-lsp.exe"))
            .unwrap_or_else(|| "C:/Users/Max/.cargo/bin/alpha-lsp.exe".to_string());

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AlphaExtension);
