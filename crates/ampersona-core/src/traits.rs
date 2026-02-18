use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::actions::ActionId;
use crate::errors::{AuditError, MetricError, PolicyDecision, PolicyError};
use crate::types::AuditEventType;

/// Query for a specific metric value.
#[derive(Debug, Clone)]
pub struct MetricQuery {
    pub name: String,
    pub window: Option<Duration>,
}

/// Typed metric sample with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub value: serde_json::Value,
    pub sampled_at: DateTime<Utc>,
}

/// Provides metric values for gate evaluation.
pub trait MetricsProvider {
    fn get_metric(&self, query: &MetricQuery) -> Result<MetricSample, MetricError>;
}

/// Unified policy request.
#[derive(Debug, Clone)]
pub struct PolicyRequest {
    pub action: Option<ActionId>,
    pub path: Option<String>,
    pub context: HashMap<String, serde_json::Value>,
}

/// Authority with all layers resolved (workspace+persona+gate+elevation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAuthority {
    pub autonomy: crate::types::AutonomyLevel,
    pub allowed_actions: Vec<ActionId>,
    pub denied_actions: Vec<ActionId>,
    pub scope: Option<crate::spec::authority::Scope>,
    pub limits: Option<crate::spec::authority::Limits>,
    pub scoped_actions: HashMap<String, crate::spec::authority::ScopedAction>,
}

/// Evaluates policy requests against resolved authority.
pub trait AuthorityEnforcer {
    fn evaluate(
        &self,
        req: &PolicyRequest,
        authority: &ResolvedAuthority,
    ) -> Result<PolicyDecision, PolicyError>;
}

/// Audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub event_type: AuditEventType,
    pub decision: String,
    pub reason: String,
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
}

/// Gate transition event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateTransitionEvent {
    pub gate_id: String,
    pub direction: crate::types::GateDirection,
    pub enforcement: crate::types::GateEnforcement,
    pub decision: String,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub metrics_snapshot: HashMap<String, serde_json::Value>,
    pub criteria_results: Vec<CriteriaResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    pub is_override: bool,
    pub state_rev: u64,
    pub metrics_hash: String,
    pub ts: DateTime<Utc>,
}

/// Result of evaluating a single criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriteriaResult {
    pub metric: String,
    pub op: crate::types::CriterionOp,
    pub value: serde_json::Value,
    pub actual: serde_json::Value,
    pub pass: bool,
}

/// Elevation lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationEvent {
    pub elevation_id: String,
    pub action: String,
    pub reason: String,
    pub ts: DateTime<Utc>,
}

/// Override event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideEvent {
    pub gate_id: String,
    pub reason: String,
    pub approver: String,
    pub metrics_snapshot: HashMap<String, serde_json::Value>,
    pub ts: DateTime<Utc>,
}

/// Receives audit events for persistence.
pub trait AuditSink {
    fn log_decision(&self, entry: &AuditEntry) -> Result<(), AuditError>;
    fn log_gate_transition(&self, event: &GateTransitionEvent) -> Result<(), AuditError>;
    fn log_elevation(&self, event: &ElevationEvent) -> Result<(), AuditError>;
    fn log_override(&self, event: &OverrideEvent) -> Result<(), AuditError>;
}
