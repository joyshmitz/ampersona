use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};

use ampersona_core::spec::gates::{Criterion, Gate, MetricSchema};
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

            let (all_pass, results, snapshot) = self.evaluate_criteria(
                &gate.criteria,
                metrics,
                gate.direction,
                gate.metrics_schema.as_ref(),
            );

            if all_pass {
                // Compute metrics hash for idempotency
                let metrics_hash = compute_metrics_hash(&snapshot);

                // Idempotency check: skip if same (gate_id, metrics_hash, state_rev)
                if let Some(last) = &state.last_transition {
                    if last.gate_id == gate.id
                        && last.metrics_hash.as_deref() == Some(&metrics_hash)
                        && last.state_rev == state.state_rev
                    {
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
        direction: GateDirection,
        metrics_schema: Option<&HashMap<String, MetricSchema>>,
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
                window: criterion.window_seconds.map(Duration::from_secs),
            };

            let (actual, pass, type_mismatch) = match metrics.get_metric(&query) {
                Ok(sample) => {
                    snapshot.insert(criterion.metric.clone(), sample.value.clone());

                    // Type validation: check metric value matches declared schema type
                    if let Some(mismatch) =
                        check_metric_type(&criterion.metric, &sample.value, metrics_schema)
                    {
                        // Fail-closed: demote fires, promote blocked
                        let pass = direction == GateDirection::Demote;
                        (sample.value, pass, Some(mismatch))
                    } else {
                        let pass = compare_values(&criterion.op, &sample.value, &criterion.value);
                        (sample.value, pass, None)
                    }
                }
                Err(_) => {
                    all_pass = false;
                    (serde_json::Value::Null, false, None)
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
                type_mismatch,
            });
        }

        (all_pass, results, snapshot)
    }
}

/// Check if a metric value matches the declared type in metrics_schema.
/// Returns Some(mismatch_description) if there's a type mismatch, None if ok or no schema.
fn check_metric_type(
    metric_name: &str,
    value: &serde_json::Value,
    schema: Option<&HashMap<String, MetricSchema>>,
) -> Option<String> {
    let schema = schema?;
    let metric_schema = schema.get(metric_name)?;
    let expected_type = metric_schema.metric_type.as_str();

    let matches = match expected_type {
        "number" | "numeric" | "float" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" | "bool" => value.is_boolean(),
        "string" => value.is_string(),
        _ => true, // unknown schema type → no check
    };

    if matches {
        None
    } else {
        Some(format!(
            "metric '{}' expected type '{}', got {}",
            metric_name,
            expected_type,
            value_type_name(value)
        ))
    }
}

fn value_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
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
                    window_seconds: None,
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
                    window_seconds: None,
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
            active_overlay: None,
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
                window_seconds: None,
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
                state_rev: 0,
            }),
            pending_transition: None,
            active_overlay: None,
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
                window_seconds: None,
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
            active_overlay: None,
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
                    window_seconds: None,
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
                        metric: "policy_violations".into(),
                        op: CriterionOp::Gte,
                        window_seconds: None,
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
                state_rev: 0,
            }),
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        };

        // Agent has accumulated violations
        let mut metrics_map = HashMap::new();
        metrics_map.insert("policy_violations".into(), serde_json::json!(5));
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
                window_seconds: None,
            }],
        )];

        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: None,
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        };

        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(50));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;
        let result = evaluator.evaluate(&gates, &state, &metrics);
        assert!(result.is_none());
    }

    // ── Metrics type validation tests ─────────────────────────────

    #[test]
    fn type_mismatch_demote_fires() {
        // String "hot" for numeric metric, demote gate → criterion passes (fail-closed: demote fires)
        let evaluator = DefaultGateEvaluator;
        let criteria = vec![Criterion {
            metric: "temperature".into(),
            op: CriterionOp::Gte,
            window_seconds: None,
            value: serde_json::json!(100),
        }];
        let mut schema = HashMap::new();
        schema.insert(
            "temperature".to_string(),
            ampersona_core::spec::gates::MetricSchema {
                metric_type: "number".to_string(),
            },
        );
        let mut metrics_map = HashMap::new();
        metrics_map.insert("temperature".into(), serde_json::json!("hot")); // string, not number
        let metrics = TestMetrics(metrics_map);

        let (all_pass, results, _) =
            evaluator.evaluate_criteria(&criteria, &metrics, GateDirection::Demote, Some(&schema));
        assert!(
            all_pass,
            "demote with type mismatch should pass (fail-closed)"
        );
        assert!(results[0].type_mismatch.is_some());
    }

    #[test]
    fn type_mismatch_promote_blocked() {
        // String "hot" for numeric metric, promote gate → criterion fails (fail-closed: promote blocked)
        let evaluator = DefaultGateEvaluator;
        let criteria = vec![Criterion {
            metric: "temperature".into(),
            op: CriterionOp::Gte,
            window_seconds: None,
            value: serde_json::json!(100),
        }];
        let mut schema = HashMap::new();
        schema.insert(
            "temperature".to_string(),
            ampersona_core::spec::gates::MetricSchema {
                metric_type: "number".to_string(),
            },
        );
        let mut metrics_map = HashMap::new();
        metrics_map.insert("temperature".into(), serde_json::json!("hot"));
        let metrics = TestMetrics(metrics_map);

        let (all_pass, results, _) =
            evaluator.evaluate_criteria(&criteria, &metrics, GateDirection::Promote, Some(&schema));
        assert!(
            !all_pass,
            "promote with type mismatch should fail (fail-closed)"
        );
        assert!(results[0].type_mismatch.is_some());
    }

    #[test]
    fn type_match_normal_behavior() {
        // Correct types → no change in behavior
        let evaluator = DefaultGateEvaluator;
        let criteria = vec![Criterion {
            metric: "score".into(),
            op: CriterionOp::Gte,
            window_seconds: None,
            value: serde_json::json!(10),
        }];
        let mut schema = HashMap::new();
        schema.insert(
            "score".to_string(),
            ampersona_core::spec::gates::MetricSchema {
                metric_type: "number".to_string(),
            },
        );
        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(15));
        let metrics = TestMetrics(metrics_map);

        let (all_pass, results, _) =
            evaluator.evaluate_criteria(&criteria, &metrics, GateDirection::Promote, Some(&schema));
        assert!(all_pass, "correct types should pass normally");
        assert!(results[0].type_mismatch.is_none());
    }

    #[test]
    fn window_seconds_passed_to_metric_query() {
        // Verify that criterion.window_seconds propagates to MetricQuery.window
        use std::time::Duration as StdDuration;

        struct WindowCapture(std::sync::Mutex<Option<Option<StdDuration>>>);
        impl MetricsProvider for WindowCapture {
            fn get_metric(&self, query: &MetricQuery) -> Result<MetricSample, MetricError> {
                *self.0.lock().unwrap() = Some(query.window);
                Ok(MetricSample {
                    name: query.name.clone(),
                    value: serde_json::json!(0.95),
                    sampled_at: Utc::now(),
                })
            }
        }

        let evaluator = DefaultGateEvaluator;

        // With window_seconds = Some(604800) (7 days)
        let criteria = vec![Criterion {
            metric: "sop.completion_rate".into(),
            op: CriterionOp::Gte,
            window_seconds: Some(604800),
            value: serde_json::json!(0.9),
        }];
        let capture = WindowCapture(std::sync::Mutex::new(None));
        evaluator.evaluate_criteria(&criteria, &capture, GateDirection::Promote, None);
        let captured = capture.0.lock().unwrap().unwrap();
        assert_eq!(captured, Some(StdDuration::from_secs(604800)));

        // With window_seconds = None
        let criteria_no_window = vec![Criterion {
            metric: "sop.completion_rate".into(),
            op: CriterionOp::Gte,
            window_seconds: None,
            value: serde_json::json!(0.9),
        }];
        let capture2 = WindowCapture(std::sync::Mutex::new(None));
        evaluator.evaluate_criteria(&criteria_no_window, &capture2, GateDirection::Promote, None);
        let captured2 = capture2.0.lock().unwrap().unwrap();
        assert_eq!(captured2, None);
    }

    #[test]
    fn idempotency_skips_duplicate_transition() {
        // First evaluation fires the gate
        let gates = vec![make_gate(
            "promote",
            GateDirection::Promote,
            "active",
            "trusted",
            vec![Criterion {
                metric: "score".into(),
                op: CriterionOp::Gte,
                window_seconds: None,
                value: serde_json::json!(10),
            }],
        )];

        let mut metrics_map = HashMap::new();
        metrics_map.insert("score".into(), serde_json::json!(15));
        let metrics = TestMetrics(metrics_map);

        let evaluator = DefaultGateEvaluator;

        // First eval: should fire
        let state = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: None,
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        };
        let result = evaluator.evaluate(&gates, &state, &metrics).unwrap();
        assert_eq!(result.gate_id, "promote");
        let fired_hash = result.metrics_hash.clone();

        // Second eval: same gate, same metrics_hash, same state_rev → idempotent skip
        let state2 = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 1,
            active_elevations: vec![],
            last_transition: Some(TransitionRecord {
                gate_id: "promote".into(),
                from_phase: Some("active".into()),
                to_phase: "trusted".into(),
                at: Utc::now(),
                decision_id: "gate-1".into(),
                metrics_hash: Some(fired_hash.clone()),
                state_rev: 1,
            }),
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        };
        let result2 = evaluator.evaluate(&gates, &state2, &metrics);
        assert!(
            result2.is_none(),
            "idempotent: same (gate, hash, rev) must skip"
        );

        // Third eval: same gate, same hash, but state_rev advanced → NOT idempotent
        let state3 = PhaseState {
            name: "test".into(),
            current_phase: Some("active".into()),
            state_rev: 2,
            active_elevations: vec![],
            last_transition: Some(TransitionRecord {
                gate_id: "promote".into(),
                from_phase: Some("active".into()),
                to_phase: "trusted".into(),
                at: Utc::now(),
                decision_id: "gate-1".into(),
                metrics_hash: Some(fired_hash),
                state_rev: 1,
            }),
            pending_transition: None,
            active_overlay: None,
            updated_at: Utc::now(),
        };
        let result3 = evaluator.evaluate(&gates, &state3, &metrics);
        assert!(result3.is_some(), "different state_rev must re-evaluate");
    }
}
