use std::error::Error;
use std::process::Command;

#[test]
fn the_mcp_server_and_edit_hook_are_still_stubs() -> Result<(), Box<dyn Error>> {
    for (binary, program) in [
        (env!("CARGO_BIN_EXE_djf-mcp"), "djf-mcp"),
        (env!("CARGO_BIN_EXE_djf-edit-hook"), "djf-edit-hook"),
    ] {
        let output = Command::new(binary).output()?;
        assert_eq!(output.status.code(), Some(1), "{program}");
        assert!(output.stdout.is_empty(), "{program}");
        assert_eq!(
            String::from_utf8(output.stderr)?,
            format!("{program}: not implemented yet; use the Python dejafix 0.4.x\n")
        );
    }
    Ok(())
}
