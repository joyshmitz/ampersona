# AGENTS.md — ampersona

> Guidelines for AI coding agents working in this Rust codebase.

---

## Agent Mail Identity

| Agent | Program | Name |
|-------|---------|------|
| claude-code | claude-code | **LilacCat** |

---

## What This Project Is

**ampersona** is a platform for AI agent identity, authority, and trust progression. Three pillars:

| Pillar | Question | Sections |
|--------|----------|----------|
| **Identity** | Who is the agent? | psychology, voice, capabilities, directives |
| **Authority** | What may it do? | autonomy, scope, actions, limits, elevations, delegation |
| **Gates** | When does trust change? | promote/demote criteria, enforcement modes, metrics |

The `amp` CLI manages the full lifecycle: init → check → sign → gate → status → prompt → fleet → register.

### Key Files

| File | Purpose |
|------|---------|
| `VISION.md` | Scope, exclusions, philosophy — frozen |
| `SPEC.md` | Schema, types, CLI, traits, state, precedence rules |
| `ARCHITECTURE.md` | Crate boundaries, module map, trait contracts |
| `FEATURE_PARITY.md` | Status matrix (50+ rows) |
| `ADR/` | 6 Architecture Decision Records (immutable) |
| `THREAT_MODEL.md` | Assets, trust boundaries, abuse cases, mitigations |
| `OPERATIONS.md` | Key rotation, backup/restore, incident playbooks |

### Crate Structure

| Crate | Path | Purpose |
|-------|------|---------|
| `ampersona-core` | `crates/ampersona-core/` | Types, traits, schema, migration, serde |
| `ampersona-engine` | `crates/ampersona-engine/` | Policy checker, gate evaluator, state, precedence |
| `ampersona-sign` | `crates/ampersona-sign/` | JCS canonicalization, ed25519 sign/verify |
| `amp` | `crates/amp/` | Binary crate (CLI) |

### Consumer Integration

| Consumer | Dependency | Traits |
|----------|-----------|--------|
| mcp_agent_mail (Python) | JSON only | — |
| mcp_agent_mail_rust | `ampersona-core` | AuditSink |
| zeroclaw | `ampersona-core` | AuthorityEnforcer + MetricsProvider |
| odoov19 | JSON + CLI | — |

---

## Toolchain

- **Language:** Rust 2021 edition
- **Binary:** `amp`
- **Workspace:** 4 crates (see above)
- **Key deps:** clap 4, serde_json, jsonschema, ed25519-dalek, toon (path dep)
- **Unsafe code:** `#![forbid(unsafe_code)]` in all crates

### Build & Test

```bash
cargo fmt --check                           # format check
cargo clippy -- -D warnings                 # lint
cargo test --workspace                      # all tests
cargo build --release                       # release build
```

### Test Levels

| Level | What | Where |
|-------|------|-------|
| Unit | PolicyChecker, GateEvaluator, precedence | `crates/*/src/**/tests.rs` |
| Integration | CLI e2e | `tests/` workspace root |
| Backward compat | v0.2 examples | `tests/backward_compat.rs` |
| Golden | persona → expected prompt | `tests/golden/` |
| Consumer fixtures | per-consumer examples | `examples/*.json` |
| Property-based | precedence/merge invariants | `tests/property/*.rs` |
| Fuzz | parse/migrate/import robustness | `fuzz/` |
| Concurrency | lock contention + idempotency | `tests/concurrency/*.rs` |

---

## Code Discipline

- **No file deletion** without explicit permission
- **Edit existing files** — don't create `_v2` or `_improved` variants
- **No backwards-compat shims** — fix it right, no tech debt
- **Verify after changes:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test --workspace`
- **Spec-first** — code only from specification
- **Deny-by-default** — unknown actions denied, unknown fields rejected in strict mode
- **`#![forbid(unsafe_code)]`** in every crate

---

## Integration Points

### mcp_agent_mail

ampersona is the **identity+authority layer** on top of agent mail coordination.

The `amp register` subcommand:
1. Reads persona JSON (file or stdin)
2. Extracts `name` (AdjectiveNoun) → agent name
3. Uses `role` as `task_description` (or full system prompt with `--prompt`)
4. Outputs `register_agent` arguments as JSON (composable with pipes)
5. `--rpc` wraps in JSON-RPC 2.0 envelope
6. `--toon` uses TOON format for task_description
7. `--with-authority` includes authority summary in task_description

### TOON (toon_rust)

Path dependency at `../toon_rust`. Token-efficient persona encoding (~29% fewer tokens).

### Naming Convention

Persona `name` field MUST be AdjectiveNoun format for mcp_agent_mail compatibility:
- Valid: `QuietStone`, `WarmBirch`, `SharpFlint`
- Invalid: `Rex`, `DevOpsBot`, `security-auditor`

---

## Session Protocol

1. Read `AGENTS.md` before starting work
2. Check existing code before making changes
3. `cargo fmt --check && cargo clippy -- -D warnings && cargo test --workspace` after any code change
4. Commit with descriptive messages
