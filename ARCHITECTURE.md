# ampersona — Architecture

## Workspace Layout

```
ampersona/
├── Cargo.toml                        # workspace root
├── crates/
│   ├── ampersona-core/               # types, traits, schema, migration, serde
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs              # UnitFloat, AutonomyLevel, enums
│   │   │   ├── spec/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── identity.rs       # Psychology, Voice, Capabilities, Directives
│   │   │   │   ├── authority.rs      # Authority, Scope, Actions, Limits, Elevations
│   │   │   │   ├── gates.rs          # Gate, Criterion, GateEffect, GateDirection
│   │   │   │   ├── audit.rs          # AuditConfig, AuditEventType
│   │   │   │   └── signature.rs      # Signature spec type
│   │   │   ├── actions.rs            # ActionId (builtin enum + custom parsing)
│   │   │   ├── traits.rs             # AuthorityEnforcer, MetricsProvider, AuditSink
│   │   │   ├── state.rs              # PhaseState, ActiveElevation, TransitionRecord
│   │   │   ├── schema.rs             # JSON Schema validation (v0.2 + v1.0)
│   │   │   ├── migrate.rs            # v0.2 → v1.0 migration
│   │   │   ├── prompt.rs             # Markdown + TOON output
│   │   │   ├── compose.rs            # Merge + workspace defaults
│   │   │   ├── register.rs           # mcp_agent_mail bridge
│   │   │   ├── templates.rs          # Built-in archetypes
│   │   │   └── errors.rs             # PolicyError, MetricError, AuditError
│   │   └── schema/
│   │       ├── ampersona-v0.2.schema.json
│   │       └── ampersona-v1.0.schema.json
│   ├── ampersona-engine/             # policy, gates, precedence, state machine
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── policy/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── checker.rs        # DefaultPolicyChecker
│   │   │   │   ├── precedence.rs     # Authority layering
│   │   │   │   └── action_registry.rs # Builtin + custom validation
│   │   │   ├── gates/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── evaluator.rs      # DefaultGateEvaluator
│   │   │   │   ├── override_gate.rs  # Override mechanism
│   │   │   │   └── decision.rs       # GateDecisionRecord
│   │   │   ├── state/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── phase.rs          # Phase state management
│   │   │   │   ├── elevation.rs      # Elevation activation/TTL
│   │   │   │   ├── drift.rs          # DriftLedger
│   │   │   │   ├── atomic.rs         # temp+fsync+rename, advisory lock
│   │   │   │   └── audit_log.rs      # Hash-chain append + checkpoints
│   │   │   └── convert/
│   │   │       ├── mod.rs
│   │   │       ├── aieos.rs          # AIEOS import/export (feature-gated)
│   │   │       └── zeroclaw.rs       # zeroclaw import/export (feature-gated)
│   ├── ampersona-sign/               # canonicalization + ed25519
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── canonical.rs          # JCS/RFC8785 canonicalization
│   │   │   ├── sign.rs               # ed25519 signing
│   │   │   └── verify.rs             # Verification
│   └── amp/                          # binary crate (CLI)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
├── examples/                         # v0.2 + v1.0 consumer fixtures
├── schema/                           # legacy schema location (symlink)
├── tests/
│   ├── backward_compat.rs
│   ├── golden/
│   ├── property/
│   └── concurrency/
├── fuzz/
└── README.md
```

## Crate Dependencies

```
amp ───────────────> ampersona-core
    ───────────────> ampersona-engine
    ───────────────> ampersona-sign

ampersona-engine ──> ampersona-core
                 ──> ampersona-sign (optional, feature = "signing")
                 ──> (feature = "aieos")
                 ──> (feature = "zeroclaw")

ampersona-sign ───> ampersona-core (types only)

Consumer deps:
  mcp_agent_mail_rust ──> ampersona-core (types + AuditSink)
  zeroclaw            ──> ampersona-core (types + AuthorityEnforcer + MetricsProvider)
```

## Module Boundaries

### ampersona-core

**Responsibility:** Data types, serialization, validation, traits. No business logic.

| Module | Exports | Does NOT |
|--------|---------|----------|
| `types` | UnitFloat, AutonomyLevel, CriterionOp, enums | Evaluate anything |
| `spec::*` | Persona, Psychology, Voice, Authority, Gate, etc. | Enforce policies |
| `actions` | ActionId parsing, builtin registry, custom validation | Check permissions |
| `traits` | AuthorityEnforcer, MetricsProvider, AuditSink | Implement them |
| `state` | PhaseState, ActiveElevation, TransitionRecord | Persist/load state |
| `schema` | validate(), validator(), version detection | Schema evolution logic |
| `migrate` | v0.2 → v1.0 conversion | Engine migration |
| `prompt` | to_system_prompt(), to_toon() | Authority enforcement |
| `compose` | merge_authority(), merge_personas() | Precedence resolution |
| `register` | build_args(), wrap_rpc() | Agent coordination |
| `templates` | generate(), list_templates() | Runtime evaluation |
| `errors` | PolicyError, MetricError, AuditError | Handle errors |

### ampersona-engine

**Responsibility:** Policy evaluation, gate state machine, precedence resolution. Business logic.

| Module | Exports | Depends on |
|--------|---------|-----------|
| `policy::checker` | DefaultPolicyChecker (impl AuthorityEnforcer) | core::traits, core::actions |
| `policy::precedence` | resolve_authority() → ResolvedAuthority | core::spec::authority |
| `policy::action_registry` | validate_action(), is_builtin() | core::actions |
| `gates::evaluator` | DefaultGateEvaluator | core::spec::gates, core::traits |
| `gates::override_gate` | process_override() | core::state |
| `gates::decision` | GateDecisionRecord, CriteriaResult | core::spec::gates |
| `state::phase` | load_state(), save_state() | core::state |
| `state::elevation` | activate(), deactivate(), enforce_ttl() | core::state |
| `state::drift` | append_drift(), read_drift() | core::state |
| `state::atomic` | AtomicWriter, AdvisoryLock | (std::fs) |
| `state::audit_log` | append_audit(), verify_chain() | core::spec::audit |
| `convert::aieos` | import_aieos(), export_aieos() | core::spec (feature-gated) |
| `convert::zeroclaw` | import_zeroclaw(), export_zeroclaw() | core::spec (feature-gated) |

### ampersona-sign

**Responsibility:** Cryptographic operations. Isolated for security review.

| Module | Exports | Depends on |
|--------|---------|-----------|
| `canonical` | canonicalize() → Vec<u8> (JCS/RFC8785) | core (types for signed_fields) |
| `sign` | sign_persona(), sign_state() | canonical, ed25519-dalek |
| `verify` | verify_persona(), verify_state() | canonical, ed25519-dalek |

### amp (binary)

**Responsibility:** CLI argument parsing, command dispatch. Thin wrapper.

| Function | Calls |
|----------|-------|
| `cmd_prompt` | core::prompt |
| `cmd_validate` | core::schema |
| `cmd_new` | core::templates |
| `cmd_templates` | core::templates |
| `cmd_list` | core (scan + print) |
| `cmd_register` | core::register |
| `cmd_init` | core::templates (scaffold) |
| `cmd_check` | core::schema + engine::action_registry + sign::verify |
| `cmd_status` | engine::state |
| `cmd_authority` | engine::policy |
| `cmd_elevate` | engine::state::elevation |
| `cmd_gate` | engine::gates |
| `cmd_migrate` | core::migrate |
| `cmd_import` | engine::convert |
| `cmd_export` | engine::convert |
| `cmd_compose` | core::compose |
| `cmd_diff` | core (diff logic) |
| `cmd_sign` | sign::sign |
| `cmd_verify` | sign::verify |
| `cmd_audit` | engine::state::audit_log |
| `cmd_fleet` | engine (batch) |

## Trait Contracts

### AuthorityEnforcer

```rust
/// Implemented by consumers (e.g., zeroclaw) to enforce policy at runtime.
/// ampersona-engine provides DefaultPolicyChecker as a reference implementation.
pub trait AuthorityEnforcer {
    fn evaluate(
        &self,
        req: &PolicyRequest,
        authority: &ResolvedAuthority,
    ) -> Result<PolicyDecision, PolicyError>;
}
```

**Contract:**
- Must return `Deny` for unknown actions (deny-by-default)
- Must respect precedence order (deny > elevation > gate > persona > workspace)
- Must check rate limits before allowing
- Must not cache decisions across state_rev changes

### MetricsProvider

```rust
/// Implemented by consumers (e.g., zeroclaw) to supply metrics for gate evaluation.
pub trait MetricsProvider {
    fn get_metric(&self, query: &MetricQuery) -> Result<MetricSample, MetricError>;
}
```

**Contract:**
- Must return current value for the named metric
- Must respect `window` if provided (time-windowed aggregation)
- Must return `MetricError::NotFound` for unknown metrics
- Must return `MetricError::TypeMismatch` if metric type doesn't match schema

### AuditSink

```rust
/// Implemented by consumers (e.g., agent_mail_rust) to receive audit events.
pub trait AuditSink {
    fn log_decision(&self, entry: &AuditEntry) -> Result<(), AuditError>;
    fn log_gate_transition(&self, event: &GateTransitionEvent) -> Result<(), AuditError>;
    fn log_elevation(&self, event: &ElevationEvent) -> Result<(), AuditError>;
    fn log_override(&self, event: &OverrideEvent) -> Result<(), AuditError>;
}
```

**Contract:**
- Must persist events durably (not just in-memory)
- Must not silently drop events
- Should include timestamp if not already present
- May batch writes for performance

## State Update Protocol

```
1. Lock:    flock(<name>.state.lock, LOCK_EX | LOCK_NB) → fail if held
2. Read:    load <name>.state.json → PhaseState
3. Verify:  check state_rev matches expected
4. Mutate:  apply changes (transition, elevation, TTL expiry)
5. Bump:    state_rev += 1
6. Write:   serialize to temp file in same directory
7. Sync:    fsync(temp_fd)
8. Rename:  rename(temp, <name>.state.json) → atomic
9. Audit:   append to <name>.audit.jsonl (with prev_hash)
10. Unlock: close lock fd
```

Concurrent writers: second writer gets `EWOULDBLOCK` on step 1 and retries with backoff.
