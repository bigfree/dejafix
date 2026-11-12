use std::io::Write;
use std::process::ExitCode;

pub fn stub(program: &str, stderr: &mut impl Write) -> ExitCode {
    writeln!(
        stderr,
        "{program}: not implemented yet; use the Python dejafix 0.4.x"
    )
    .map_or(ExitCode::from(3), |()| ExitCode::FAILURE)
}

#[cfg(test)]
mod tests;
