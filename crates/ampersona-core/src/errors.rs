use std::fmt;

/// Policy decision result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow { reason: String },
    Deny { reason: String },
    NeedsApproval { reason: String },
}

impl fmt::Display for PolicyDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyDecision::Allow { reason } => write!(f, "Allow: {reason}"),
            PolicyDecision::Deny { reason } => write!(f, "Deny: {reason}"),
            PolicyDecision::NeedsApproval { reason } => write!(f, "NeedsApproval: {reason}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("invalid action: {0}")]
    InvalidAction(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MetricError {
    #[error("metric not found: {0}")]
    NotFound(String),
    #[error("type mismatch for metric {0}")]
    TypeMismatch(String),
    #[error("metrics provider unavailable")]
    ProviderUnavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("write failure: {0}")]
    WriteFailure(String),
    #[error("chain corruption at entry {0}")]
    ChainCorruption(u64),
}

/// Structured check result for `amp check --json`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckReport {
    pub file: String,
    pub version: String,
    pub pass: bool,
    pub errors: Vec<CheckIssue>,
    pub warnings: Vec<CheckIssue>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckIssue {
    pub code: String,
    pub check: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
