# ampersona — Threat Model

## Assets

| Asset | Sensitivity | Location |
|-------|-----------|----------|
| Persona spec (identity + authority + gates) | High — defines agent permissions | `persona.json` |
| Signing keys (ed25519 private) | Critical — controls persona integrity | External (not in repo) |
| State files (phase, elevations) | High — controls current trust level | `<name>.state.json` |
| Audit log | Medium — tamper-evident record | `<name>.audit.jsonl` |
| Drift ledger | Low-Medium — metrics history | `<name>.drift.jsonl` |
| Workspace defaults | Medium — baseline authority | `.ampersona/defaults.json` |

## Trust Boundaries

```
┌─────────────────────────────────┐
│  Human Operator                  │
│  (creates personas, holds keys)  │
├──────────────┬──────────────────┤
│              │ amp CLI           │
│              │ (validates,       │
│              │  evaluates,       │
│              │  signs)           │
├──────────────┼──────────────────┤
│  Agent       │  Consumer Runtime │
│  (reads      │  (enforces        │
│   persona)   │   authority)      │
├──────────────┴──────────────────┤
│  External Systems                │
│  (file system, APIs, repos)      │
└─────────────────────────────────┘
```

**Key boundaries:**
1. **Operator → CLI**: Persona files are trusted input (operator authored)
2. **CLI → Consumer**: Persona spec + state are the contract
3. **Consumer → External**: Authority enforcement happens here (the actual "do or deny")
4. **Agent → State**: Agents should not directly modify state files

## Abuse Cases & Mitigations

### A1: Persona Tampering

**Threat:** Attacker modifies persona JSON to grant elevated permissions.

**Mitigations:**
- Ed25519 signatures on persona files (`amp sign` / `amp verify`)
- JCS canonicalization prevents byte-level manipulation
- `signed_fields` must cover all authority-related fields
- `amp check` verifies signature integrity

### A2: State File Manipulation

**Threat:** Attacker modifies `<name>.state.json` to set phase=trusted or inject fake elevations.

**Mitigations:**
- State files are signed (same key as persona)
- `state_rev` monotonic counter detects rollback
- Advisory lock prevents concurrent writes
- Atomic write (temp+fsync+rename) prevents partial state
- Hash-chain audit log detects state changes not recorded in audit

### A3: Audit Log Tampering

**Threat:** Attacker removes or modifies audit entries to hide policy violations.

**Mitigations:**
- Hash-chain: each entry includes `prev_hash` (SHA-256 of previous)
- Signed checkpoints at configurable intervals
- `amp audit --verify` validates chain integrity
- Append-only: no update/delete operations exposed

### A4: Elevation Abuse

**Threat:** Agent obtains elevation and uses it beyond intended scope.

**Mitigations:**
- TTL enforcement: elevations expire automatically
- `reason_required`: elevations must have documented justification
- Sliding window rate limiter: bounded activations per time window
- Audit log records all elevation changes with timestamps

### A5: Gate Bypass

**Threat:** Agent manipulates metrics to pass a promote gate.

**Mitigations:**
- `MetricsProvider` trait: metrics come from consumer runtime, not agent
- Gate decision records capture full metrics snapshot
- Demote gates have priority over promote gates (safety-first)
- Override mechanism requires human approver with delegation level
- `is_override: true` flag clearly marks bypasses in audit

### A6: Action Vocabulary Injection

**Threat:** Agent uses crafted action name to bypass deny rules.

**Mitigations:**
- Deny-by-default: unrecognized actions are denied
- Builtin action enum: known actions are a closed set
- `custom:<vendor>/<action>` namespace: extensions require explicit prefix
- `amp check --strict` rejects unknown actions

### A7: Path Traversal via Scope

**Threat:** Agent uses `../` or symlinks to escape `allowed_paths`.

**Mitigations:**
- `ScopedFileAccess` type with explicit read/write/deny_write paths
- `validate_symlinks: true` in `ScopedShell` follows and checks targets
- Consumer runtime (not ampersona) does actual path enforcement
- `amp check` validates path patterns for obvious traversal attempts

### A8: Denial of Service via Fuzz Input

**Threat:** Malformed JSON causes panic or excessive resource use.

**Mitigations:**
- Fuzz tests on parse/migrate/import paths
- `jsonschema` validation before processing
- `#![forbid(unsafe_code)]` — no undefined behavior
- Size limits on input files (consumer-configurable)

## Out of Scope

- Key management (key generation, distribution, revocation) — external to ampersona
- Network security (TLS, authentication) — consumer responsibility
- Sandbox enforcement (Landlock, seccomp) — consumer runtime (zeroclaw)
- Secrets in persona files — explicitly excluded by design
