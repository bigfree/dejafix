use std::io::{self, Write};
use std::process::ExitCode;

use super::stub;

struct BrokenPipe;

impl Write for BrokenPipe {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn stub_names_the_program_and_fails() {
    let mut stderr = Vec::new();
    let code = stub("djf", &mut stderr);
    assert_eq!(code, ExitCode::FAILURE);
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "djf: not implemented yet; use the Python dejafix 0.4.x\n"
    );
}

#[test]
fn stub_reports_an_unwritable_stderr_as_an_io_failure() {
    assert_eq!(stub("djf-mcp", &mut BrokenPipe), ExitCode::from(3));
}
