#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
fn main() {
    std::process::exit(cli_common::repeat::run("stuck"));
}
