use regex::Regex;

use super::types::Glob;

fn literal_prefix(pattern: &str) -> String {
    let segments: Vec<&str> = pattern.split('/').collect();
    let literal = segments
        .iter()
        .take_while(|segment| !segment.contains(['*', '?']))
        .count();
    let keep = if literal == segments.len() {
        literal.saturating_sub(1)
    } else {
        literal
    };
    segments
        .iter()
        .take(keep)
        .fold(String::new(), |mut prefix, segment| {
            prefix.push_str(segment);
            prefix.push('/');
            prefix
        })
}

fn translated(pattern: &str, ignore_case: bool) -> String {
    let mut regex = String::from(if ignore_case { "(?i)\\A" } else { "\\A" });
    let mut chars = pattern.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    regex.push_str("(?:.*/)?");
                } else {
                    regex.push_str(".*");
                }
            }
            '*' => regex.push_str("[^/]*"),
            '?' => regex.push_str("[^/]"),
            other => regex.push_str(&regex::escape(other.encode_utf8(&mut [0; 4]))),
        }
    }
    regex.push_str("\\z");
    regex
}

impl Glob {
    #[must_use]
    pub fn new(pattern: &str, ignore_case: bool) -> Option<Self> {
        let trimmed = pattern
            .trim()
            .trim_start_matches("./")
            .trim_start_matches('/');
        if trimmed.is_empty() {
            return None;
        }
        let pattern = if trimmed.ends_with('/') {
            format!("{trimmed}**")
        } else {
            trimmed.to_owned()
        };
        let regex = Regex::new(&translated(&pattern, ignore_case)).ok()?;
        Some(Self {
            regex,
            prefix: literal_prefix(&pattern),
            ignore_case,
        })
    }

    #[must_use]
    pub fn matches(&self, relative: &str) -> bool {
        self.regex.is_match(&relative.replace('\\', "/"))
    }

    #[must_use]
    pub fn rest<'p>(&self, relative: &'p str) -> &'p str {
        let Some(head) = relative.get(..self.prefix.len()) else {
            return relative;
        };
        let same = if self.ignore_case {
            head.eq_ignore_ascii_case(&self.prefix)
        } else {
            head == self.prefix
        };
        if same {
            relative.get(self.prefix.len()..).unwrap_or(relative)
        } else {
            relative
        }
    }
}

#[must_use]
pub fn boundary_globs(patterns: &[String]) -> Vec<Glob> {
    patterns
        .iter()
        .filter_map(|pattern| Glob::new(pattern, cfg!(windows)))
        .collect()
}

#[must_use]
pub fn matching<'g>(globs: &'g [Glob], relative: &str) -> Option<&'g Glob> {
    globs.iter().find(|glob| glob.matches(relative))
}

#[must_use]
pub fn path_query(relative: &str, glob: Option<&Glob>) -> String {
    let relative = relative.replace('\\', "/");
    let rest = glob.map_or(relative.as_str(), |glob| glob.rest(&relative));
    let (folder, file) = rest.rsplit_once('/').unwrap_or(("", rest));
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    let stem = if stem.is_empty() { file } else { stem };
    if folder.is_empty() {
        stem.to_owned()
    } else {
        format!("{folder}/{stem}")
    }
}
