use std::collections::HashMap;

use ampersona_core::actions::ActionId;
use ampersona_core::spec::authority::{Authority, AuthorityOverlay, DenyEntry, Elevation};
use ampersona_core::state::ActiveElevation;
use ampersona_core::traits::{DenyMeta, ResolvedAuthority};
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
    let mut deny_metadata = HashMap::new();

    for layer in layers {
        // Autonomy: minimum
        autonomy = autonomy.min(layer.autonomy);

        if let Some(actions) = &layer.actions {
            // Deny: union (with metadata preservation)
            if let Some(deny) = &actions.deny {
                for entry in deny {
                    let id = entry.action_id().clone();
                    if let DenyEntry::WithReason {
                        reason,
                        compliance_ref,
                        ..
                    } = entry
                    {
                        deny_metadata.insert(
                            id.to_string(),
                            DenyMeta {
                                reason: Some(reason.clone()),
                                compliance_ref: compliance_ref.clone(),
                            },
                        );
                    }
                    all_denied.push(id);
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
        deny_metadata,
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

/// Apply an authority overlay as a post-resolution patch.
///
/// Overlay uses patch-replace semantics (ADR-010):
/// - Present fields REPLACE the resolved value
/// - Deny is additive (union) — explicit deny never weakened
/// - Actions.allow replaces allowed minus deny
/// - Absent fields leave resolved values unchanged
pub fn apply_overlay(base: &ResolvedAuthority, overlay: &AuthorityOverlay) -> ResolvedAuthority {
    let mut result = base.clone();

    // Autonomy: REPLACE (not min)
    if let Some(autonomy) = overlay.autonomy {
        result.autonomy = autonomy;
    }

    if let Some(ref actions) = overlay.actions {
        // Deny: UNION (additive — deny never weakened)
        if let Some(ref deny) = actions.deny {
            for entry in deny {
                let id = entry.action_id().clone();
                if !result.denied_actions.contains(&id) {
                    result.denied_actions.push(id.clone());
                }
                result.allowed_actions.retain(|a| a != &id);

                // Preserve deny metadata
                if let DenyEntry::WithReason {
                    reason,
                    compliance_ref,
                    ..
                } = entry
                {
                    result.deny_metadata.insert(
                        id.to_string(),
                        DenyMeta {
                            reason: Some(reason.clone()),
                            compliance_ref: compliance_ref.clone(),
                        },
                    );
                }
            }
        }

        // Allow: REPLACE (minus deny — deny always wins)
        if let Some(ref allow) = actions.allow {
            result.allowed_actions = allow
                .iter()
                .filter(|a| !result.denied_actions.contains(a))
                .cloned()
                .collect();
        }
    }

    // Scope: REPLACE
    if let Some(ref scope) = overlay.scope {
        result.scope = Some(scope.clone());
    }

    // Limits: REPLACE
    if let Some(ref limits) = overlay.limits {
        result.limits = Some(limits.clone());
    }

    result
}

/// Load workspace defaults from .ampersona/defaults.json and return as Authority if present.
/// Logs a warning to stderr if the file exists but cannot be parsed.
pub fn load_workspace_defaults() -> Option<Authority> {
    let path = ".ampersona/defaults.json";
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None, // file doesn't exist — not an error
    };
    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("  warn: {path}: unparseable JSON: {e}");
            return None;
        }
    };
    let authority = match data.get("authority") {
        Some(a) => a,
        None => {
            eprintln!("  warn: {path}: missing 'authority' key");
            return None;
        }
    };
    match serde_json::from_value(authority.clone()) {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("  warn: {path}: invalid authority: {e}");
            None
        }
    }
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
    fn workspace_defaults_restrict_persona_authority() {
        let workspace_defaults = make_authority(AutonomyLevel::Readonly, vec!["read_file"], vec![]);
        let persona = make_authority(AutonomyLevel::Full, vec!["read_file", "write_file"], vec![]);

        let resolved = resolve_authority(&[&workspace_defaults, &persona]);
        assert_eq!(resolved.autonomy, AutonomyLevel::Readonly);
        assert_eq!(resolved.allowed_actions.len(), 1);
        assert_eq!(resolved.allowed_actions[0].to_string(), "read_file");
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

    // ── Overlay tests: ADR-010 patch-replace semantics ────────
    // These test apply_overlay() which uses patch-replace (not meet-semilattice).
    // Before the fix, these tested resolve_authority and FAILED.
    // After ADR-010, overlay is a post-resolution patch via apply_overlay().

    fn make_overlay(
        autonomy: Option<AutonomyLevel>,
        allow: Option<Vec<&str>>,
        deny: Option<Vec<&str>>,
    ) -> AuthorityOverlay {
        let actions = if allow.is_some() || deny.is_some() {
            Some(Actions {
                allow: allow.map(|v| v.into_iter().filter_map(|s| s.parse().ok()).collect()),
                deny: deny.map(|v| {
                    v.into_iter()
                        .map(|s| DenyEntry::Simple(s.parse().unwrap()))
                        .collect()
                }),
                scoped: None,
            })
        } else {
            None
        };
        AuthorityOverlay {
            autonomy,
            scope: None,
            actions,
            limits: None,
        }
    }

    #[test]
    fn overlay_expands_autonomy() {
        // Persona: supervised, overlay: full → effective should be full.
        // ADR-010: overlay REPLACES autonomy (not min).
        let persona = make_authority(AutonomyLevel::Supervised, vec!["read_file"], vec![]);
        let base = resolve_authority(&[&persona]);
        assert_eq!(base.autonomy, AutonomyLevel::Supervised);

        let overlay = make_overlay(Some(AutonomyLevel::Full), None, None);
        let effective = apply_overlay(&base, &overlay);
        assert_eq!(
            effective.autonomy,
            AutonomyLevel::Full,
            "overlay should expand autonomy from supervised to full"
        );
    }

    #[test]
    fn overlay_adds_allowed_actions() {
        // Persona: [read_file], overlay: [read_file, deploy] → deploy should be allowed.
        // ADR-010: overlay REPLACES allowed (not intersection).
        let persona = make_authority(AutonomyLevel::Full, vec!["read_file"], vec![]);
        let base = resolve_authority(&[&persona]);

        let overlay = make_overlay(None, Some(vec!["read_file", "deploy"]), None);
        let effective = apply_overlay(&base, &overlay);
        let action_names: Vec<String> = effective
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(
            action_names.contains(&"deploy".to_string()),
            "overlay should add deploy to allowed actions, got: {action_names:?}"
        );
    }

    #[test]
    fn overlay_cannot_override_deny() {
        // Persona denies deploy, overlay allows deploy → deny wins.
        // ADR-010 invariant: deny(persona) ⊆ deny(effective).
        let persona = make_authority(AutonomyLevel::Full, vec!["read_file"], vec!["deploy"]);
        let base = resolve_authority(&[&persona]);

        let overlay = make_overlay(None, Some(vec!["read_file", "deploy"]), None);
        let effective = apply_overlay(&base, &overlay);
        let denied_names: Vec<String> = effective
            .denied_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(
            denied_names.contains(&"deploy".to_string()),
            "explicit deny must survive overlay"
        );
        let allowed_names: Vec<String> = effective
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(
            !allowed_names.contains(&"deploy".to_string()),
            "denied action must not appear in allowed"
        );
    }

    #[test]
    fn overlay_with_only_actions_preserves_autonomy() {
        // Partial overlay: only actions, no autonomy → autonomy unchanged.
        let persona = make_authority(AutonomyLevel::Supervised, vec!["read_file"], vec![]);
        let base = resolve_authority(&[&persona]);

        let overlay = make_overlay(None, Some(vec!["read_file", "deploy"]), None);
        let effective = apply_overlay(&base, &overlay);
        assert_eq!(
            effective.autonomy,
            AutonomyLevel::Supervised,
            "autonomy should be unchanged when overlay doesn't set it"
        );
        let action_names: Vec<String> = effective
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(action_names.contains(&"deploy".to_string()));
    }

    #[test]
    fn overlay_replaces_not_merges() {
        // Second overlay replaces first completely (no stacking).
        let persona = make_authority(AutonomyLevel::Supervised, vec!["read_file"], vec![]);
        let base = resolve_authority(&[&persona]);

        let overlay1 = make_overlay(
            Some(AutonomyLevel::Full),
            Some(vec!["read_file", "deploy"]),
            None,
        );
        let after_first = apply_overlay(&base, &overlay1);
        assert_eq!(after_first.autonomy, AutonomyLevel::Full);

        // Second overlay: only sets autonomy to readonly, doesn't mention actions
        let overlay2 = make_overlay(Some(AutonomyLevel::Readonly), None, None);
        let after_second = apply_overlay(&base, &overlay2);
        // Should be based on base, not on after_first (no stacking)
        assert_eq!(after_second.autonomy, AutonomyLevel::Readonly);
        // Actions should be from base (overlay2 doesn't set actions)
        assert_eq!(after_second.allowed_actions, base.allowed_actions);
    }

    #[test]
    fn overlay_lifecycle_promote_demote() {
        // Promote sets overlay, demote replaces it, clearing the promote overlay.
        let persona = make_authority(AutonomyLevel::Supervised, vec!["read_file"], vec![]);
        let base = resolve_authority(&[&persona]);

        // Promote overlay: expand to full autonomy + deploy
        let promote_overlay = make_overlay(
            Some(AutonomyLevel::Full),
            Some(vec!["read_file", "deploy"]),
            None,
        );
        let after_promote = apply_overlay(&base, &promote_overlay);
        assert_eq!(after_promote.autonomy, AutonomyLevel::Full);
        assert!(after_promote
            .allowed_actions
            .iter()
            .any(|a| a.to_string() == "deploy"));

        // Demote overlay: restrict to readonly, no actions
        let demote_overlay = make_overlay(Some(AutonomyLevel::Readonly), Some(vec![]), None);
        let after_demote = apply_overlay(&base, &demote_overlay);
        assert_eq!(after_demote.autonomy, AutonomyLevel::Readonly);
        assert!(after_demote.allowed_actions.is_empty());

        // No overlay (cleared): back to base
        assert_eq!(base.autonomy, AutonomyLevel::Supervised);
    }

    #[test]
    fn overlay_deny_additive() {
        // Overlay can add new deny entries but cannot remove existing ones.
        let persona = make_authority(
            AutonomyLevel::Full,
            vec!["read_file", "deploy"],
            vec!["delete_production_data"],
        );
        let base = resolve_authority(&[&persona]);
        assert_eq!(base.denied_actions.len(), 1);

        let overlay = make_overlay(None, None, Some(vec!["deploy"]));
        let effective = apply_overlay(&base, &overlay);
        // Both the original deny and the overlay deny should be present
        let denied_names: Vec<String> = effective
            .denied_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(denied_names.contains(&"delete_production_data".to_string()));
        assert!(denied_names.contains(&"deploy".to_string()));
        // deploy should be removed from allowed
        let allowed_names: Vec<String> = effective
            .allowed_actions
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert!(!allowed_names.contains(&"deploy".to_string()));
    }

    #[test]
    fn deny_metadata_preserved() {
        let a = Authority {
            autonomy: AutonomyLevel::Full,
            scope: None,
            actions: Some(Actions {
                allow: Some(vec!["read_file".parse().unwrap()]),
                deny: Some(vec![DenyEntry::WithReason {
                    action: "delete_production_data".parse().unwrap(),
                    reason: "Retention policy".into(),
                    compliance_ref: Some("ISO 9001:2015 §7.5".into()),
                }]),
                scoped: None,
            }),
            limits: None,
            elevations: None,
            delegation: None,
            ext: None,
        };
        let resolved = resolve_authority(&[&a]);
        assert_eq!(resolved.denied_actions.len(), 1);
        let meta = resolved
            .deny_metadata
            .get("delete_production_data")
            .expect("deny_metadata should contain entry");
        assert_eq!(meta.reason.as_deref(), Some("Retention policy"));
        assert_eq!(meta.compliance_ref.as_deref(), Some("ISO 9001:2015 §7.5"));
    }
}
