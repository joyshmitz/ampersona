use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// An action identifier: either a builtin name or a custom-namespaced action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionId {
    Builtin(BuiltinAction),
    Custom { vendor: String, action: String },
}

impl ActionId {
    pub fn is_builtin(&self) -> bool {
        matches!(self, ActionId::Builtin(_))
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, ActionId::Custom { .. })
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionId::Builtin(b) => write!(f, "{}", b.as_str()),
            ActionId::Custom { vendor, action } => write!(f, "custom:{vendor}/{action}"),
        }
    }
}

impl FromStr for ActionId {
    type Err = ActionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix("custom:") {
            let (vendor, action) = rest
                .split_once('/')
                .ok_or(ActionParseError::InvalidCustomFormat)?;
            if vendor.is_empty() || action.is_empty() {
                return Err(ActionParseError::InvalidCustomFormat);
            }
            Ok(ActionId::Custom {
                vendor: vendor.to_string(),
                action: action.to_string(),
            })
        } else if let Some(builtin) = BuiltinAction::from_str_opt(s) {
            Ok(ActionId::Builtin(builtin))
        } else {
            Err(ActionParseError::UnknownAction(s.to_string()))
        }
    }
}

impl Serialize for ActionId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ActionId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        // For deserialization, accept unknown actions as builtin parse failures
        // and store them as custom if they have the prefix, otherwise error
        ActionId::from_str(&s).or_else(|_| {
            // Accept unknown non-custom actions for forward compat during deserialization
            // They will be caught by `amp check --strict`
            Ok(ActionId::Custom {
                vendor: "_unknown".to_string(),
                action: s,
            })
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActionParseError {
    #[error("invalid custom action format (expected custom:<vendor>/<action>)")]
    InvalidCustomFormat,
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

/// Well-known builtin actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinAction {
    ReadFile,
    WriteFile,
    DeleteFile,
    RunTests,
    RunCommand,
    GitCommit,
    GitPush,
    GitPushMain,
    GitPull,
    CreateBranch,
    DeleteBranch,
    CreatePr,
    MergePr,
    Deploy,
    InstallPackage,
    ModifyConfig,
    AccessNetwork,
    SendMessage,
    ApproveChange,
    DeleteProductionData,
    AutoApproveCapa,
}

impl BuiltinAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::DeleteFile => "delete_file",
            Self::RunTests => "run_tests",
            Self::RunCommand => "run_command",
            Self::GitCommit => "git_commit",
            Self::GitPush => "git_push",
            Self::GitPushMain => "git_push_main",
            Self::GitPull => "git_pull",
            Self::CreateBranch => "create_branch",
            Self::DeleteBranch => "delete_branch",
            Self::CreatePr => "create_pr",
            Self::MergePr => "merge_pr",
            Self::Deploy => "deploy",
            Self::InstallPackage => "install_package",
            Self::ModifyConfig => "modify_config",
            Self::AccessNetwork => "access_network",
            Self::SendMessage => "send_message",
            Self::ApproveChange => "approve_change",
            Self::DeleteProductionData => "delete_production_data",
            Self::AutoApproveCapa => "auto_approve_capa",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "read_file" => Some(Self::ReadFile),
            "write_file" => Some(Self::WriteFile),
            "delete_file" => Some(Self::DeleteFile),
            "run_tests" => Some(Self::RunTests),
            "run_command" => Some(Self::RunCommand),
            "git_commit" => Some(Self::GitCommit),
            "git_push" => Some(Self::GitPush),
            "git_push_main" => Some(Self::GitPushMain),
            "git_pull" => Some(Self::GitPull),
            "create_branch" => Some(Self::CreateBranch),
            "delete_branch" => Some(Self::DeleteBranch),
            "create_pr" => Some(Self::CreatePr),
            "merge_pr" => Some(Self::MergePr),
            "deploy" => Some(Self::Deploy),
            "install_package" => Some(Self::InstallPackage),
            "modify_config" => Some(Self::ModifyConfig),
            "access_network" => Some(Self::AccessNetwork),
            "send_message" => Some(Self::SendMessage),
            "approve_change" => Some(Self::ApproveChange),
            "delete_production_data" => Some(Self::DeleteProductionData),
            "auto_approve_capa" => Some(Self::AutoApproveCapa),
            _ => None,
        }
    }

    /// All builtin actions for enumeration.
    pub fn all() -> &'static [BuiltinAction] {
        &[
            Self::ReadFile,
            Self::WriteFile,
            Self::DeleteFile,
            Self::RunTests,
            Self::RunCommand,
            Self::GitCommit,
            Self::GitPush,
            Self::GitPushMain,
            Self::GitPull,
            Self::CreateBranch,
            Self::DeleteBranch,
            Self::CreatePr,
            Self::MergePr,
            Self::Deploy,
            Self::InstallPackage,
            Self::ModifyConfig,
            Self::AccessNetwork,
            Self::SendMessage,
            Self::ApproveChange,
            Self::DeleteProductionData,
            Self::AutoApproveCapa,
        ]
    }

    /// Suggest the closest builtin for a misspelled action name.
    pub fn suggest(input: &str) -> Option<&'static str> {
        let input_lower = input.to_lowercase();
        Self::all()
            .iter()
            .map(|b| (b.as_str(), edit_distance(&input_lower, b.as_str())))
            .filter(|(_, d)| *d <= 3)
            .min_by_key(|(_, d)| *d)
            .map(|(name, _)| name)
    }
}

/// Simple Levenshtein distance for typo suggestions.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for (i, row) in dp.iter_mut().enumerate().take(a.len() + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(b.len() + 1) {
        *val = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_builtin() {
        let id: ActionId = "read_file".parse().unwrap();
        assert!(id.is_builtin());
        assert_eq!(id.to_string(), "read_file");
    }

    #[test]
    fn parse_custom() {
        let id: ActionId = "custom:zeroclaw/sandbox_escape".parse().unwrap();
        assert!(id.is_custom());
        assert_eq!(id.to_string(), "custom:zeroclaw/sandbox_escape");
    }

    #[test]
    fn parse_invalid_custom() {
        assert!("custom:noslash".parse::<ActionId>().is_err());
        assert!("custom:/empty".parse::<ActionId>().is_err());
        assert!("custom:empty/".parse::<ActionId>().is_err());
    }

    #[test]
    fn suggest_typo() {
        assert_eq!(BuiltinAction::suggest("delet_data"), None);
        assert_eq!(BuiltinAction::suggest("read_fil"), Some("read_file"));
        assert_eq!(BuiltinAction::suggest("git_pussh"), Some("git_push"));
    }

    #[test]
    fn serde_roundtrip() {
        let id: ActionId = "write_file".parse().unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"write_file\"");
    }
}
