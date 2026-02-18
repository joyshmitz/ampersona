# ADR-002: Deny-by-Default Policy Engine

**Status:** Accepted
**Date:** 2026-02-18

## Context

Permission systems can be allow-by-default (everything permitted unless denied) or deny-by-default (everything denied unless allowed). For autonomous AI agents, fail-open is dangerous — an unrecognized action silently succeeding could cause harm.

## Decision

The ampersona policy engine is **deny-by-default**:

1. **Unknown actions** are denied. If an action is not in the vocabulary (builtin or `custom:<vendor>/<action>`), the policy engine returns `Deny`.
2. **Explicit deny always wins** — highest precedence in authority layering.
3. **Unknown fields** are rejected in `--strict` mode (`amp check --strict`).
4. **Authority precedence** (highest → lowest): explicit deny → active elevation → gate overlay → persona authority → workspace defaults.
5. **Merge rules** favor restriction: deny = union, allow = intersection minus deny, limits = minimum, autonomy = minimum.

## Consequences

- Agents cannot accidentally gain permissions they weren't explicitly granted
- New action types require explicit vocabulary registration (builtin enum or `custom:` namespace)
- `amp check --strict` becomes the CI gate standard
- Slight increase in initial setup friction (must explicitly allow actions)
- Clear audit trail: every denial has a traceable reason
