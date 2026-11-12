use std::borrow::Cow;

use super::location::location_to_file;
use super::types::{Never, PathContext, PathResolver, RelativePath};

const MEMORY_DIR: &str = ".dejafix";

pub fn project_relative<'a>(
    path: &'a str,
    context: &PathContext,
    resolver: &impl PathResolver,
) -> RelativePath<'a> {
    if path.is_empty() {
        return RelativePath(Cow::Borrowed(path));
    }
    let candidate = normpath(slashed(path));
    if !is_absolute(&candidate) {
        return RelativePath(candidate);
    }
    match anchor(&candidate, context, Some(resolver)) {
        Some(relative) => RelativePath(Cow::Owned(relative)),
        None => RelativePath(candidate),
    }
}

pub fn is_memory_path(path: &str, context: &PathContext, resolver: &impl PathResolver) -> bool {
    let normalized = slashed(path);
    if has_drive(&normalized) && !has_drive(&context.root) {
        return normalized.split('/').any(|part| part == MEMORY_DIR);
    }
    if is_absolute(&normalized) {
        return anchor(&normpath(normalized), context, Some(resolver))
            .is_some_and(|relative| first_is_memory(&relative));
    }
    [context.root.as_str(), context.cwd.as_str()]
        .into_iter()
        .find_map(|base| {
            anchor(
                &normpath(Cow::Owned(join(base, &normalized))),
                context,
                None::<&Never>,
            )
        })
        .is_some_and(|relative| first_is_memory(&relative))
}

pub fn location_file<'a>(
    location: &'a str,
    context: &PathContext,
    resolver: &impl PathResolver,
) -> Option<RelativePath<'a>> {
    let file = location_to_file(location)?;
    if !file.contains(['/', '.']) || is_memory_path(&file, context, resolver) {
        return None;
    }
    Some(match file {
        Cow::Borrowed(file) => project_relative(file, context, resolver),
        Cow::Owned(file) => RelativePath(Cow::Owned(
            project_relative(&file, context, resolver).0.into_owned(),
        )),
    })
}

impl RelativePath<'_> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_owned(self) -> RelativePath<'static> {
        RelativePath(Cow::Owned(self.0.into_owned()))
    }
}

pub(crate) fn normpath(path: Cow<'_, str>) -> Cow<'_, str> {
    if is_normal(&path) {
        return path;
    }
    let slashes = if path.starts_with("//") && !path.starts_with("///") {
        2
    } else {
        usize::from(path.starts_with('/'))
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if slashes == 0 && parts.last().is_none_or(|last| *last == "..") => {
                parts.push(part);
            }
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let joined = format!("{}{}", "/".repeat(slashes), parts.join("/"));
    Cow::Owned(if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    })
}

fn is_normal(path: &str) -> bool {
    let body = path.strip_prefix('/').unwrap_or(path);
    !path.is_empty()
        && path != "/"
        && !body.starts_with('/')
        && body.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn slashed(path: &str) -> Cow<'_, str> {
    if path.contains('\\') {
        Cow::Owned(path.replace('\\', "/"))
    } else {
        Cow::Borrowed(path)
    }
}

fn has_drive(path: &str) -> bool {
    matches!(path.as_bytes(), [letter, b':', b'/', ..] if letter.is_ascii_alphabetic())
}

fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || has_drive(path)
}

fn join(base: &str, path: &str) -> String {
    if base.is_empty() || base.ends_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn first_is_memory(relative: &str) -> bool {
    relative.split('/').next() == Some(MEMORY_DIR)
}

impl PathResolver for Never {
    fn resolve(&self, _: &str) -> Option<String> {
        None
    }
}

fn anchor(
    candidate: &str,
    context: &PathContext,
    resolver: Option<&impl PathResolver>,
) -> Option<String> {
    relative_to(candidate, &context.root, context.fold_case).or_else(|| {
        let real = resolver?.resolve(candidate)?;
        relative_to(&real, &context.root, context.fold_case)
    })
}

fn parts(path: &str) -> (&str, impl Iterator<Item = &str>) {
    let (anchor, rest) = if let Some(rest) = path
        .strip_prefix("//")
        .filter(|rest| !rest.starts_with('/'))
    {
        ("//", rest)
    } else if let Some(rest) = path.strip_prefix('/') {
        ("/", rest)
    } else if has_drive(path) {
        (
            path.get(..2).unwrap_or_default(),
            path.get(3..).unwrap_or_default(),
        )
    } else {
        ("", path)
    };
    (
        anchor,
        rest.split('/')
            .filter(|part| !part.is_empty() && *part != "."),
    )
}

fn same(left: &str, right: &str, fold_case: bool) -> bool {
    if fold_case {
        left.chars()
            .flat_map(char::to_lowercase)
            .eq(right.chars().flat_map(char::to_lowercase))
    } else {
        left == right
    }
}

fn relative_to(path: &str, root: &str, fold_case: bool) -> Option<String> {
    let (path_anchor, mut path_parts) = parts(path);
    let (root_anchor, root_parts) = parts(root);
    if !same(path_anchor, root_anchor, fold_case) {
        return None;
    }
    for root_part in root_parts {
        if !path_parts
            .next()
            .is_some_and(|part| same(part, root_part, fold_case))
        {
            return None;
        }
    }
    let rest: Vec<&str> = path_parts.collect();
    Some(if rest.is_empty() {
        ".".to_owned()
    } else {
        rest.join("/")
    })
}

impl std::borrow::Borrow<str> for RelativePath<'_> {
    fn borrow(&self) -> &str {
        &self.0
    }
}
