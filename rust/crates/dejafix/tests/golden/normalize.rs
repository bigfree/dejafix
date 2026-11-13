use std::collections::HashMap;
use std::path::Path;

use regex::{Captures, Regex};

use crate::types::Normalizer;

const ID: &str = r"(?P<id>evt_[0-9a-f]{20}|(?:pat|got)_[0-9a-f]{12})";
const DJF_BIN: &str = r#"(?P<djf>(?m:^DJF_BIN="[^"$`\n]*"$))"#;
const STAMP: &str =
    r"(?P<stamp>\d{4}-\d{2}-\d{2}(?:T\d{2}:\d{2}:\d{2}(?:\.\d{6})?Z| \d{2}:\d{2})?)";

impl Normalizer {
    pub fn new(roots: &[(&Path, &str)], since: &str) -> Result<Self, regex::Error> {
        let mut labels = HashMap::new();
        for (path, label) in roots {
            if let Ok(canonical) = std::fs::canonicalize(path) {
                labels.insert(canonical.display().to_string(), (*label).to_owned());
            }
            labels.insert(path.display().to_string(), (*label).to_owned());
        }
        let mut paths: Vec<&String> = labels.keys().collect();
        paths.sort_by_key(|path| std::cmp::Reverse(path.len()));
        let escaped: Vec<String> = paths.into_iter().map(|path| regex::escape(path)).collect();
        let pattern = if escaped.is_empty() {
            format!("{DJF_BIN}|{ID}|{STAMP}")
        } else {
            format!("{DJF_BIN}|(?P<root>{})|{ID}|{STAMP}", escaped.join("|"))
        };
        Ok(Self {
            pattern: Regex::new(&pattern)?,
            labels,
            since: since.to_owned(),
            today: since.get(..10).unwrap_or(since).to_owned(),
            ids: HashMap::new(),
            versions: None,
        })
    }

    pub fn with_versions(mut self, versions: &[&str]) -> Result<Self, regex::Error> {
        let alternatives: Vec<String> = versions
            .iter()
            .map(|version| regex::escape(version))
            .collect();
        self.versions = Some(Regex::new(&format!(
            r#"(?P<lead>"last_version": "|dejafix |upgraded to |Running )(?:{})\b"#,
            alternatives.join("|")
        ))?);
        Ok(self)
    }

    pub fn apply(&mut self, text: &str) -> String {
        let Self {
            pattern,
            labels,
            since,
            today,
            ids,
            versions,
        } = self;
        let text = match versions {
            Some(versions) => versions.replace_all(text, "${lead}<VERSION>"),
            None => std::borrow::Cow::Borrowed(text),
        };
        pattern
            .replace_all(&text, |caps: &Captures| {
                if caps.name("djf").is_some() {
                    return r#"DJF_BIN="<DJF>""#.to_owned();
                }
                if let Some(root) = caps.name("root") {
                    return labels.get(root.as_str()).cloned().unwrap_or_default();
                }
                if let Some(id) = caps.name("id") {
                    let next = ids.len() + 1;
                    let number = ids.entry(id.as_str().to_owned()).or_insert(next);
                    return format!("<ID {number}>");
                }
                let stamp = caps.name("stamp").map_or("", |found| found.as_str());
                match stamp.len() {
                    10 if stamp >= today.as_str() => "<TODAY>".to_owned(),
                    16 if stamp >= minute(since).as_str() => "<MINUTE>".to_owned(),
                    20 | 27 if stamp.get(..19) >= since.get(..19) => "<NOW>".to_owned(),
                    _ => stamp.to_owned(),
                }
            })
            .into_owned()
    }
}

fn minute(since: &str) -> String {
    let date = since.get(..10).unwrap_or(since);
    let time = since.get(11..16).unwrap_or("");
    format!("{date} {time}")
}
