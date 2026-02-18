use ampersona_core::state::{ActiveElevation, PhaseState};
use chrono::{Duration, Utc};

/// Activate an elevation grant.
pub fn activate(
    state: &mut PhaseState,
    elevation_id: &str,
    ttl_seconds: i64,
    reason: &str,
    granted_by: &str,
) {
    let now = Utc::now();
    state.active_elevations.push(ActiveElevation {
        elevation_id: elevation_id.to_string(),
        granted_at: now,
        expires_at: now + Duration::seconds(ttl_seconds),
        reason: reason.to_string(),
        granted_by: granted_by.to_string(),
    });
}

/// Remove expired elevations, returns the list of expired IDs.
pub fn enforce_ttl(state: &mut PhaseState) -> Vec<String> {
    let mut expired = Vec::new();
    state.active_elevations.retain(|e| {
        if e.is_expired() {
            expired.push(e.elevation_id.clone());
            false
        } else {
            true
        }
    });
    expired
}
