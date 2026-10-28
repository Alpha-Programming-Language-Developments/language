use alpha_lang::lsp::LspServer;

fn main() {
    let mut server = LspServer::new();
    if let Err(e) = server.run() {
        eprintln!("[alpha-lsp] Server error: {}", e);
        std::process::exit(1);
    }
}
