# ADR-005: Hash-Chain Audit Log

**Status:** Accepted
**Date:** 2026-02-18

## Context

AI agent actions need auditable history. Simple append-only logs can be silently truncated or tampered with. Compliance requirements (ISO 9001, 21 CFR Part 11) demand tamper-evident records.

## Decision

Audit and drift logs use a **hash-chain** structure:

1. Each entry includes `prev_hash` — SHA-256 of the previous entry's canonical JSON.
2. The first entry in a chain uses `prev_hash: "genesis"`.
3. `amp audit --verify` walks the chain and validates every hash link.
4. **Signed checkpoints** at configurable intervals anchor the chain with ed25519 signatures.
5. **Integrity snapshots** (`<name>.integrity.json`) are signed checkpoints that can be verified independently.
6. **Append-only** — entries are never modified or deleted. Corrections are new entries referencing the original.

**File layout:**
- `<name>.audit.jsonl` — policy decisions, gate transitions, elevation changes, overrides
- `<name>.drift.jsonl` — metrics snapshots over time (for trend analysis)
- `<name>.integrity.json` — latest signed checkpoint

**Audit event taxonomy** (`AuditEventType`):
- `PolicyDecision` — action allowed/denied/needs-approval
- `GateTransition` — phase promote/demote (includes gate decision record)
- `ElevationChange` — temporary grant activated/expired
- `Override` — emergency gate bypass
- `SignatureVerify` — persona/state signature verification result
- `StateChange` — any state file modification

## Consequences

- Tamper-evident: any modification to historical entries breaks the chain
- Verifiable: `amp audit --verify` can run in CI or on-demand
- Compliance-friendly: retention_days, compliance_markers in audit config
- Storage grows monotonically (append-only); rotation/archival is out of scope for v1.0
- Checkpoint signing requires key availability; unsigned chains are valid but unanchored
