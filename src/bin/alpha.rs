// Alpha Language - CLI Executable Entry Point ('alpha').
fn main() {
    if let Err(e) = alpha_lang::driver::cli::run_cli() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
