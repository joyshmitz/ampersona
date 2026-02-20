# AGENTS.md — ampersona

> Universal entry point for all AI coding agents working in this repository.
> No vendor assumptions. Read this first, every session.

---

## Agent Mail Identity

| Agent | Program | Name |
|-------|---------|------|
| claude-code | claude-code | **LilacCat** |

---

## What This Project Is

**ampersona** is a platform for AI agent identity, authority, and trust progression.

Three pillars in one JSON file:

| Pillar | Question | Enforced by |
|--------|----------|-------------|
| **Identity** | Who is the agent? | `amp prompt` → system prompt |
| **Authority** | What may it do? | `AuthorityEnforcer` trait → runtime deny |
| **Gates** | When does trust change? | `MetricsProvider` → `DefaultGateEvaluator` |

The platform is **domain-agnostic by design**. A boiler room, a deployment pipeline,
a medical procedure — the same three traits, the same gate algebra, the same audit chain.

The `amp` CLI manages the full lifecycle: init → check → sign → gate → status → prompt → fleet → register.

---

## Orientation

| File | Purpose | Mutability |
|------|---------|------------|
| `VISION.md` | Scope, philosophy, what ampersona is NOT | Frozen |
| `SPEC.md` | Schema, types, CLI contract, trait contracts | Spec-first |
| `ARCHITECTURE.md` | Crate map, module boundaries, dependency graph | Follows spec |
| `THEORY.md` | Mathematical foundation, formal model, proven properties | Append-only |
| `ROADMAP.md` | Current state, v1.1 targets, failure modes | Living document |
| `FEATURE_PARITY.md` | Status matrix per feature | Updated with each phase |
| `ADR/` | Architecture Decision Records | Immutable once accepted |
| `THREAT_MODEL.md` | Assets, trust boundaries, abuse cases | Reviewed per release |
| `OPERATIONS.md` | Key rotation, backup, incident playbooks | Operational |

**Before writing any code:** read `THEORY.md` and `ROADMAP.md`.
They define what is done, what is next, and what must not be touched.

---

## Crate Structure

```
ampersona-core     types, traits, schema, migration — no business logic
ampersona-engine   policy checker, gate evaluator, state machine — business logic
ampersona-sign     JCS canonicalization, ed25519 — isolated for security review
amp                binary crate (CLI) — thin dispatch layer
```

Dependency rule: `ampersona-core` has zero business logic.
Consumers depend only on `ampersona-core`. Never on `ampersona-engine`.

---

## Three Public Traits — The Integration Contract

```rust
// ampersona-core/src/traits.rs

trait MetricsProvider     // consumer supplies: X (domain metrics)
trait AuthorityEnforcer   // consumer uses: invariant checker
trait AuditSink           // consumer supplies: proof transcript
```

These three traits are the **entire public surface** for domain integration.
A new domain implements them. The platform changes nothing.

---

## Toolchain

- Rust 2021 edition, `#![forbid(unsafe_code)]` in every crate
- **Key deps:** clap 4, serde_json, jsonschema, ed25519-dalek, tru (git dep: `toon_rust`)
- `cargo fmt --check && cargo clippy -- -D warnings && cargo test --workspace`
- 55 features done, 129 tests green — this is the baseline

### Test Levels

| Level | What | Where |
|-------|------|-------|
| Unit | PolicyChecker, GateEvaluator, precedence | `crates/*/src/**` (embedded `#[cfg(test)]`) |
| Integration | CLI e2e | `crates/amp/tests/` |
| Conformance | spec-runtime contracts, boundary hardening | `crates/amp/tests/conformance_tests.rs` |
| Backward compat | v0.2 examples | `crates/amp/tests/backward_compat.rs` |
| Golden | persona → expected prompt | `crates/amp/tests/golden/` |
| Consumer fixtures | per-consumer examples | `examples/*.json` |
| Property-based | precedence/merge invariants | embedded in `policy/precedence.rs` |
| Fuzz | parse/migrate/import robustness | `fuzz/` |
| Concurrency | lock contention + idempotency | embedded in `state/atomic.rs` |

---

## Code Discipline

**Always:**
- Read `THEORY.md` and `ROADMAP.md` before starting
- Check existing code before writing new code
- Spec-first: ADR → `SPEC.md` → types → engine → CLI → tests
- Run full check after every change
- Deny-by-default: unknown actions denied, unknown fields rejected in strict mode

**Never:**
- Delete files without explicit instruction
- Create `_v2` or `_improved` variants — edit in place
- Add backwards-compat shims — fix it right, no tech debt
- Write code without an ADR if the change touches public types or traits
- Move to the next roadmap phase while the current one has failing tests
- Implement `CriteriaLogic::Not` or recursive nesting without a concrete use case
- Use `.ampersona/` directory for development agent configuration —
  that directory is for production persona management, not repository tooling

---

## Change Protocol for Public Types or Traits

```
1. ADR        — decision + consequences, immutable once merged
2. SPEC.md    — type specification
3. JSON Schema — if serialization is affected
4. Core types — ampersona-core
5. Engine     — business logic
6. CLI        — commands if needed
7. Tests      — unit + integration + backward compat
8. Migration  — amp migrate if breaking
```

Skipping steps is a failure mode. See `ROADMAP.md` for recovery paths.

---

## Consumer Map

| Consumer | Depends on | Traits used |
|----------|-----------|-------------|
| `mcp_agent_mail_rust` | `ampersona-core` | `AuditSink` |
| `zeroclaw` / `e-claw` | `ampersona-core` | `AuthorityEnforcer` + `MetricsProvider` |
| `odoov19` | JSON + CLI | — |
| `mcp_agent_mail` (Python) | JSON only | — |

The integration between `zeroclaw` and `ampersona` is the **primary unfinished seam**.
See `ROADMAP.md` Phases 1–2.

### mcp_agent_mail Integration

ampersona is the **identity+authority layer** on top of agent mail coordination.

The `amp register` subcommand:
1. Reads persona JSON (file or stdin)
2. Extracts `name` (AdjectiveNoun) → agent name
3. Uses `role` as `task_description` (or full system prompt with `--prompt`)
4. Outputs `register_agent` arguments as JSON (composable with pipes)
5. `--rpc` wraps in JSON-RPC 2.0 envelope
6. `--toon` uses TOON format for task_description
7. `--with-authority` includes authority summary in task_description

### Naming Convention

Persona `name` field MUST be AdjectiveNoun format for mcp_agent_mail compatibility:
- Valid: `QuietStone`, `WarmBirch`, `SharpFlint`
- Invalid: `Rex`, `DevOpsBot`, `security-auditor`

---

## Session Protocol

1. Read this file before starting work
2. Read `THEORY.md` and `ROADMAP.md` for current state
3. Check existing code before making changes
4. `cargo fmt --check && cargo clippy -- -D warnings && cargo test --workspace` after any code change
5. Commit with descriptive messages
