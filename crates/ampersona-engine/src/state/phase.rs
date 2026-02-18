use ampersona_core::state::PhaseState;
use anyhow::{Context, Result};

/// Load phase state from a file.
pub fn load_state(path: &str) -> Result<PhaseState> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read state {path}"))?;
    serde_json::from_str(&content).with_context(|| format!("{path}: invalid state JSON"))
}

/// Save phase state to a file (non-atomic, use atomic module for production).
pub fn save_state(path: &str, state: &PhaseState) -> Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json).with_context(|| format!("cannot write state {path}"))
}
