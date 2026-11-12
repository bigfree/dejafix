use std::fs;

use super::detect_stack;

#[test]
fn nothing_is_detected_without_manifests() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        detect_stack(dir.path()),
        crate::core::global::Stack::default()
    );
}

#[test]
fn package_json_yields_libraries_tags_and_frameworks() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("package.json"),
        "\u{feff}{\"dependencies\": {\"react\": \"1\", \"Next\": \"1\"}, \"devDependencies\": {\"eslint-plugin-react\": \"1\", \"vite\": \"1\"}}",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
    let stack = detect_stack(dir.path());
    assert_eq!(
        stack.libraries,
        ["eslint-plugin-react", "next", "react", "vite"]
    );
    assert_eq!(
        stack.tags,
        ["github-actions", "javascript", "nextjs", "react", "vite"]
    );
    assert_eq!(stack.frameworks, ["nextjs", "react"]);
    assert_eq!(stack.manifest_files, ["package.json", ".github/workflows"]);
}

#[test]
fn a_corrupt_manifest_still_counts_as_present() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), "{not json").unwrap();
    let stack = detect_stack(dir.path());
    assert_eq!(stack.tags, ["javascript"]);
    assert_eq!(stack.libraries, Vec::<String>::new());
}

#[test]
fn python_rust_and_go_manifests_are_parsed() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"x\"\n\n[project.optional-dependencies]\n\"FastAPI[all]>=0.1\",\nuvicorn\n[tool.x]\nflask = 1\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\n[dependencies]\nAxum = \"0.7\"\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("go.mod"),
        "module x\ngo 1.22\nrequire (\n\tgithub.com/gin-gonic/gin v1\n)\n",
    )
    .unwrap();
    let stack = detect_stack(dir.path());
    assert_eq!(
        stack.libraries,
        ["axum", "fastapi", "flask", "gin", "uvicorn"]
    );
    assert!(stack.frameworks.contains(&"axum".to_owned()));
    assert!(stack.tags.contains(&"python".to_owned()) && stack.tags.contains(&"go".to_owned()));
}
