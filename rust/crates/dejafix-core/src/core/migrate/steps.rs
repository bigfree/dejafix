use std::fmt;

use super::types::MigrateStep;

impl fmt::Display for MigrateStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rename => formatter.write_str("rename .projectmem/ -> .dejafix/"),
            Self::Paths(count) => write!(
                formatter,
                "update .projectmem paths recorded in {count} memory files"
            ),
            Self::Instructions => {
                formatter.write_str("refresh AI_INSTRUCTIONS.md (previous copy kept as .bak)")
            }
            Self::Gitignore => formatter.write_str("update .gitignore entries"),
            Self::Bridges(files) => {
                write!(
                    formatter,
                    "replace the bridge block in {}",
                    files.join(", ")
                )
            }
            Self::Hooks(names) => write!(formatter, "reinstall git hooks: {}", names.join(", ")),
        }
    }
}
