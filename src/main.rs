// Alpha Language - CLI Executable Entry Point.
pub const VERSION: &str = "0.1.0";

fn main() {
    if let Err(e) = alpha_lang::driver::cli::run_cli() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
