use toml::de::{DeTable, DeValue};

use super::types::{ConfigError, LinkSpec, ProjectLinks};

pub const CONFIG_LIMIT: usize = 64 * 1024;
pub const RELATION_CHARS: usize = 200;

#[must_use]
pub fn sanitize(text: &str, max_chars: usize) -> String {
    let flat = text
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "'");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    let head: String = flat.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", head.trim_end())
}

fn text_of<'t>(table: &'t DeTable<'_>, key: &str) -> Option<&'t str> {
    table.get(key)?.get_ref().as_str()
}

fn list_of<'t, 'i>(
    table: &'t DeTable<'i>,
    key: &'static str,
) -> Result<&'t [toml::Spanned<DeValue<'i>>], ConfigError> {
    match table.get(key).map(toml::Spanned::get_ref) {
        None => Ok(&[]),
        Some(DeValue::Array(items)) => Ok(items),
        Some(_) => Err(ConfigError::WrongType(key)),
    }
}

fn link_of(item: &DeValue<'_>) -> Option<LinkSpec> {
    let table = item.as_table()?;
    let project = text_of(table, "project")?.trim();
    if project.is_empty() {
        return None;
    }
    let relation = text_of(table, "relation")
        .map(|relation| sanitize(relation, RELATION_CHARS))
        .unwrap_or_default();
    Some(LinkSpec {
        project: project.to_owned(),
        relation,
    })
}

pub fn parse_config(text: &str) -> Result<ProjectLinks, ConfigError> {
    if text.len() > CONFIG_LIMIT {
        return Err(ConfigError::TooLarge(CONFIG_LIMIT));
    }
    let parsed = DeTable::parse(text).map_err(|error| {
        let message = error.to_string();
        ConfigError::Invalid(sanitize(
            message.lines().next().unwrap_or_default(),
            RELATION_CHARS,
        ))
    })?;
    let table = parsed.get_ref();
    let boundary = list_of(table, "boundary")?
        .iter()
        .filter_map(|item| item.get_ref().as_str())
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(str::to_owned)
        .collect();
    let links = list_of(table, "links")?
        .iter()
        .filter_map(|item| link_of(item.get_ref()))
        .collect();
    Ok(ProjectLinks { boundary, links })
}
