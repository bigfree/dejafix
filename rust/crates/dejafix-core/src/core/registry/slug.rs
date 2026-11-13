const FALLBACK: &str = "project";

#[must_use]
pub fn slugify(value: &str) -> String {
    let lowered = crate::core::text::python_strip(value).to_lowercase();
    let mut slug = String::with_capacity(lowered.len());
    for character in lowered.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        FALLBACK.to_owned()
    } else {
        slug.to_owned()
    }
}

#[must_use]
pub fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
}

#[must_use]
pub fn unique_id(base: &str, is_taken: impl Fn(&str) -> bool) -> String {
    if !is_taken(base) {
        return base.to_owned();
    }
    (2_u64..=u64::MAX)
        .map(|number| format!("{base}-{number}"))
        .find(|candidate| !is_taken(candidate))
        .unwrap_or_else(|| base.to_owned())
}

#[must_use]
pub fn normalize_path(raw: &str) -> String {
    if cfg!(windows) {
        normalize_with(&raw.replace('/', "\\"), '\\')
    } else {
        normalize_with(raw, '/')
    }
}

fn normalize_with(raw: &str, separator: char) -> String {
    let double = format!("{separator}{separator}");
    let root = if raw.starts_with(&double) && !raw.starts_with(&format!("{double}{separator}")) {
        double
    } else if raw.starts_with(separator) {
        separator.to_string()
    } else {
        String::new()
    };
    let parts: Vec<&str> = raw
        .split(separator)
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let joined = format!("{root}{}", parts.join(&separator.to_string()));
    if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    }
}

#[must_use]
pub fn path_name(normalized: &str) -> &str {
    let separator = if cfg!(windows) { '\\' } else { '/' };
    normalized.rsplit(separator).next().unwrap_or_default()
}
