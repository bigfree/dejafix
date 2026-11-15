use std::path::PathBuf;
use std::process::ExitCode;

use dejafix_core::testing::{FIXTURE_SEED, FIXTURE_SIZES, gotcha_lines, write_project};

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
    let global = out.join("home/global");
    let lines: String = gotcha_lines(500, FIXTURE_SEED)
        .into_iter()
        .map(|line| line + "\n")
        .collect();
    if let Err(error) = std::fs::create_dir_all(&global)
        .and_then(|()| std::fs::write(global.join("library_gotchas.jsonl"), lines))
    {
        eprintln!("gen-fixtures: {error}");
        return ExitCode::FAILURE;
    }
    println!("{}", global.display());
    ExitCode::SUCCESS
}
