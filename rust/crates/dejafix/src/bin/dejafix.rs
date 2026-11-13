use std::process::ExitCode;

fn main() -> ExitCode {
    dejafix::run(
        std::env::args_os(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}
