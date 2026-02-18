use std::collections::HashMap;

use ampersona_core::actions::ActionId;
use ampersona_core::spec::authority::{Authority, Elevation};
use ampersona_core::state::ActiveElevation;
use ampersona_core::traits::ResolvedAuthority;
use ampersona_core::types::AutonomyLevel;

/// Resolve authority from multiple layers (workspace defaults → persona → gate overlay → elevation).
///
/// Merge rules:
/// - deny = union (all denies from all layers)
/// - allow = intersection minus deny
/// - limits = minimum
/// - autonomy = minimum
pub fn resolve_authority(layers: &[&Authority]) -> ResolvedAuthority {
    let mut autonomy = AutonomyLevel::Full;
    let mut all_denied: Vec<ActionId> = Vec::new();
    let mut all_allowed: Option<Vec<ActionId>> = None;
    let mut scope = None;
    let mut limits = None;
    let mut scoped_actions = HashMap::new();

    for layer in layers {
        // Autonomy: minimum
        autonomy = autonomy.min(layer.autonomy);

        if let Some(actions) = &layer.actions {
            // Deny: union
            if let Some(deny) = &actions.deny {
                for entry in deny {
                    all_denied.push(entry.action_id().clone());
                }
            }

            // Allow: intersection
            if let Some(allow) = &actions.allow {
                match &mut all_allowed {
                    None => {
                        all_allowed = Some(allow.clone());
                    }
                    Some(existing) => {
                        existing.retain(|a| allow.contains(a));
                    }
                }
            }

            // Scoped: merge
            if let Some(s) = &actions.scoped {
                for (k, v) in s {
                    scoped_actions.insert(k.clone(), v.clone());
                }
            }
        }

        // Scope: last layer wins (overlay replaces)
        if layer.scope.is_some() {
            scope = layer.scope.clone();
        }

        // Limits: minimum
        if let Some(l) = &layer.limits {
            limits = Some(merge_limits_opt(limits.as_ref(), l));
        }
    }

    // Remove denied actions from allowed
    let allowed_actions = all_allowed
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !all_denied.contains(a))
        .collect();

    ResolvedAuthority {
        autonomy,
        allowed_actions,
        denied_actions: all_denied,
        scope,
        limits,
        scoped_actions,
    }
}

/// Resolve authority with active elevations applied.
///
/// Active (non-expired) elevations add their granted actions to the allow list.
/// Elevations are applied AFTER the base authority resolution and BEFORE deny checks.
///
/// Precedence (highest → lowest):
/// 1. Explicit deny — always wins
/// 2. Active elevation grants (within TTL)
/// 3. Gate authority_overlay (phase-specific)
/// 4. Persona authority
/// 5. Workspace defaults (.ampersona/defaults.json)
pub fn resolve_with_elevations(
    layers: &[&Authority],
    active_elevations: &[ActiveElevation],
    elevation_defs: &[Elevation],
) -> ResolvedAuthority {
    let mut resolved = resolve_authority(layers);

    // Apply active (non-expired) elevation grants
    for active in active_elevations {
        if active.is_expired() {
            continue;
        }

        // Find the elevation definition
        if let Some(elev_def) = elevation_defs.iter().find(|e| e.id == active.elevation_id) {
            // Parse grants to extract action allow list additions
            if let Some(grants_obj) = elev_def.grants.as_object() {
                if let Some(allow_val) = grants_obj.get("actions.allow") {
                    if let Some(allow_arr) = allow_val.as_array() {
                        for action in allow_arr {
                            if let Some(action_str) = action.as_str() {
                                if let Ok(action_id) = action_str.parse::<ActionId>() {
                                    // Elevation grants add to allowed (unless explicitly denied)
                                    if !resolved.denied_actions.contains(&action_id)
                                        && !resolved.allowed_actions.contains(&action_id)
                                    {
                                        resolved.allowed_actions.push(action_id);
                                    }
                                }
                            }
                        }
                    }
                }

                // Support autonomy elevation
                if let Some(autonomy_val) = grants_obj.get("autonomy") {
                    if let Some(autonomy_str) = autonomy_val.as_str() {
                        if let Ok(new_autonomy) =
                            serde_json::from_value::<AutonomyLevel>(serde_json::json!(autonomy_str))
                        {
                            // Elevation can only INCREASE autonomy, never decrease
                            if new_autonomy > resolved.autonomy {
                                resolved.autonomy = new_autonomy;
                            }
                        }
                    }
                }
            }
        }
    }

    resolved
}

/// Load workspace defaults from .ampersona/defaults.json and return as Authority if present.
pub fn load_workspace_defaults() -> Option<Authority> {
    let path = ".ampersona/defaults.json";
    let content = std::fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    let authority = data.get("authority")?;
    serde_json::from_value(authority.clone()).ok()
}

fn merge_limits_opt(
    existing: Option<&ampersona_core::spec::authority::Limits>,
    new: &ampersona_core::spec::authority::Limits,
) -> ampersona_core::spec::authority::Limits {
    match existing {
        None => new.clone(),
        Some(e) => ampersona_core::spec::authority::Limits {
            max_actions_per_hour: min_opt(e.max_actions_per_hour, new.max_actions_per_hour),
            max_cost_per_day_cents: min_opt(e.max_cost_per_day_cents, new.max_cost_per_day_cents),
            require_approval_for: e
                .require_approval_for
                .clone()
                .or_else(|| new.require_approval_for.clone()),
        },
    }
}

fn min_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ampersona_core::spec::authority::{Actions, DenyEntry, Limits};
    use chrono::{Duration, Utc};

    fn make_authority(autonomy: AutonomyLevel, allow: Vec<&str>, deny: Vec<&str>) -> Authority {
        Authority {
            autonomy,
            scope: None,
            actions: Some(Actions {
                allow: Some(allow.into_iter().filter_map(|s| s.parse().ok()).collect()),
                deny: Some(
                    deny.into_iter()
                        .map(|s| DenyEntry::Simple(s.parse().unwrap()))
                        .collect(),
                ),
                scoped: None,
            }),
            limits: None,
            elevations: None,
            delegation: None,
            ext: None,
        }
    }

    #[test]
    fn deny_is_union() {
        let a = make_authority(AutonomyLevel::Full, vec!["read_file"], vec!["deploy"]);
        let b = make_authority(
            AutonomyLevel::Full,
            vec!["read_file"],
            vec!["git_push_main"],
        );
        let resolved = resolve_authority(&[&a, &b]);
        assert_eq!(resolved.denied_actions.len(), 2);
    }

    #[test]
    fn allow_is_intersection() {
        let a = make_authority(AutonomyLevel::Full, vec!["read_file", "write_file"], vec![]);
        let b = make_authority(AutonomyLevel::Full, vec!["read_file"], vec![]);
        let resolved = resolve_authority(&[&a, &b]);
        assert_eq!(resolved.allowed_actions.len(), 1);
        assert_eq!(resolved.allowed_actions[0].to_string(), "read_file");
    }

    #[test]
    fn autonomy_is_min() {
        let a = make_authority(AutonomyLevel::Full, vec![], vec![]);
        let b = make_authority(AutonomyLevel::Supervised, vec![], vec![]);
        let resolved = resolve_authority(&[&a, &b]);
        assert_eq!(resolved.autonomy, AutonomyLevel::Supervised);
    }

    #[test]
    fn limits_are_min() {
        let a = Authority {
            autonomy: AutonomyLevel::Full,
            scope: None,
            actions: None,
            limits: Some(Limits {
                max_actions_per_hour: Some(100),
                max_cost_per_day_cents: Some(1000),
                require_approval_for: None,
            }),
            elevations: None,
            delegation: None,
            ext: None,
        };
        let b = Authority {
            autonomy: AutonomyLevel::Full,
            scope: None,
            actions: None,
            limits: Some(Limits {
                max_actions_per_hour: Some(50),
                max_cost_per_day_cents: Some(2000),
                require_approval_for: None,
            }),
            elevations: None,
            delegation: None,
            ext: None,
        };
        let resolved = resolve_authority(&[&a, &b]);
        assert_eq!(
            resolved.limits.as_ref().unwrap().max_actions_per_hour,
            Some(50)
        );
        assert_eq!(
            resolved.limits.as_ref().unwrap().max_cost_per_day_cents,
            Some(1000)
        );
    }

    #[test]
    fn deny_removes_from_allowed() {
        let a = make_authority(
            AutonomyLevel::Full,
            vec!["read_file", "write_file"],
            vec!["write_file"],
        );
        let resolved = resolve_authority(&[&a]);
        assert_eq!(resolved.allowed_actions.len(), 1);
        assert_eq!(resolved.allowed_actions[0].to_string(), "read_file");
    }

    #[test]
    fn elevation_grants_add_actions() {
        let base = make_authority(
            AutonomyLevel::Supervised,
            vec!["read_file", "write_file"],
            vec![],
        );
        let elevation_defs = vec![Elevation {
            id: "release-deploy".into(),
            grants: serde_json::json!({"actions.allow": ["git_push_main"]}),
            requires: ampersona_core::types::GateApproval::Human,
            ttl_seconds: 3600,
            reason_required: true,
        }];
        let active = vec![ActiveElevation {
            elevation_id: "release-deploy".into(),
            granted_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1),
            reason: "release".into(),
            granted_by: "admin".into(),
        }];

        let resolved = resolve_with_elevations(&[&base], &active, &elevation_defs);
        let action_names: Vec<String> = resolved
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(action_names.contains(&"git_push_main".to_string()));
    }

    #[test]
    fn expired_elevation_ignored() {
        let base = make_authority(AutonomyLevel::Supervised, vec!["read_file"], vec![]);
        let elevation_defs = vec![Elevation {
            id: "release-deploy".into(),
            grants: serde_json::json!({"actions.allow": ["git_push_main"]}),
            requires: ampersona_core::types::GateApproval::Human,
            ttl_seconds: 3600,
            reason_required: true,
        }];
        let active = vec![ActiveElevation {
            elevation_id: "release-deploy".into(),
            granted_at: Utc::now() - Duration::hours(2),
            expires_at: Utc::now() - Duration::hours(1), // expired
            reason: "release".into(),
            granted_by: "admin".into(),
        }];

        let resolved = resolve_with_elevations(&[&base], &active, &elevation_defs);
        let action_names: Vec<String> = resolved
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(!action_names.contains(&"git_push_main".to_string()));
    }

    #[test]
    fn elevation_denied_action_not_granted() {
        let base = make_authority(
            AutonomyLevel::Full,
            vec!["read_file"],
            vec!["git_push_main"], // explicitly denied
        );
        let elevation_defs = vec![Elevation {
            id: "release-deploy".into(),
            grants: serde_json::json!({"actions.allow": ["git_push_main"]}),
            requires: ampersona_core::types::GateApproval::Human,
            ttl_seconds: 3600,
            reason_required: true,
        }];
        let active = vec![ActiveElevation {
            elevation_id: "release-deploy".into(),
            granted_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1),
            reason: "release".into(),
            granted_by: "admin".into(),
        }];

        let resolved = resolve_with_elevations(&[&base], &active, &elevation_defs);
        // deny always wins — elevation cannot override explicit deny
        let action_names: Vec<String> = resolved
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(!action_names.contains(&"git_push_main".to_string()));
    }
}
