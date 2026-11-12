use std::error::Error;
use std::process::Command;

#[test]
fn the_mcp_server_is_still_a_stub() -> Result<(), Box<dyn Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_djf-mcp")).output()?;
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
    assert_eq!(
        String::from_utf8(output.stderr)?,
        "djf-mcp: not implemented yet; use the Python dejafix 0.4.x\n"
    );
    Ok(())
}
