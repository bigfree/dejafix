use std::process::ExitCode;

fn main() -> ExitCode {
    dejafix::stub("dejafix", &mut std::io::stderr())
}
