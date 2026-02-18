# ADR-004: Workspace Layout — Four Crates

**Status:** Accepted
**Date:** 2026-02-18

## Context

v0.2 is a single `ampersona` package with `src/` containing all code. v1.0 adds authority engine, gate evaluator, signing, and state management. Keeping everything in one crate would:

- Force consumers (zeroclaw, agent_mail_rust) to depend on the full engine when they only need types/traits
- Make the binary bloated for users who only need validation
- Prevent feature-gating of optional integrations (AIEOS import, zeroclaw export)

## Decision

Split into a Cargo workspace with four crates:

| Crate | Purpose | Consumers |
|-------|---------|-----------|
| `ampersona-core` | Types, traits, schema, migration, serde | Everyone (publishable) |
| `ampersona-engine` | Policy checker, gate evaluator, state, precedence | `amp` CLI |
| `ampersona-sign` | JCS canonicalization, ed25519 sign/verify | `amp` CLI, optional for engine |
| `amp` | Binary crate (CLI) | End users |

**Dependency graph:**
```
amp → ampersona-core + ampersona-engine + ampersona-sign
ampersona-engine → ampersona-core
ampersona-engine →(optional) ampersona-sign (for state signing)
ampersona-engine →(feature) "aieos" (import/export)
ampersona-engine →(feature) "zeroclaw" (import/export)
ampersona-sign → ampersona-core (type defs only)
```

**Consumer dependencies:**
- `mcp_agent_mail_rust` → `ampersona-core` (types + AuditSink trait)
- `zeroclaw` → `ampersona-core` (types + AuthorityEnforcer + MetricsProvider)

## Consequences

- `ampersona-core` is a lightweight, publishable crate with no engine logic
- Consumers only pull in the types and traits they need
- Feature gates isolate optional integrations (no bloat for unrelated consumers)
- Workspace builds all crates together; CI tests the full dependency graph
- Existing v0.2 code in `src/` must be migrated to the new crate structure
