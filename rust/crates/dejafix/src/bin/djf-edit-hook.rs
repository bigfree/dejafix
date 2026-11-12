use std::process::ExitCode;

fn main() -> ExitCode {
    dejafix::stub("djf-edit-hook", &mut std::io::stderr())
}
