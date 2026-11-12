use std::process::ExitCode;

fn main() -> ExitCode {
    dejafix::stub("djf-mcp", &mut std::io::stderr())
}
