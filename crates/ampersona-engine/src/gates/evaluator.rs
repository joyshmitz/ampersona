use std::collections::HashMap;

use chrono::Utc;
use sha2::{Digest, Sha256};

use ampersona_core::spec::gates::{Criterion, Gate};
use ampersona_core::state::PhaseState;
use ampersona_core::traits::{CriteriaResult, MetricQuery, MetricsProvider};
use ampersona_core::types::{CriterionOp, GateApproval, GateDirection, GateEnforcement};

use super::decision::GateDecisionRecord;

/// Deterministic gate evaluator.
///
/// Algorithm:
/// 1. Collect candidate gates whose from_phase matches current phase
/// 2. Sort by (direction: demote > promote, priority DESC, id ASC)
/// 3. Check cooldown/hysteresis — skip if last transition was too recent
/// 4. Evaluate criteria for first passing gate
/// 5. Check idempotency — skip if same (gate_id, metrics_hash, state_rev)
/// 6. One transition per evaluation tick
pub struct DefaultGateEvaluator;

impl DefaultGateEvaluator {
    pub fn evaluate(
        &self,
        gates: &[Gate],
        state: &PhaseState,
        metrics: &dyn MetricsProvider,
    ) -> Option<GateDecisionRecord> {
        let now = Utc::now();

        // Collect candidates matching current phase
        let mut candidates: Vec<&Gate> = gates
            .iter()
            .filter(|g| g.from_phase.as_deref() == state.current_phase.as_deref())
            .collect();

        // Sort: demote > promote, then priority DESC, then id ASC
        candidates.sort_by(|a, b| {
            let dir_ord = |d: &GateDirection| match d {
                GateDirection::Demote => 0,
                GateDirection::Promote => 1,
            };
            dir_ord(&a.direction)
                .cmp(&dir_ord(&b.direction))
                .then_with(|| b.priority.cmp(&a.priority))
                .then_with(|| a.id.cmp(&b.id))
        });

        // Evaluate each candidate
        for gate in candidates {
            // Check cooldown — skip if last transition from same gate is too recent
            if gate.cooldown_seconds > 0 {
                if let Some(last) = &state.last_transition {
                    if last.gate_id == gate.id {
                        let elapsed = (now - last.at).num_seconds();
                        if elapsed < gate.cooldown_seconds as i64 {
                            continue;
                        }
                    }
                }
            }

            let (all_pass, results, snapshot) = self.evaluate_criteria(&gate.criteria, metrics);

            if all_pass {
                // Compute metrics hash for idempotency
                let metrics_hash = compute_metrics_hash(&snapshot);

                // Idempotency check: skip if same (gate_id, metrics_hash, state_rev)
                if let Some(last) = &state.last_transition {
                    if last.gate_id == gate.id
                        && last.metrics_hash.as_deref() == Some(&metrics_hash)
                        && state.state_rev == last.metrics_hash.as_ref().map(|_| state.state_rev).unwrap_or(0)
                    {
                        // Same gate with same metrics already transitioned at this state_rev
                        continue;
                    }
                }

                // Handle approval type
                let decision = match gate.approval {
                    GateApproval::Human => "pending_human".to_string(),
                    GateApproval::Quorum => {
                        return Some(GateDecisionRecord {
                            gate_id: gate.id.clone(),
                            direction: gate.direction,
                            enforcement: gate.enforcement,
                            decision: "error_quorum_not_supported".to_string(),
                            from_phase: state.current_phase.clone(),
                            to_phase: gate.to_phase.clone(),
                            metrics_snapshot: snapshot,
                            criteria_results: results,
                            is_override: false,
                            state_rev: state.state_rev,
                            metrics_hash,
                        });
                    }
                    GateApproval::Auto => {
                        if gate.enforcement == GateEnforcement::Observe {
                            "observed".to_string()
                        } else {
                            "transition".to_string()
                        }
                    }
                };

                return Some(GateDecisionRecord {
                    gate_id: gate.id.clone(),
                    direction: gate.direction,
                    enforcement: gate.enforcement,
                    decision,
                    from_phase: state.current_phase.clone(),
                    to_phase: gate.to_phase.clone(),
                    metrics_snapshot: snapshot,
                    criteria_results: results,
                    is_override: false,
                    state_rev: state.state_rev,
                    metrics_hash,
                });
            }
        }

        None
    }

    pub fn evaluate_criteria(
        &self,
        criteria: &[Criterion],
        metrics: &dyn MetricsProvider,
    ) -> (
        bool,
        Vec<CriteriaResult>,
        HashMap<String, serde_json::Value>,
    ) {
        let mut all_pass = true;
        let mut results = Vec::new();
        let mut snapshot = HashMap::new();

        for criterion in criteria {
            let query = MetricQuery {
                name: criterion.metric.clone(),
                window: None,
            };

            let (actual, pass) = match metrics.get_metric(&query) {
                Ok(sample) => {
                    snapshot.insert(criterion.metric.clone(), sample.value.clone());
                    let pass = compare_values(&criterion.op, &sample.value, &criterion.value);
                    (sample.value, pass)
                }
                Err(_) => {
                    all_pass = false;
                    (serde_json::Value::Null, false)
                }
            };

            if !pass {
                all_pass = false;
            }

            results.push(CriteriaResult {
                metric: criterion.metric.clone(),
                op: criterion.op,
                value: criterion.value.clone(),
                actual,
                pass,
            });
        }

        (all_pass, results, snapshot)
    }
}

/// Compute a deterministic hash of metrics snapshot for idempotency checks.
fn compute_metrics_hash(snapshot: &HashMap<String, serde_json::Value>) -> String {
    let mut keys: Vec<&String> = snapshot.keys().collect();
    keys.sort();
    let canonical: Vec<String> = keys
        .iter()
        .map(|k| format!("{}:{}", k, snapshot[*k]))
        .collect();
    let joined = canonical.join(",");
    format!("sha256:{:x}", Sha256::digest(joined.as_bytes()))
}

fn compare_values(
    op: &CriterionOp,
    actual: &serde_json::Value,
    expected: &serde_json::Value,
) -> bool {
    match op {
        CriterionOp::Eq => actual == expected,
        CriterionOp::Neq => actual != expected,
        CriterionOp::Gt => cmp_num(actual, expected).is_some_and(|c| c > 0),
        CriterionOp::Gte => cmp_num(actual, expected).is_some_and(|c| c >= 0),
        CriterionOp::Lt => cmp_num(actual, expected).is_some_and(|c| c < 0),
        CriterionOp::Lte => cmp_num(actual, expected).is_some_and(|c| c <= 0),
    }
}

fn cmp_num(a: &serde_json::Value, b: &serde_json::Value) -> Option<i8> {
    let a_f = a.as_f64()?;
    let b_f = b.as_f64()?;
    if a_f > b_f {
        Some(1)
    } else if a_f < b_f {
        Some(-1)
    } else {
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ampersona_core::errors::MetricError;
    use ampersona_core::spec::gates::Gate;
    use ampersona_core::state::{PhaseState, TransitionRecord};
    use ampersona_core::traits::MetricSample;
    use chrono::{Duration, Utc};

    struct TestMetrics(HashMap<String, serde_json::Value>);

    impl MetricsProvider for TestMetrics {
        fn get_metric(&self, query: &MetricQuery) -> Result<MetricSample, MetricError> {
            self.0
                .get(&query.name)
                .map(|v| MetricSample {
                    name: query.name.clone(),
                    value: v.clone(),
                    sampled_at: Utc::now(),
                })
                .ok_or(MetricError::NotFound(query.name.clone()))
        }
    }

    fn make_gate(
        id: &str,
        direction: GateDirection,
        from: &str,
        to: &str,
        criteria: Vec<Criterion>,
    ) -> Gate {
        Gate {
            id: id.into(),
            direction,
            enforcement: GateEnforcement::Enforce,
            priority: 10,
            cooldown_seconds: 0,
            from_phase: Some(from.into()),
            to_phase: to.into(),
            criteria,
            metrics_schema: None,
            approval: ampersona_core::types::GateApproval::Auto,
            on_pass: None,
        }
    }

    #[test]
    fn demote_wins_over_promote() {
        let gates = vec![
            make_gate(
                "promote_gate",
                GateDirection::Promote,
                "active",
                "trusted",
                vec![Criterion {
                    metric: "score".into(),
                    op: CriterionOp::Gte,
                    value: serde_json::json!(5),
                }],
            ),
            make_gate(
                "demote_gate",
                GateDirection::Demote,
                "active",
                "probation",
                vec![Criterion {
                    metric: "violations".into(),
                    op: CriterionOp::Gte,
                    value: serde_json::json!(3),
                }],
            ),
        ];

        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: None,
            pending_transition: None,
            updated_at: Utc::now(),
        };

        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(10));
        metrics_map.insert("violations".into(), serde_json::json!(5));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&gates, &state, &metrics);

        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.gate_id, "demote_gate");
        assert_eq!(record.direction, GateDirection::Demote);
    }

    #[test]
    fn cooldown_prevents_reevaluation() {
        let gates = vec![make_gate(
            "trust_decay",
            GateDirection::Demote,
            "trusted",
            "active",
            vec![Criterion {
                metric: "violations".into(),
                op: CriterionOp::Gte,
                value: serde_json::json!(3),
            }],
        )];
        // Set cooldown
        let mut gates = gates;
        gates[0].cooldown_seconds = 86400; // 24h

        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("trusted".into()),
            state_rev: 2,
            active_elevations: vec![],
            last_transition: Some(TransitionRecord {
                gate_id: "trust_decay".into(),
                from_phase: Some("active".into()),
                to_phase: "trusted".into(),
                at: Utc::now() - Duration::hours(1), // only 1h ago
                decision_id: "gate-1".into(),
                metrics_hash: None,
            }),
            pending_transition: None,
            updated_at: Utc::now(),
        };

        let mut metrics_map = HashMap::new();
        metrics_map.insert("violations".into(), serde_json::json!(5));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&gates, &state, &metrics);

        // Cooldown not expired → gate should not fire
        assert!(result.is_none());
    }

    #[test]
    fn observe_mode_does_not_block() {
        let mut gate = make_gate(
            "observe_gate",
            GateDirection::Promote,
            "active",
            "trusted",
            vec![Criterion {
                metric: "score".into(),
                op: CriterionOp::Gte,
                value: serde_json::json!(10),
            }],
        );
        gate.enforcement = GateEnforcement::Observe;

        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: None,
            pending_transition: None,
            updated_at: Utc::now(),
        };

        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(15));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&[gate], &state, &metrics);

        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.decision, "observed");
        assert_eq!(record.enforcement, GateEnforcement::Observe);
    }

    #[test]
    fn metrics_hash_is_deterministic() {
        let mut s1 = HashMap::new();
        s1.insert("a".to_string(), serde_json::json!(1));
        s1.insert("b".to_string(), serde_json::json!(2));

        let mut s2 = HashMap::new();
        s2.insert("b".to_string(), serde_json::json!(2));
        s2.insert("a".to_string(), serde_json::json!(1));

        assert_eq!(compute_metrics_hash(&s1), compute_metrics_hash(&s2));
    }

    #[test]
    fn trust_decay_auto_demotes() {
        // Simulate trust decay: agent in "trusted" phase, violations accumulate,
        // demote gate fires automatically to bring back to "active"
        let gates = vec![
            make_gate(
                "promote_to_trusted",
                GateDirection::Promote,
                "active",
                "trusted",
                vec![Criterion {
                    metric: "tasks_completed".into(),
                    op: CriterionOp::Gte,
                    value: serde_json::json!(20),
                }],
            ),
            {
                let mut g = make_gate(
                    "trust_decay",
                    GateDirection::Demote,
                    "trusted",
                    "active",
                    vec![Criterion {
                        metric: "policy_violations_30d".into(),
                        op: CriterionOp::Gte,
                        value: serde_json::json!(3),
                    }],
                );
                g.priority = 20;
                g.cooldown_seconds = 86400;
                g
            },
        ];

        // Agent is in trusted phase
        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("trusted".into()),
            state_rev: 5,
            active_elevations: vec![],
            last_transition: Some(TransitionRecord {
                gate_id: "promote_to_trusted".into(),
                from_phase: Some("active".into()),
                to_phase: "trusted".into(),
                at: Utc::now() - Duration::days(30), // promoted 30 days ago
                decision_id: "gate-4".into(),
                metrics_hash: None,
            }),
            pending_transition: None,
            updated_at: Utc::now(),
        };

        // Agent has accumulated violations
        let mut metrics_map = HashMap::new();
        metrics_map.insert("policy_violations_30d".into(), serde_json::json!(5));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&gates, &state, &metrics);

        assert!(result.is_some());
        let record = result.unwrap();
        assert_eq!(record.gate_id, "trust_decay");
        assert_eq!(record.direction, GateDirection::Demote);
        assert_eq!(record.to_phase, "active");
        assert!(!record.is_override);

        // Verify the demotion decision
        assert_eq!(record.decision, "transition");
        assert_eq!(record.from_phase, Some("trusted".into()));
    }

    #[test]
    fn no_gate_fires_when_criteria_fail() {
        let gates = vec![make_gate(
            "promote",
            GateDirection::Promote,
            "active",
            "trusted",
            vec![Criterion {
                metric: "score".into(),
                op: CriterionOp::Gte,
                value: serde_json::json!(100),
            }],
        )];

        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: None,
            pending_transition: None,
            updated_at: Utc::now(),
        };

        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(50));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&gates, &state, &metrics);
        assert!(result.is_none());
    }
}
