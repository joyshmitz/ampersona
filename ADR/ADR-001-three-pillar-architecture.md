# ADR-001: Three-Pillar Architecture (Identity + Authority + Gates)

**Status:** Accepted
**Date:** 2026-02-18

## Context

ampersona v0.2 defines agent identity only: psychology, voice, capabilities, directives. There is no concept of what an agent is *allowed* to do or how trust evolves over time.

Consumers like zeroclaw need scoped permissions. Consumers like odoov19 need phase-gated trust progression. Defining these outside the persona creates a split-brain problem where identity and authority diverge.

## Decision

Extend the persona spec from one dimension (identity) to three pillars:

1. **Identity** — Who the agent is (psychology, voice, capabilities, directives). Carried forward from v0.2.
2. **Authority** — What the agent may do (autonomy level, scoped actions, limits, elevations, delegation, deny-by-default).
3. **Gates** — When trust changes (promote/demote criteria, metrics-based evaluation, enforcement modes, cooldown/hysteresis).

All three pillars live in a single persona JSON file. Authority and gates are optional — a v0.2-style identity-only file remains valid.

## Consequences

- Schema grows from ~240 lines to ~800+ lines
- `amp validate` must auto-detect v0.2 vs v1.0
- `amp migrate` needed for v0.2 → v1.0 conversion
- Consumers can adopt pillars incrementally (identity only → + authority → + gates)
- Single source of truth: no authority config outside the persona
