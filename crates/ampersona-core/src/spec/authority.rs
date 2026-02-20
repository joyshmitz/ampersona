use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::actions::ActionId;
use crate::types::{AutonomyLevel, GateApproval, RiskLevel};

/// Partial authority overlay applied as a patch after resolution.
///
/// All fields are Optional. Only present fields replace the resolved authority.
/// See ADR-010 for semantics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthorityOverlay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<AutonomyLevel>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Actions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    pub autonomy: AutonomyLevel,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Actions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevations: Option<Vec<Elevation>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation: Option<Delegation>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    #[serde(default = "default_true")]
    pub workspace_only: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_paths: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub forbidden_paths: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<ActionId>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<DenyEntry>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scoped: Option<HashMap<String, ScopedAction>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DenyEntry {
    Simple(ActionId),
    WithReason {
        action: ActionId,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        compliance_ref: Option<String>,
    },
}

impl DenyEntry {
    pub fn action_id(&self) -> &ActionId {
        match self {
            DenyEntry::Simple(id) => id,
            DenyEntry::WithReason { action, .. } => action,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "$type")]
pub enum ScopedAction {
    #[serde(rename = "shell")]
    Shell(ScopedShell),
    #[serde(rename = "git")]
    Git(ScopedGit),
    #[serde(rename = "file_access")]
    FileAccess(ScopedFileAccess),
    #[serde(rename = "custom")]
    Custom(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedShell {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_high_risk: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_subshells: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_redirects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_background: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validate_symlinks: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedGit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_operations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_branches: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny_push_branches: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedFileAccess {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny_write: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_actions_per_hour: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cost_per_day_cents: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_approval_for: Option<Vec<RiskLevel>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Elevation {
    pub id: String,
    pub grants: serde_json::Value,
    pub requires: GateApproval,
    pub ttl_seconds: u64,
    #[serde(default)]
    pub reason_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_delegate_to: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,
}
