mod instructions;
mod lax;
mod schema;
mod server;
mod types;

use std::process::ExitCode;

use dejafix_core::{McpSettings, McpState};
use rmcp::ServiceExt;

use crate::config::McpConfig;
pub use schema::instructions;
pub use types::DejafixServer;

const FAILED: u8 = 3;

fn settings(config: McpConfig) -> Option<McpSettings> {
    Some(McpSettings {
        pinned: config.root,
        cwd: config.cwd,
        home: config.home,
        dejafix_home: config.dejafix_home?,
        redact: config.redact,
        zone: config.zone,
    })
}

async fn serve(server: DejafixServer) -> Result<(), String> {
    let running = server
        .serve(rmcp::transport::io::stdio_pipes())
        .await
        .map_err(|error| error.to_string())?;
    running.waiting().await.map_err(|error| error.to_string())?;
    Ok(())
}

#[must_use]
pub fn main() -> ExitCode {
    let report = |message: &str| {
        eprintln!("djf-mcp: {message}");
        ExitCode::from(FAILED)
    };
    let config = match McpConfig::from_env(std::env::args_os()) {
        Ok(config) => config,
        Err(error) => return report(&format!("cannot read the environment: {error}")),
    };
    let Some(settings) = settings(config) else {
        return report("cannot determine the home directory");
    };
    let server = match DejafixServer::new(McpState::new(settings)) {
        Ok(server) => server,
        Err(error) => return report(&format!("bad tool table: {error}")),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return report(&format!("cannot start the runtime: {error}")),
    };
    match runtime.block_on(serve(server)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => report(&error),
    }
}

#[cfg(test)]
mod tests;
