# ADR-003: Deterministic Gate Evaluation

**Status:** Accepted
**Date:** 2026-02-18

## Context

Gates evaluate metrics to decide trust transitions (promote/demote). With multiple gates potentially active, the evaluation order must be deterministic and safe. Non-deterministic evaluation could cause flapping (rapid promote/demote cycles) or inconsistent state across runs.

## Decision

Gate evaluation follows a deterministic algorithm:

1. **Demote wins over promote** — safety-first. If both a promote and demote gate fire in the same evaluation, only the demote applies.
2. **Candidate gates sorted by** `(direction: demote > promote, priority DESC, id ASC)`.
3. **One transition per evaluation tick** — the first passing gate in sorted order wins.
4. **Cooldown/hysteresis** prevents flapping: a gate cannot re-fire within `cooldown_seconds` of its last transition.
5. **Idempotent evaluation** per `(gate_id, metrics_hash, state_rev)` — same inputs always produce same output.
6. **Enforcement modes**: `enforce` applies the transition; `observe` logs but does not apply (safe rollout).
7. **Human gates** create a pending transition with `decision_id`; `auto` gates apply immediately.

## Consequences

- Trust state is reproducible: given the same metrics and state, evaluation always produces the same result
- Demote-first prevents a compromised metrics source from maintaining elevated trust
- Cooldown prevents oscillation when metrics hover near thresholds
- `observe` mode enables gradual rollout of new gates without risk
- One-transition-per-tick simplifies reasoning about state changes
- Gate decision records capture full metrics snapshot for audit
