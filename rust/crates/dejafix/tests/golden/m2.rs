use std::error::Error;
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use dejafix_core::testing::{
    casefold, casefold_table, is_python_word, redact, write_python_json_indented,
};
use serde_json::Value;

use crate::harness::{isolated, python, repository};

type TestResult = Result<(), Box<dyn Error>>;

pub fn oracle(args: &[&str], cwd: &Path, stdin: &str) -> Result<String, Box<dyn Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/oracle_m2.py");
    let repository = repository()?;
    let home = tempfile::tempdir()?;
    let mut command = isolated(python(&repository), home.path(), &repository.join("src"));
    let mut child = command
        .env("PYTHONHASHSEED", "0")
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(stdin.as_bytes())?;
    let finished = child.wait_with_output()?;
    if !finished.status.success() {
        return Err(String::from_utf8_lossy(&finished.stderr)
            .into_owned()
            .into());
    }
    Ok(String::from_utf8(finished.stdout)?)
}

#[test]
fn casefold_table_matches_python_for_every_code_point() -> TestResult {
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["casefold"], dir.path(), "")?;
    let mut rust = String::new();
    for (from, to) in casefold_table() {
        writeln!(
            rust,
            "{:x}\t{}",
            u32::from(*from),
            serde_json::to_string(to)?
        )?;
    }
    let mut python = String::new();
    for line in expected.lines() {
        let (code, json) = line.split_once('\t').unwrap_or((line, ""));
        let text: String = serde_json::from_str(json)?;
        writeln!(python, "{code}\t{}", serde_json::to_string(&text)?)?;
    }
    assert_eq!(python, rust);
    assert_eq!(casefold("ẞ"), "ss");
    Ok(())
}

#[test]
fn python_word_characters_match_isalnum_up_to_the_unicode_version() -> TestResult {
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["word"], dir.path(), "")?;
    let mut python = vec![false; 0x11_0000];
    for line in expected.lines() {
        let (first, last) = line.split_once('-').ok_or("bad range")?;
        for code in u32::from_str_radix(first, 16)?..=u32::from_str_radix(last, 16)? {
            if let Some(slot) = python.get_mut(usize::try_from(code)?) {
                *slot = true;
            }
        }
    }
    let unassigned = regex::Regex::new(r"\A\p{Cn}\z")?;
    let mut buffer = [0; 4];
    let differences: Vec<String> = (0..0x11_0000_u32)
        .filter_map(char::from_u32)
        .filter(|c| {
            python
                .get(usize::try_from(u32::from(*c)).unwrap_or(0))
                .copied()
                != Some(is_python_word(*c))
        })
        .filter(|c| !unassigned.is_match(c.encode_utf8(&mut buffer)))
        .map(|c| format!("{:x}", u32::from(c)))
        .take(20)
        .collect();
    assert!(differences.is_empty(), "{differences:?}");
    Ok(())
}

const JSON_DOCUMENTS: &[&str] = &[
    r#"["/Users/a/code/x", "/tmp/\u00e9 space"]"#,
    "[]",
    "{}",
    r#"{"schema_version": 1, "active_project": null, "by_path": {"/a": {"id": "a", "alias": null, "default_brain": "coding", "tags": [], "created_at": "2026-11-12T10:00:00Z", "updated_at": "2026-11-12T10:00:00Z"}}}"#,
    r#"{"last_version": "0.4.0", "update_check": true, "update_checked_at": 1794500000.123456, "z": [1, 2.5, -0.0, 1e300, 12345678901234567, "\ud83d\ude80"]}"#,
    r#"{"libraries": ["vite", "react", "\u017elu\u0165"]}"#,
    r#"{"/x/.claude.json": {"client": "Claude Code", "pinned": false, "seen": "2026-11-12 10:31"}, "nested": [[], [{}], {"a": [{"b": null}]}]}"#,
];

#[test]
fn indented_json_matches_python_json_dumps() -> TestResult {
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["dumps"], dir.path(), &JSON_DOCUMENTS.join("\n"))?;
    let mut rust = String::new();
    for document in JSON_DOCUMENTS {
        let value: Value = serde_json::from_str(document)?;
        for (indent, sort_keys) in [(1, false), (2, true)] {
            let mut out = Vec::new();
            write_python_json_indented(&mut out, &value, indent, sort_keys)?;
            rust.push_str(&String::from_utf8(out)?);
            rust.push_str("\n\0\n");
        }
    }
    assert_eq!(expected, rust);
    Ok(())
}

fn secrets() -> Vec<String> {
    let a = |n: usize| "a".repeat(n);
    vec![
        format!("sk-{}", a(48)),
        format!("sk-{}", a(39)),
        format!("sk-{}-_{}", a(20), a(20)),
        format!("ghp_{}", a(36)),
        format!("ghs_{}", a(37)),
        format!("github_pat_{}_{}", a(22), "b".repeat(59)),
        format!("AKIA{}", "A".repeat(16)),
        format!("AKIA{}", "A".repeat(17)),
        format!("AIza{}", a(35)),
        format!("xoxb-{}-{}", "1".repeat(12), a(24)),
        format!("xoxp-{}", a(9)),
        format!("sk_live_{}", a(24)),
        format!("rk_test_{}", a(40)),
        format!("eyJ{}.eyJ{}.{}", a(30), "b".repeat(30), "c".repeat(30)),
        format!("Bearer {}", a(30)),
        format!("bearer\t\u{2028}{}", a(20)),
        format!("Bearer {}", a(19)),
        "-----BEGIN RSA PRIVATE KEY-----".to_owned(),
        "-----BEGIN PRIVATE KEY-----".to_owned(),
        "tried `contain: layout` in styles.css".to_owned(),
    ]
}

const NEIGHBOURS: &[&str] = &[
    "", " ", "x", "X", "9", "_", "-", ".", "(", "'", "=", "é", "ж", "一", "\u{663}", "😀",
    "\u{a0}", "\u{2028}", "\u{1c}", "\n", "Bearer",
];

fn redaction_corpus() -> Vec<String> {
    let mut corpus = Vec::new();
    for secret in secrets() {
        for left in NEIGHBOURS {
            for right in NEIGHBOURS {
                corpus.push(format!("{left}{secret}{right}"));
            }
        }
    }
    let fragments = [
        "sk-",
        "Bearer ",
        "bearer",
        "aaaaaaaaaa",
        "AAAAAAAA",
        "ghp_",
        "_",
        "-",
        " ",
        ".",
        "eyJ",
        "AKIA",
        "xoxb-",
        "1111",
        "é",
        "x",
        "sk_live_",
        "-----BEGIN ",
        "PRIVATE KEY-----",
        "\n",
    ];
    let mut state: u64 = 0x5eed;
    for _ in 0..2_000 {
        let mut text = String::new();
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let length = (state >> 59) + 1;
        for _ in 0..length * 3 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let index = usize::try_from((state >> 33) % 20).unwrap_or(0);
            text.push_str(fragments.get(index).copied().unwrap_or(""));
        }
        corpus.push(text);
    }
    corpus
}

#[test]
fn redaction_matches_python_on_the_corpus() -> TestResult {
    let corpus = redaction_corpus();
    let input: Vec<String> = corpus
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<_, _>>()?;
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["redact"], dir.path(), &input.join("\n"))?;
    let mut mismatches = Vec::new();
    for (text, python) in corpus.iter().zip(expected.lines()) {
        let out = redact(text);
        let kinds: Vec<&str> = out.fired.iter().map(|kind| kind.as_str()).collect();
        let rust = serde_json::to_string(&serde_json::json!([out.text, kinds]))?;
        let python: Value = serde_json::from_str(python)?;
        if serde_json::to_string(&python)? != rust {
            mismatches.push(format!("{text:?}: {python} != {rust}"));
        }
    }
    assert_eq!(expected.lines().count(), corpus.len());
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    Ok(())
}

fn without_section(summary: &str, heading: &str) -> String {
    let Some(start) = summary.find(heading) else {
        return summary.to_owned();
    };
    let end = summary
        .get(start + heading.len()..)
        .and_then(|rest| rest.find("\n## "))
        .map_or(summary.len(), |at| start + heading.len() + at);
    format!("{}{}", &summary[..start], &summary[end..])
}

fn without_date(summary: &str) -> String {
    summary
        .lines()
        .filter(|line| !line.starts_with("_Last updated: "))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn summary_and_issue_files_match_python_on_every_fixture() -> TestResult {
    use dejafix_core::testing::{FIXTURE_SEED, write_project};
    use dejafix_core::{
        EventLog, FsResolver, SummaryInput, issue_files, key_files, path_context, render_summary,
    };
    for size in [100, 3_000] {
        let dir = tempfile::tempdir()?;
        write_project(dir.path(), size, FIXTURE_SEED)?;
        let python: Value = serde_json::from_str(&oracle(
            &["summary", &dir.path().display().to_string()],
            dir.path(),
            "",
        )?)?;
        let bytes = std::fs::read(dir.path().join(".dejafix/events.jsonl"))?;
        let clock = dejafix_core::Clock {
            now: jiff::Timestamp::now(),
            zone: jiff::tz::TimeZone::UTC,
        };
        let context = path_context(dir.path())?;
        let log = EventLog::parse(&bytes, &clock, &context, &FsResolver);
        let summary = render_summary(
            &log,
            &SummaryInput {
                project_name: &dir.path().file_name().ok_or("name")?.to_string_lossy(),
                today: jiff::civil::date(2026, 1, 1),
                purpose: None,
            },
        );
        let expected = python["summary"].as_str().ok_or("summary")?;
        assert_eq!(
            without_date(&without_section(expected, "## Key files")),
            without_date(&without_section(&summary, "## Key files")),
            "fixture {size}"
        );
        let counts = python["counts"].as_object().ok_or("counts")?;
        let rust_counts = key_files(&log);
        assert_eq!(counts.len(), rust_counts.len(), "fixture {size}");
        for (path, count) in &rust_counts {
            assert_eq!(
                counts.get(path).and_then(Value::as_u64),
                u64::try_from(*count).ok(),
                "{path}"
            );
        }
        assert!(rust_counts.windows(2).all(|pair| pair[0].1 >= pair[1].1));
        let issues = python["issues"].as_object().ok_or("issues")?;
        let rust_issues = issue_files(&log);
        assert_eq!(issues.len(), rust_issues.len(), "fixture {size}");
        for file in rust_issues {
            let name = file.name.ok_or("unnamed issue file")?;
            assert_eq!(
                issues.get(&name).and_then(Value::as_str),
                Some(file.contents.as_str()),
                "{name}"
            );
        }
    }
    Ok(())
}

const PURPOSE_CORPUS: &[&str] = &[
    "# Map\n\n## Project purpose\nA forecasting tool.\n\n## Structure\n",
    "## Project purpose\r\nCRLF purpose.\r\n\r\n## Structure\r\n",
    "## Project purpose\rCR purpose.\r## Next",
    "## Project purpose  \t\n \n Spaced body \n## Next\n",
    "##Project purpose\nNo space.\n",
    "## Project purpose x\nSuffix.\n## Project purpose\nSecond.\n",
    "## Project purpose\n\n## Structure\n- a\n## Next\n",
    "## Project purpose\nNot described yet.\n",
    "## Project purpose\nShort description of what the project does.\n",
    "## Project purpose\nStatus: not created yet\n",
    "## Project purpose\nThis file should be created by the first AI assistant\n",
    "## Project purpose\nReplace this placeholder with words.\n",
    "## Project purpose\n   \n",
    "## Project purpose\nRuns to the end of the file",
    "intro\n## What this project is\nOld heading.\n## Decisions\n- x\n",
    "## What this project is\n## Project purpose\nBoth.\n",
    "## Project purpose\u{2028}\nUnicode space.\n",
    "## Project purpose\u{1c}\nSeparator.\n",
    "## Project purpose\nbody\n##not heading\n## real\n",
    "",
];

#[test]
fn purpose_extraction_matches_python() -> TestResult {
    use dejafix_core::{extract_purpose_from_map, extract_purpose_from_summary};
    let input: Vec<String> = PURPOSE_CORPUS
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<_, _>>()?;
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["purpose"], dir.path(), &input.join("\n"))?;
    let lines: Vec<&str> = expected.lines().collect();
    assert_eq!(lines.len(), PURPOSE_CORPUS.len());
    for (text, python) in PURPOSE_CORPUS.iter().zip(lines) {
        let read = dejafix_core::testing::universal_newlines(text);
        let rust = serde_json::json!([
            extract_purpose_from_map(&read),
            extract_purpose_from_summary(text)
        ]);
        let python: Value = serde_json::from_str(python)?;
        assert_eq!(python, rust, "{text:?}");
    }
    Ok(())
}

const MANIFESTS: &[&[(&str, &str)]] = &[
    &[],
    &[(
        "pyproject.toml",
        "[project]\nname = \"x\"\ndependencies = [\"typer>=0.9\"]\n\n[project.optional-dependencies]\n\"FastAPI[all]>=0.1\",\nuvicorn ; python_version>'3'\n\"pytest\" ,\n[tool.poetry.dev-dependencies]\nblack = \"*\"\npython = \"^3.11\"\n[x]\nnumpy\n",
    )],
    &[(
        "requirements.txt",
        "\u{feff}Django>=4\n# comment\n-r other.txt\nrequests[socks]==2 ; os_name\n\nhttpx @ git+https://x\r\nCelery~=5\n",
    )],
    &[(
        "package.json",
        "\u{feff}{\"dependencies\": {\"react\": \"18\", \"Next\": \"1\", \"@vue/core\": \"3\"}, \"devDependencies\": {\"tailwindcss\": \"3\", \"typescript\": \"5\", \"eslint-plugin-react\": \"1\"}}",
    )],
    &[(
        "package.json",
        "{\"dependencies\": [\"express\", \"vite\"]}",
    )],
    &[(
        "package.json",
        "{\"dependencies\": \"vue\", \"devDependencies\": null}",
    )],
    &[("package.json", "[1, 2]")],
    &[("package.json", "{broken")],
    &[(
        "Cargo.toml",
        "[package]\nname = \"x\"\n[dependencies]\nAxum = \"0.7\"\nserde = { version = \"1\" }\n# not = dep\n[dev-dependencies]\nactix-web = \"4\"\n[workspace.dependencies]\ntokio = \"1\"\n",
    )],
    &[(
        "go.mod",
        "module example.com/x\n\ngo 1.22\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.0\n\tgithub.com/spf13/Cobra v1\n)\n// indirect\nrequire golang.org/x/net v0.1\n",
    )],
    &[
        ("Dockerfile", "FROM x"),
        ("docker-compose.yml", ""),
        ("Gemfile", ""),
        ("pom.xml", ""),
        ("build.gradle.kts", ""),
        ("bun.lockb", ""),
        ("tsconfig.json", "{}"),
        ("Pipfile", ""),
        ("setup.py", ""),
    ],
    &[(
        "pyproject.toml",
        "\u{ff}\u{fe} not utf8 is written as bytes below",
    )],
];

#[test]
fn stack_detection_matches_python() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut roots = Vec::new();
    for (index, files) in MANIFESTS.iter().enumerate() {
        let root = dir.path().join(format!("case{index}"));
        std::fs::create_dir_all(&root)?;
        for (name, text) in *files {
            std::fs::write(root.join(name), text)?;
        }
        roots.push(root);
    }
    std::fs::write(
        roots.last().ok_or("no root")?.join("pyproject.toml"),
        b"\xff\xfe[x]\n",
    )?;
    std::fs::create_dir_all(dir.path().join("case0/.github/workflows"))?;
    let arguments: Vec<String> = roots
        .iter()
        .map(|root| root.display().to_string())
        .collect();
    let mut args = vec!["stack"];
    args.extend(arguments.iter().map(String::as_str));
    let expected = oracle(&args, dir.path(), "")?;
    for (root, python) in roots.iter().zip(expected.lines()) {
        let stack = dejafix_core::detect_stack(root);
        let rust = serde_json::json!({
            "frameworks": stack.frameworks,
            "libraries": stack.libraries,
            "manifest_files": stack.manifest_files,
            "tags": stack.tags,
        });
        let python: Value = serde_json::from_str(python)?;
        assert_eq!(python, rust, "{}", root.display());
    }
    Ok(())
}

#[test]
fn promotion_matches_python() -> TestResult {
    use dejafix_core::{EventKind, Outcome, promotable_set, promotion};
    let cases: Vec<(String, &str, Option<&str>, Vec<&str>)> = vec![
        (
            "react strict mode double-invokes".into(),
            "attempt",
            Some("failed"),
            vec!["react"],
        ),
        (
            "react strict mode".into(),
            "attempt",
            Some("worked"),
            vec!["react"],
        ),
        ("react strict mode".into(), "attempt", None, vec![]),
        (
            "the next HMR reload".into(),
            "attempt",
            Some("partial"),
            vec!["vite"],
        ),
        (
            "the next HMR reload".into(),
            "attempt",
            Some("partial"),
            vec![],
        ),
        (
            "gotcha: tailwind purge".into(),
            "note",
            None,
            vec!["tailwindcss"],
        ),
        (
            "Lesson: Redis and celery and NGINX".into(),
            "decision",
            None,
            vec![],
        ),
        (
            "  never use pandas inplace".into(),
            "decision",
            None,
            vec!["pandas"],
        ),
        (
            "don't trust docker cache".into(),
            "note",
            None,
            vec!["docker"],
        ),
        ("Use FastAPI".into(), "decision", None, vec!["fastapi"]),
        (
            "gotcha: vitest and jest both".into(),
            "note",
            None,
            vec!["jest", "vitest"],
        ),
        ("bug: prisma_client.x".into(), "note", None, vec![]),
        ("bug: préact react".into(), "note", None, vec![]),
        ("gotcha: torch.compile".into(), "issue", None, vec![]),
        ("warning: next.js".into(), "note", None, vec!["nextjs"]),
    ];
    let input: Vec<String> = cases
        .iter()
        .map(|(summary, kind, outcome, libraries)| {
            serde_json::to_string(&serde_json::json!([summary, kind, outcome, libraries]))
        })
        .collect::<Result<_, _>>()?;
    let dir = tempfile::tempdir()?;
    let expected = oracle(&["promote"], dir.path(), &input.join("\n"))?;
    let promotable = promotable_set(&[]);
    for ((summary, kind, outcome, libraries), python) in cases.iter().zip(expected.lines()) {
        let kind = match *kind {
            "attempt" => EventKind::Attempt,
            "decision" => EventKind::Decision,
            "note" => EventKind::Note,
            _ => EventKind::Issue,
        };
        let outcome = outcome.and_then(|outcome| match outcome {
            "failed" => Some(Outcome::Failed),
            "partial" => Some(Outcome::Partial),
            "worked" => Some(Outcome::Worked),
            _ => None,
        });
        let libraries: Vec<String> = libraries
            .iter()
            .map(|library| (*library).to_owned())
            .collect();
        let rust = promotion(summary, kind, outcome, &promotable, &libraries, &[]);
        let python: Option<String> = serde_json::from_str(python)?;
        assert_eq!(python, rust, "{summary}");
    }
    Ok(())
}
