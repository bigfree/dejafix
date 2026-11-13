use super::types::{CaptureSource, Confidence, EventKind, Outcome};

impl EventKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Hypothesis => "hypothesis",
            Self::Attempt => "attempt",
            Self::Fix => "fix",
            Self::Decision => "decision",
            Self::Note => "note",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "issue" => Self::Issue,
            "hypothesis" => Self::Hypothesis,
            "attempt" => Self::Attempt,
            "fix" => Self::Fix,
            "decision" => Self::Decision,
            "note" => Self::Note,
            _ => return None,
        })
    }
}

impl Outcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Worked => "worked",
            Self::Failed => "failed",
            Self::Partial => "partial",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "worked" => Self::Worked,
            "failed" => Self::Failed,
            "partial" => Self::Partial,
            _ => return None,
        })
    }
}

impl CaptureSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitPostCommit => "git_post_commit",
            Self::GitPostRevert => "git_post_revert",
            Self::GitPostMerge => "git_post_merge",
            Self::ChurnDetector => "churn_detector",
            Self::CiParser => "ci_parser",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "git_post_commit" => Self::GitPostCommit,
            "git_post_revert" => Self::GitPostRevert,
            "git_post_merge" => Self::GitPostMerge,
            "churn_detector" => Self::ChurnDetector,
            "ci_parser" => Self::CiParser,
            _ => return None,
        })
    }
}

impl Confidence {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => return None,
        })
    }
}
