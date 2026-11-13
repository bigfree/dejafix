use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Probe, Resolution, ResolveError, Source, resolve};
use crate::core::registry::Registry;

#[derive(Default)]
struct Fake {
    memory: HashSet<PathBuf>,
    dirs: HashSet<PathBuf>,
    cwd_project: Option<PathBuf>,
}

impl Fake {
    fn with(projects: &[&str]) -> Self {
        let mut fake = Self::default();
        for name in projects {
            let path = PathBuf::from(format!("/work/{name}"));
            fake.dirs.insert(path.clone());
            fake.memory.insert(path);
        }
        fake
    }
}

impl Probe for Fake {
    fn has_memory(&self, path: &Path) -> bool {
        self.memory.contains(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }

    fn expand_user(&self, text: &str) -> PathBuf {
        PathBuf::from(text.replacen('~', "/home/me", 1))
    }

    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        Some(path.to_path_buf())
    }

    fn lookup(&self, text: &str) -> Option<PathBuf> {
        Path::new(text).is_absolute().then(|| PathBuf::from(text))
    }

    fn discover(&self) -> Option<PathBuf> {
        self.cwd_project.clone()
    }
}

fn registry(names: &[&str], active: Option<&str>) -> Registry {
    let mut registry = Registry::default();
    for name in names {
        let _ = registry.register(
            PathBuf::from(format!("/work/{name}")),
            None,
            "2026-01-01T00:00:00Z",
        );
    }
    registry.active_project = active.map(str::to_owned);
    registry
}

fn root(result: &Result<Resolution, ResolveError>) -> (&str, Option<Source>) {
    match result {
        Ok(resolution) => (
            resolution.root.to_str().unwrap_or_default(),
            Some(resolution.source),
        ),
        Err(_) => ("", None),
    }
}

fn message(result: Result<Resolution, ResolveError>) -> String {
    result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
}

#[test]
fn unknown_project_errors_with_the_known_ones() {
    let registry = registry(&["alpha"], None);
    let text = message(resolve(
        &registry,
        Some("nope"),
        None,
        &[],
        &Fake::with(&["alpha"]),
    ));
    assert_eq!(
        text,
        "No project 'nope' — not a registered id, alias, or path. Registered: alpha"
    );
    assert_eq!(
        message(resolve(
            &Registry::default(),
            Some("nope"),
            None,
            &[],
            &Fake::default()
        )),
        "No project 'nope' — not a registered id, alias, or path. Registered: none registered"
    );
}

#[test]
fn explicit_selection_wins() {
    let registry = registry(&["alpha", "beta"], Some("alpha"));
    let probe = Fake::with(&["alpha", "beta"]);
    assert_eq!(
        root(&resolve(&registry, Some("beta"), None, &[], &probe)),
        ("/work/beta", Some(Source::Explicit))
    );
}

#[test]
fn an_explicit_folder_outside_the_registry_is_used_by_its_name() {
    let mut probe = Fake::with(&["alpha"]);
    probe.dirs.insert(PathBuf::from("/home/me/gamma"));
    probe.memory.insert(PathBuf::from("/home/me/gamma"));
    probe.dirs.insert(PathBuf::from("/plain"));
    let found = resolve(&Registry::default(), Some("~/gamma"), None, &[], &probe);
    assert_eq!(root(&found), ("/home/me/gamma", Some(Source::Explicit)));
    assert_eq!(
        found.map(|resolution| resolution.name).ok().as_deref(),
        Some("gamma")
    );
    assert_eq!(
        message(resolve(
            &Registry::default(),
            Some("/plain"),
            None,
            &[],
            &probe
        )),
        "/plain has no .dejafix/ — run `djf init` there first."
    );
}

#[test]
fn active_project_is_used_when_no_selection() {
    let registry = registry(&["alpha", "beta"], Some("beta"));
    let probe = Fake::with(&["alpha", "beta"]);
    assert_eq!(
        root(&resolve(&registry, None, None, &[], &probe)),
        ("/work/beta", Some(Source::Active))
    );
}

#[test]
fn a_registered_project_without_memory_is_named() {
    let registry = registry(&["alpha", "beta"], Some("beta"));
    let probe = Fake::with(&["alpha"]);
    assert_eq!(
        message(resolve(&registry, None, None, &[], &probe)),
        "'beta' is registered at /work/beta, but that folder has no .dejafix/ any more. Run `djf init` there, or `djf project remove beta`."
    );
}

#[test]
fn client_roots_outrank_a_stale_active_project() {
    let registry = registry(&["alpha", "beta"], Some("alpha"));
    let probe = Fake::with(&["alpha", "beta"]);
    let found = resolve(
        &registry,
        None,
        None,
        &[PathBuf::from("/work/beta")],
        &probe,
    );
    assert_eq!(root(&found), ("/work/beta", Some(Source::Roots)));
}

#[test]
fn two_usable_roots_is_an_error_not_a_coin_flip() {
    let probe = Fake::with(&["alpha", "beta"]);
    let roots = [PathBuf::from("/work/alpha"), PathBuf::from("/work/beta")];
    assert_eq!(
        message(resolve(&Registry::default(), None, None, &roots, &probe)),
        "Your client offered several dejafix projects (alpha, beta). Pass project=\"<name>\" on the call so the write lands in the right one."
    );
}

#[test]
fn nothing_selected_refuses_rather_than_guessing() {
    let registry = registry(&["alpha", "beta"], None);
    let probe = Fake::with(&["alpha", "beta"]);
    assert_eq!(
        message(resolve(&registry, None, None, &[], &probe)),
        "No project selected. Pass project=\"<name>\", or run `djf project use <name>` to set an active one. Registered: alpha, beta"
    );
}

#[test]
fn the_working_folder_comes_before_the_only_project() {
    let registry = registry(&["alpha", "beta"], None);
    let mut probe = Fake::with(&["alpha", "beta"]);
    probe.cwd_project = Some(PathBuf::from("/work/beta"));
    let found = resolve(&registry, None, None, &[], &probe);
    assert_eq!(root(&found), ("/work/beta", Some(Source::Cwd)));
    assert_eq!(
        found.map(|resolution| resolution.name).ok().as_deref(),
        Some("beta")
    );
}

#[test]
fn pinned_root_is_a_boundary_not_a_default() {
    let registry = registry(&["alpha", "beta"], None);
    let probe = Fake::with(&["alpha", "beta"]);
    let alpha = Path::new("/work/alpha");
    assert_eq!(
        root(&resolve(&registry, None, Some(alpha), &[], &probe)),
        ("/work/alpha", Some(Source::Pinned))
    );
    assert_eq!(
        root(&resolve(&registry, Some("alpha"), Some(alpha), &[], &probe)),
        ("/work/alpha", Some(Source::Pinned))
    );
    assert_eq!(
        root(&resolve(
            &registry,
            Some("/work/alpha"),
            Some(alpha),
            &[],
            &probe
        )),
        ("/work/alpha", Some(Source::Pinned))
    );
    assert_eq!(
        message(resolve(&registry, Some("beta"), Some(alpha), &[], &probe)),
        "This server is pinned to 'alpha' (/work/alpha) and cannot write to 'beta'. Start dejafix without --root to serve several projects, or use that project's own server."
    );
    assert_eq!(
        message(resolve(
            &registry,
            None,
            Some(Path::new("/work/gone")),
            &[],
            &probe
        )),
        "--root /work/gone has no .dejafix/ — run `djf init` there first."
    );
}

#[test]
fn a_single_registered_project_needs_no_selection() {
    let probe = Fake::with(&["alpha", "beta"]);
    let one = registry(&["alpha"], None);
    assert_eq!(
        root(&resolve(&one, None, None, &[], &probe)),
        ("/work/alpha", Some(Source::Only))
    );
    let two = registry(&["alpha", "beta"], None);
    assert!(resolve(&two, None, None, &[], &probe).is_err());
}

#[test]
fn sources_are_named_as_python_names_them() {
    let names: Vec<&str> = [
        Source::Pinned,
        Source::Explicit,
        Source::Roots,
        Source::Active,
        Source::Cwd,
        Source::Only,
    ]
    .into_iter()
    .map(Source::as_str)
    .collect();
    assert_eq!(
        names,
        ["pinned", "explicit", "roots", "active", "cwd", "only"]
    );
}
