use std::cell::Cell;

use proptest::prelude::*;

use crate::testing::NoResolve;

use super::{
    PathContext, PathResolver, is_memory_path, location_file, location_to_file, project_relative,
};

fn context() -> PathContext {
    PathContext {
        root: "/work/proj".into(),
        cwd: "/work/proj/sub".into(),
        fold_case: false,
    }
}

fn relative(path: &str) -> String {
    project_relative(path, &context(), &NoResolve)
        .as_str()
        .to_owned()
}

#[test]
fn location_to_file_cases() {
    let cases = [
        ("src/foo.py", Some("src/foo.py")),
        ("src/foo.py:42", Some("src/foo.py")),
        ("src/foo.py:42:7", Some("src/foo.py")),
        ("foo.py", Some("foo.py")),
        ("deep/nested/path/mod.ts:1", Some("deep/nested/path/mod.ts")),
        (r"C:\src\foo.py", Some(r"C:\src\foo.py")),
        (r"C:\src\foo.py:42", Some(r"C:\src\foo.py")),
        ("D:/repo/app.tsx:9", Some("D:/repo/app.tsx")),
        (r"src\windows\rel.py:3", Some(r"src\windows\rel.py")),
        ("AuthService.validate", None),
        ("SomeClass", None),
        ("", None),
        ("   ", None),
    ];
    for (location, expected) in cases {
        assert_eq!(
            location_to_file(location).as_deref(),
            expected,
            "{location:?}"
        );
    }
}

#[test]
fn location_to_file_follows_python_corners() {
    let cases = [
        ("  \"'src/a.py:3'\"  ", Some("src/a.py")),
        ("a.PY", Some("a.PY")),
        ("a.\u{212a}t", Some("a.\u{212a}t")),
        ("a.py:1:2:3", None),
        ("src/a.py:1\n'", Some("src/a.py\n")),
        ("a.py:1\n'", None),
        (":12", None),
        ("C:\\:12", Some("C:\\")),
        ("x.py:\u{0661}\u{0662}", Some("x.py")),
        ("C:", None),
        ("v1.2", None),
    ];
    for (location, expected) in cases {
        assert_eq!(
            location_to_file(location).as_deref(),
            expected,
            "{location:?}"
        );
    }
}

#[test]
fn a_bare_path_counts_the_same_as_one_with_a_line_number() {
    let bare = location_file("src/foo.py", &context(), &NoResolve).unwrap();
    let lined = location_file("src/foo.py:42", &context(), &NoResolve).unwrap();
    assert_eq!(bare, lined);
}

#[test]
fn a_windows_path_is_not_counted_as_a_file_called_c() {
    assert_eq!(
        location_to_file(r"C:\src\payments\stripe.py:12").as_deref(),
        Some(r"C:\src\payments\stripe.py")
    );
}

#[test]
fn a_method_location_is_not_a_file() {
    assert!(location_file("AuthService.validate", &context(), &NoResolve).is_none());
}

#[test]
fn the_same_file_by_two_spellings_counts_once() {
    let first = location_file("src/foo.py", &context(), &NoResolve).unwrap();
    let second = location_file("./src/foo.py:42", &context(), &NoResolve).unwrap();
    assert_eq!(first.as_str(), second.as_str());
}

#[test]
fn one_spelling_per_file() {
    for spelling in [
        "src/a.py",
        "./src/a.py",
        "src//a.py",
        "src/./a.py",
        "src\\a.py",
        "/work/proj/src/a.py",
        "src/x/../a.py",
        "src/a.py/",
    ] {
        assert_eq!(relative(spelling), "src/a.py", "{spelling}");
    }
    assert_eq!(relative("/elsewhere/a.py"), "/elsewhere/a.py");
    assert_eq!(relative("/work/proj"), ".");
    assert_eq!(relative("/work/project/a.py"), "/work/project/a.py");
    assert_eq!(relative(""), "");
    assert_eq!(relative("."), ".");
    assert_eq!(relative("../a.py"), "../a.py");
    assert_eq!(relative("a/../../b"), "../b");
    assert_eq!(relative("/../a"), "/a");
    assert_eq!(relative("//x//y"), "//x/y");
    assert_eq!(relative("///x"), "/x");
    assert_eq!(
        relative("C:\\work\\legacy\\Report.cs"),
        "C:/work/legacy/Report.cs"
    );
}

#[test]
fn an_unchanged_path_is_not_copied() {
    let path = "src/a.py";
    assert!(matches!(
        project_relative(path, &context(), &NoResolve).0,
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn the_resolved_spelling_is_tried_when_the_lexical_one_is_outside() {
    struct Private<'c>(&'c Cell<usize>);
    impl PathResolver for Private<'_> {
        fn resolve(&self, path: &str) -> Option<String> {
            self.0.set(self.0.get() + 1);
            Some(format!("/private{path}"))
        }
    }
    let calls = Cell::new(0);
    let context = PathContext {
        root: "/private/tmp/proj".into(),
        ..context()
    };
    assert_eq!(
        project_relative("/tmp/proj/a.py", &context, &Private(&calls)).as_str(),
        "a.py"
    );
    assert_eq!(
        project_relative("/private/tmp/proj/b.py", &context, &Private(&calls)).as_str(),
        "b.py"
    );
    assert_eq!(
        project_relative("rel.py", &context, &Private(&calls)).as_str(),
        "rel.py"
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn windows_roots_compare_without_case() {
    let context = PathContext {
        root: "C:/Users/Dev/proj".into(),
        cwd: "C:/Users/Dev/proj".into(),
        fold_case: true,
    };
    assert_eq!(
        project_relative("c:\\users\\dev\\PROJ\\src\\a.py", &context, &NoResolve).as_str(),
        "src/a.py"
    );
    assert!(is_memory_path(
        "C:\\Users\\Dev\\proj\\.dejafix\\x",
        &context,
        &NoResolve
    ));
}

#[test]
fn memory_paths_are_only_the_root_dejafix_directory() {
    let c = context();
    assert!(is_memory_path(".dejafix/summary.md", &c, &NoResolve));
    assert!(is_memory_path("./.dejafix/events.jsonl", &c, &NoResolve));
    assert!(is_memory_path("../.dejafix/summary.md", &c, &NoResolve));
    assert!(is_memory_path("src/../.dejafix/summary.md", &c, &NoResolve));
    assert!(is_memory_path("/work/proj/.dejafix/x", &c, &NoResolve));
    assert!(is_memory_path("D:\\repo\\.dejafix\\x", &c, &NoResolve));
    assert!(!is_memory_path("sub/.dejafix/x", &c, &NoResolve));
    assert!(!is_memory_path("tests/fixtures/.dejafix/x", &c, &NoResolve));
    assert!(!is_memory_path("/elsewhere/.dejafix/x", &c, &NoResolve));
    assert!(!is_memory_path(".dejafixx/a", &c, &NoResolve));
    assert!(!is_memory_path("", &c, &NoResolve));
}

#[test]
fn location_file_ignores_non_paths() {
    let c = context();
    assert_eq!(
        location_file("src/a.py:42", &c, &NoResolve)
            .unwrap()
            .as_str(),
        "src/a.py"
    );
    assert!(location_file("deploy pipeline", &c, &NoResolve).is_none());
    assert!(location_file("", &c, &NoResolve).is_none());
    assert!(location_file(".dejafix/summary.md:3", &c, &NoResolve).is_none());
}

proptest! {
    #[test]
    fn location_parsing_never_panics(location in "\\PC{0,60}") {
        let _ = location_to_file(&location);
        let _ = location_file(&location, &context(), &NoResolve);
    }

    #[test]
    fn path_handling_never_panics(path in "[a-zA-Z0-9_./\\\\:é ]{0,40}") {
        let _ = project_relative(&path, &context(), &NoResolve);
        let _ = is_memory_path(&path, &context(), &NoResolve);
    }

    #[test]
    fn a_line_suffix_does_not_change_the_file(
        path in "[a-z]{1,8}(/[a-z]{1,8}){0,3}\\.(py|ts|rs)",
        line in 1_u32..10_000,
        column in proptest::option::of(1_u32..200),
    ) {
        let located = match column {
            Some(column) => format!("{path}:{line}:{column}"),
            None => format!("{path}:{line}"),
        };
        prop_assert_eq!(location_to_file(&located), location_to_file(&path));
    }

    #[test]
    fn spellings_of_one_relative_path_agree(parts in proptest::collection::vec("[a-z]{1,6}", 1..5)) {
        let unix = parts.join("/");
        let expected = relative(&unix);
        for spelling in [format!("./{unix}"), parts.join("\\"), format!("/work/proj/{unix}"), format!("{unix}/")] {
            prop_assert_eq!(relative(&spelling), expected.clone());
        }
    }

    #[test]
    fn project_relative_is_idempotent(path in "[a-zA-Z0-9_./\\\\:]{0,30}") {
        let once = relative(&path);
        prop_assert_eq!(relative(&once), once);
    }
}
