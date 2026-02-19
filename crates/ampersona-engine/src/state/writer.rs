use ampersona_core::spec::audit::AuditConfig;
use ampersona_core::state::PhaseState;
use anyhow::{Context, Result};

use super::atomic::{atomic_write, AdvisoryLock};
use super::audit_log::append_audit;

/// Coordinated state writer: lock → mutate → audit → atomic write → unlock.
///
/// Ensures that all state mutations follow the spec protocol:
/// 1. Acquire advisory lock
/// 2. Load current state
/// 3. Apply mutation (caller responsibility)
/// 4. Write audit entry (respecting AuditConfig)
/// 5. Atomic write new state
/// 6. Release lock (on drop)
pub struct StateWriter {
    state_path: String,
    audit_path: String,
    _lock: AdvisoryLock,
}

impl StateWriter {
    /// Acquire the advisory lock and prepare for state mutation.
    pub fn acquire(state_path: &str) -> Result<Self> {
        let lock = AdvisoryLock::acquire(state_path)
            .with_context(|| format!("cannot lock state {state_path}"))?;
        let audit_path = state_path.replace(".state.json", ".audit.jsonl");
        Ok(Self {
            state_path: state_path.to_string(),
            audit_path,
            _lock: lock,
        })
    }

    /// Write the updated state atomically.
    pub fn write_state(&self, state: &PhaseState) -> Result<()> {
        let json = serde_json::to_string_pretty(state)?;
        atomic_write(&self.state_path, json.as_bytes())
    }

    /// Append an audit entry if the AuditConfig allows it for this event type.
    pub fn maybe_audit(
        &self,
        audit_config: Option<&AuditConfig>,
        event_type: &str,
        entry: &serde_json::Value,
    ) -> Result<()> {
        if should_audit(audit_config, event_type) {
            append_audit(&self.audit_path, entry)?;
        }
        Ok(())
    }

    /// Unconditionally append an audit entry.
    pub fn audit(&self, entry: &serde_json::Value) -> Result<()> {
        append_audit(&self.audit_path, entry)?;
        Ok(())
    }

    /// Returns the audit path for manual operations.
    pub fn audit_path(&self) -> &str {
        &self.audit_path
    }
}

/// Check whether an event type should be audited based on AuditConfig.
fn should_audit(config: Option<&AuditConfig>, event_type: &str) -> bool {
    let Some(config) = config else {
        // No audit config → audit everything (safe default)
        return true;
    };
    match event_type {
        "GateTransition" => config.log_gate_transitions,
        "PolicyDecision" => config.log_decisions,
        // These event types are always audited (security-critical)
        "Override" | "ElevationChange" | "SignatureVerify" | "StateChange" => true,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_audit_defaults_to_true() {
        assert!(should_audit(None, "GateTransition"));
        assert!(should_audit(None, "Override"));
        assert!(should_audit(None, "PolicyDecision"));
    }

    #[test]
    fn should_audit_respects_config() {
        let config = AuditConfig {
            log_decisions: false,
            log_gate_transitions: true,
            retention_days: None,
            compliance_markers: None,
        };
        assert!(should_audit(Some(&config), "GateTransition"));
        assert!(!should_audit(Some(&config), "PolicyDecision"));
        // Security-critical events always audited
        assert!(should_audit(Some(&config), "Override"));
        assert!(should_audit(Some(&config), "ElevationChange"));
    }

    #[test]
    fn state_writer_lock_and_write() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("test.state.json");
        let state_str = state_path.to_str().unwrap();

        let state = PhaseState::new("test".to_string());
        let json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&state_path, &json).unwrap();

        let writer = StateWriter::acquire(state_str).unwrap();
        let mut new_state = state.clone();
        new_state.state_rev = 1;
        writer.write_state(&new_state).unwrap();

        // Verify state was written
        let loaded = super::super::phase::load_state(state_str).unwrap();
        assert_eq!(loaded.state_rev, 1);
    }
}
