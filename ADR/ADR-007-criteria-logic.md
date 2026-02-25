# ADR-007: CriteriaLogic (All/Any)

**Status:** Proposed
**Date:** 2026-02-22

## Context

Current `Gate.criteria: Vec<Criterion>` is evaluated as implicit AND — all criteria must pass for the gate to fire. OR logic is not expressible. This means the Guard function in the hybrid automaton is not compositional.

Reference: `THEORY.md` Gap 2.

Real use cases requiring OR:
- Demote if `error_rate > 0.1` OR `response_time_p99 > 5000`
- Promote if `tasks_completed >= 10` OR `human_approved == true`

Without OR, users must create multiple gates with identical effects, violating DRY and complicating maintenance.

## Decision

Add `CriteriaLogic` enum with two variants only:

```rust
pub enum CriteriaLogic {
    All(Vec<Criterion>),   // AND — all criteria must pass
    Any(Vec<Criterion>),   // OR — at least one criterion must pass
}
```

**Design constraints:**

1. **No `Not` variant** — no concrete gate requires it. Negation is expressible via opposite operators (`gt` vs `lte`).
2. **No recursive nesting** — `CriteriaLogic` cannot contain `CriteriaLogic`. Complexity ceiling.
3. **Backward compatible** — existing `criteria: [...]` deserialized as `CriteriaLogic::All([...])`.
4. **Default is `All`** — matches current behavior exactly.

**JSON representation:**

```json
// Existing format (backward compat):
"criteria": [{ "metric": "x", "op": "gte", "value": 10 }]

// New explicit format:
"criteria_logic": {
  "all": [{ "metric": "x", "op": "gte", "value": 10 }]
}

// OR example:
"criteria_logic": {
  "any": [
    { "metric": "error_rate", "op": "gt", "value": 0.1 },
    { "metric": "timeout_count", "op": "gt", "value": 5 }
  ]
}
```

## Consequences

### Schema changes

- Add `criteria_logic: CriteriaLogic` field to Gate
- Mark `criteria` as deprecated (warn for 2 minor versions per versioning policy)
- Both fields cannot coexist; validation error if both present
- `serde(default)` ensures backward compatibility

### Migration

`amp migrate` rewrites:
```json
"criteria": [...]  →  "criteria_logic": { "all": [...] }
```

Migration is lossless — existing AND behavior preserved exactly.

### Evaluator changes

```rust
fn evaluate_criteria_logic(logic: &CriteriaLogic, provider: &impl MetricsProvider) -> bool {
    match logic {
        CriteriaLogic::All(criteria) => criteria.iter().all(|c| evaluate_criterion(c, provider)),
        CriteriaLogic::Any(criteria) => criteria.iter().any(|c| evaluate_criterion(c, provider)),
    }
}
```

### Audit impact

`CriteriaResult` in `GateTransitionEvent` already captures per-criterion pass/fail. No structural change needed — results array shows which criteria passed under OR logic.

## Non-Goals

- **`Not` variant** — no concrete use case. Reopened only with real gate requirement.
- **Recursive nesting** — `All([Any([...]), ...])` not supported. If needed later, requires new ADR.
- **Arbitrary boolean expressions** — not a SAT solver. Two variants is the ceiling.

## Risks

| Risk | Mitigation |
|------|------------|
| Migration silently drops criteria | Test migration on all `examples/*.json`; verify round-trip |
| Schema break in `--strict` mode | `serde(default)` on `criteria_logic`; both old and new formats accepted |
| `Not` requested mid-implementation | Decline. Document as explicit non-goal. Reopen only with concrete use case. |
| Recursive nesting added "just in case" | Revert. Flat `All`/`Any` is the ceiling until proven insufficient. |
