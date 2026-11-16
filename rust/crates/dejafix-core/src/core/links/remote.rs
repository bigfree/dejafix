fn finish(host: &str, path: &str) -> Option<String> {
    let path = path.trim_matches('/');
    let path = path
        .strip_suffix(".git")
        .unwrap_or(path)
        .trim_end_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{}/{path}", host.to_ascii_lowercase()))
}

fn host_of(authority: &str) -> &str {
    let without_user = authority.rsplit('@').next().unwrap_or(authority);
    without_user.split(':').next().unwrap_or(without_user)
}

#[must_use]
pub fn normalize_remote(url: &str) -> Option<String> {
    let url = url.trim();
    if let Some((scheme, rest)) = url.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        return finish(host_of(authority), path);
    }
    let (before, after) = url.split_once(':')?;
    if before.contains(['/', '\\']) || before.chars().count() <= 1 {
        return None;
    }
    finish(host_of(before), after)
}

fn is_origin_header(header: &str) -> bool {
    let header = header.trim();
    if header.eq_ignore_ascii_case("remote.origin") {
        return true;
    }
    header
        .split_once(char::is_whitespace)
        .is_some_and(|(section, name)| {
            section.eq_ignore_ascii_case("remote") && name.trim() == "\"origin\""
        })
}

#[must_use]
pub fn origin_url(git_config: &str) -> Option<String> {
    let mut in_origin = false;
    for raw in git_config.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            in_origin = is_origin_header(header.trim_end_matches(']'));
            continue;
        }
        if in_origin
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("url")
        {
            let value = value.trim().trim_matches('"').trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

#[must_use]
pub fn project_identity(origin: Option<&str>, folder: &str) -> String {
    origin
        .and_then(normalize_remote)
        .unwrap_or_else(|| folder.to_owned())
}
