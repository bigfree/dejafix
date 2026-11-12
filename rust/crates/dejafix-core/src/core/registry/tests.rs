use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{
    Loaded, ProjectRecord, RegisterResult, Registry, RegistryError, is_slug, list_payload, load,
    meta_payload, normalize_path, slugify, unique_id,
};

const NOW: &str = "2027-01-15T08:00:00Z";
const LATER: &str = "2027-01-15T09:00:00Z";

fn absolute(path: &str) -> bool {
    Path::new(path).is_absolute()
}

fn no_resolve(_: &str) -> Option<PathBuf> {
    None
}

fn loaded(payload: &Value, meta: Option<&Value>) -> Loaded {
    load(Some(payload), meta, NOW, absolute)
}

fn ids(registry: &Registry) -> Vec<&str> {
    registry
        .projects
        .iter()
        .map(|record| record.id.as_str())
        .collect()
}

#[test]
fn slugs_follow_python() {
    assert_eq!(slugify("MatilyRec"), "matilyrec");
    assert_eq!(slugify("  My Cool_App!! "), "my-cool-app");
    assert_eq!(slugify("---"), "project");
    assert_eq!(slugify("Žluťoučký kůň"), "lu-ou-k-k");
    assert_eq!(slugify(""), "project");
    assert!(is_slug("web-2"));
    for bad in ["", "Web", "-web", "web-", "we--b", "we b"] {
        assert!(!is_slug(bad), "{bad}");
    }
    let taken = ["web", "web-2"];
    assert_eq!(unique_id("web", |id| taken.contains(&id)), "web-3");
    assert_eq!(unique_id("api", |id| taken.contains(&id)), "api");
}

#[cfg(unix)]
#[test]
fn paths_are_normalised_like_pure_posix_path() {
    assert_eq!(normalize_path("/a//b/./c/"), "/a/b/c");
    assert_eq!(normalize_path("//share/x"), "//share/x");
    assert_eq!(normalize_path("///x"), "/x");
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("/a/../b"), "/a/../b");
}

#[test]
fn a_0_2_x_list_gets_ids_and_is_marked_dirty() {
    let Loaded::Dirty(registry) = loaded(&json!(["/code/alpha", "/code/beta"]), None) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&registry), ["alpha", "beta"]);
    assert_eq!(registry.projects[0].created_at, NOW);
    assert_eq!(registry.projects[0].default_brain, "coding");
    assert_eq!(
        list_payload(&registry),
        json!(["/code/alpha", "/code/beta"])
    );
    let meta = meta_payload(&registry);
    let keys: Vec<&String> = meta["by_path"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["/code/alpha", "/code/beta"]);
    let Loaded::Clean(again) = loaded(&list_payload(&registry), Some(&meta)) else {
        panic!("not clean");
    };
    assert_eq!(again, registry);
}

#[test]
fn the_sidecar_is_written_in_python_key_order() {
    let Loaded::Dirty(mut registry) = loaded(&json!(["/code/alpha"]), None) else {
        panic!("not dirty");
    };
    registry.active_project = Some("alpha".to_owned());
    let text = serde_json::to_string(&meta_payload(&registry)).unwrap();
    assert_eq!(
        text,
        r#"{"schema_version":1,"active_project":"alpha","by_path":{"/code/alpha":{"id":"alpha","alias":null,"default_brain":"coding","tags":[],"created_at":"2027-01-15T08:00:00Z","updated_at":"2027-01-15T08:00:00Z"}}}"#
    );
}

#[test]
fn ids_do_not_collide_for_same_named_folders() {
    let Loaded::Dirty(registry) = loaded(&json!(["/a/web", "/b/web", "/c/web"]), None) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&registry), ["web", "web-2", "web-3"]);
}

#[test]
fn a_duplicate_sidecar_id_is_replaced() {
    let meta = json!({"by_path": {
        "/a/one": {"id": "same"},
        "/b/two": {"id": "same", "alias": "t", "tags": ["x", 1], "created_at": "c", "updated_at": "u"}
    }, "active_project": "same"});
    let Loaded::Dirty(registry) = loaded(&json!(["/a/one", "/b/two"]), Some(&meta)) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&registry), ["same", "two"]);
    assert_eq!(registry.projects[1].alias.as_deref(), Some("t"));
    assert_eq!(registry.projects[1].tags, ["x"]);
    assert_eq!(registry.projects[1].created_at, "c");
    assert_eq!(registry.active_project.as_deref(), Some("same"));
}

#[test]
fn junk_entries_are_dropped() {
    let payload = json!(["schema_version", "relative/path", 5, null, "/real/app"]);
    let Loaded::Dirty(registry) = loaded(&payload, Some(&json!("not a dict"))) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&registry), ["app"]);
    let Loaded::Clean(empty) = loaded(&json!(42), None) else {
        panic!("not clean");
    };
    assert_eq!(empty.projects.len(), 0);
    let Loaded::Clean(none) = load(None, None, NOW, absolute) else {
        panic!("not clean");
    };
    assert_eq!(none.projects.len(), 0);
}

#[cfg(unix)]
#[test]
fn a_string_payload_is_iterated_like_python() {
    let Loaded::Dirty(registry) = loaded(&json!("a/b"), None) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&registry), ["project"]);
    assert_eq!(registry.projects[0].path, PathBuf::from("/"));
}

#[test]
fn the_dev_format_is_converted() {
    let payload = json!({
        "schema_version": 1,
        "active_project": "alpha",
        "projects": [
            {"id": "alpha", "path": "/code/alpha", "alias": "a"},
            {"id": "no-path"},
            "junk"
        ]
    });
    let Loaded::Legacy(registry) = loaded(&payload, None) else {
        panic!("not legacy");
    };
    assert_eq!(ids(&registry), ["alpha"]);
    assert_eq!(registry.active_project.as_deref(), Some("alpha"));
    assert_eq!(registry.projects[0].alias.as_deref(), Some("a"));
    assert_eq!(registry.projects[0].created_at, "");
    assert_eq!(list_payload(&registry), json!(["/code/alpha"]));
}

fn registry_of(paths: &[&str]) -> Registry {
    let payload = Value::Array(paths.iter().map(|path| json!(path)).collect());
    loaded(&payload, None).into_registry()
}

#[test]
fn registering_is_idempotent_by_path() {
    let mut registry = registry_of(&["/code/alpha"]);
    let result = |outcome: Result<(RegisterResult, &ProjectRecord), RegistryError>| {
        outcome.map(|(result, record)| (result, record.id.clone()))
    };
    assert_eq!(
        result(registry.register(PathBuf::from("/code/alpha"), None, LATER)),
        Ok((RegisterResult::Existing, "alpha".to_owned()))
    );
    assert_eq!(
        result(registry.register(PathBuf::from("/other/alpha"), None, LATER)),
        Ok((RegisterResult::Added, "alpha-2".to_owned()))
    );
    assert_eq!(ids(&registry), ["alpha", "alpha-2"]);
    assert_eq!(registry.projects[1].created_at, LATER);
    assert_eq!(
        result(registry.register(PathBuf::from("/code/alpha"), Some("a"), LATER)),
        Ok((RegisterResult::Realiased, "alpha".to_owned()))
    );
    assert_eq!(registry.projects[0].updated_at, LATER);
    assert_eq!(
        result(registry.register(PathBuf::from("/code/alpha"), Some("a"), NOW)),
        Ok((RegisterResult::Existing, "alpha".to_owned()))
    );
    assert_eq!(
        result(registry.register(PathBuf::from("/x/new"), Some(""), NOW)),
        Err(RegistryError::BadAlias(String::new()))
    );
    assert_eq!(
        result(registry.register(PathBuf::from("/x/new"), Some("alpha-2"), NOW)),
        Err(RegistryError::AliasTaken {
            alias: "alpha-2".to_owned(),
            path: PathBuf::from("/other/alpha")
        })
    );
}

#[test]
fn lookup_is_case_insensitive_and_accepts_a_path() {
    let mut registry = registry_of(&["/code/MatilyRec", "/code/other"]);
    registry
        .set_alias("matilyrec", "mrec", NOW, no_resolve)
        .unwrap();
    for name in [
        "matilyrec",
        "MatilyRec",
        "MATILYREC",
        "mrec",
        "MRec",
        " mrec ",
    ] {
        assert_eq!(registry.find(name, no_resolve), Some(0), "{name}");
    }
    assert_eq!(
        registry.find("", |_| Some(PathBuf::from("/code/other"))),
        None
    );
    assert_eq!(
        registry.find("./x", |_| Some(PathBuf::from("/code/other"))),
        Some(1)
    );
    assert_eq!(registry.find("nope", no_resolve), None);
}

#[test]
fn aliases_and_tags_are_validated_and_stamped() {
    let mut registry = registry_of(&["/code/alpha", "/code/beta"]);
    assert_eq!(
        registry.set_alias("nope", "x", LATER, no_resolve),
        Err(RegistryError::NotFound("nope".to_owned()))
    );
    assert_eq!(
        registry.set_alias("alpha", "Bad", LATER, no_resolve),
        Err(RegistryError::BadAlias("Bad".to_owned()))
    );
    assert_eq!(
        registry
            .set_alias("alpha", "beta", LATER, no_resolve)
            .unwrap_err()
            .to_string(),
        "Alias 'beta' is already used by /code/beta."
    );
    assert_eq!(
        registry
            .set_alias("alpha", "alpha", LATER, no_resolve)
            .map(|record| record.alias.clone()),
        Ok(Some("alpha".to_owned()))
    );
    assert_eq!(
        registry
            .add_tag("alpha", "Web App", LATER, no_resolve)
            .map(|(record, changed)| (record.tags.clone(), changed)),
        Ok((vec!["web-app".to_owned()], true))
    );
    assert_eq!(
        registry
            .add_tag("alpha", "web-app", LATER, no_resolve)
            .map(|(record, changed)| (record.id.clone(), changed)),
        Ok(("alpha".to_owned(), false))
    );
    assert_eq!(registry.projects[0].tags, ["web-app"]);
    assert_eq!(
        registry
            .remove_tag("alpha", "WEB app", NOW, no_resolve)
            .map(|record| record.tags.len()),
        Ok(0)
    );
    assert_eq!(registry.projects[0].tags.len(), 0);
    assert_eq!(registry.projects[0].updated_at, NOW);
}

#[test]
fn the_active_project_is_set_cleared_and_dropped_on_removal() {
    let mut registry = registry_of(&["/code/alpha", "/code/beta"]);
    assert_eq!(
        registry
            .set_active(Some("beta"), no_resolve)
            .map(|record| record.map(|record| record.id.clone())),
        Ok(Some("beta".to_owned()))
    );
    assert_eq!(registry.active_project.as_deref(), Some("beta"));
    assert_eq!(
        registry
            .set_active(Some("x"), no_resolve)
            .unwrap_err()
            .to_string(),
        "No project 'x'. Registered: alpha, beta"
    );
    assert_eq!(
        registry.unregister("alpha", no_resolve).unwrap().id,
        "alpha"
    );
    assert_eq!(registry.active_project.as_deref(), Some("beta"));
    assert_eq!(registry.unregister("BETA", no_resolve).unwrap().id, "beta");
    assert_eq!(registry.active_project, None);
    assert_eq!(registry.known_names(), "none registered");
    assert_eq!(registry.set_active(None, no_resolve), Ok(None));
    assert_eq!(
        registry.unregister("beta", no_resolve),
        Err(RegistryError::NotFound("beta".to_owned()))
    );
}

#[test]
fn an_older_dejafix_can_only_append_a_path() {
    let mut registry = registry_of(&["/code/alpha"]);
    registry.set_alias("alpha", "a", NOW, no_resolve).unwrap();
    let meta = meta_payload(&registry);
    let Loaded::Dirty(appended) = loaded(&json!(["/code/alpha", "/code/beta"]), Some(&meta)) else {
        panic!("not dirty");
    };
    assert_eq!(ids(&appended), ["alpha", "beta"]);
    assert_eq!(appended.find("a", no_resolve), Some(0));
    assert_eq!(appended.projects[0].created_at, NOW);
}

#[test]
fn remove_id_removes_only_that_record_and_clears_it_as_active() {
    let mut registry = registry_of(&["/code/api", "/code/web"]);
    registry
        .set_alias("web", "api-old", LATER, no_resolve)
        .unwrap();
    registry.active_project = Some("web".to_owned());
    assert_eq!(
        registry.remove_id("api").map(|record| record.id),
        Some("api".to_owned())
    );
    assert_eq!(ids(&registry), ["web"]);
    assert_eq!(registry.active_project.as_deref(), Some("web"));
    assert_eq!(
        registry.remove_id("web").map(|record| record.id),
        Some("web".to_owned())
    );
    assert_eq!(registry.active_project, None);
    assert_eq!(registry.remove_id("web"), None);
}
