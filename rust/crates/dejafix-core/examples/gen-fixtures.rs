use std::path::PathBuf;
use std::process::ExitCode;

use dejafix_core::testing::{FIXTURE_SEED, FIXTURE_SIZES, write_project};

fn main() -> ExitCode {
    let Some(out) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: gen-fixtures <output-dir>");
        return ExitCode::from(2);
    };
    for size in FIXTURE_SIZES {
        let root = out.join(format!("events-{size}"));
        if let Err(error) = write_project(&root, size, FIXTURE_SEED) {
            eprintln!("gen-fixtures: {error}");
            return ExitCode::FAILURE;
        }
        println!("{}", root.display());
    }
    ExitCode::SUCCESS
}
