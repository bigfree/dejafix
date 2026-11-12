use std::process::ExitCode;

fn main() -> ExitCode {
    dejafix::stub("djf", &mut std::io::stderr())
}
