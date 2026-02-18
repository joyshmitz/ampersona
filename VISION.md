# ampersona — Vision

> Platform for AI agent identity, authority, and trust progression.

---

## The One-Sentence Vision

**Every AI agent gets a typed identity (who it is), scoped authority (what it may do), and trust gates (when permissions grow or shrink) — defined in JSON, enforced at runtime, audited with hash-chain integrity.**

---

## Three Pillars

| Pillar | Question | What it covers |
|--------|----------|---------------|
| **Identity** | Who is the agent? | Psychology, voice, capabilities, directives |
| **Authority** | What may it do? | Actions, scopes, limits, elevations, deny-by-default |
| **Gates** | When does trust change? | Promote/demote criteria, enforcement modes, drift tracking |

---

## Core Philosophy

1. **Behavioral signal only** — If it doesn't change how an LLM writes or decides, it doesn't belong. No appearance, no diet, no hobbies.
2. **Deny-by-default** — Unknown actions are denied. Unknown fields rejected in strict mode. Fail-closed, not fail-open.
3. **Spec-first** — JSON Schema defines the contract. Code implements the spec, not the other way around.
4. **Trait-first** — Consumers implement `AuthorityEnforcer`, `MetricsProvider`, `AuditSink`. No monolithic runtime.
5. **Unix pipes** — stdin/stdout composable with jq, toon, and mcp_agent_mail CLI.
6. **Schema-first** — JSON Schema Draft 2020-12, embedded in the binary, validates at build time.
7. **AdjectiveNoun names** — Persona names are mcp_agent_mail compatible by design.
8. **TOON-native** — Token-efficient encoding is first-class, not an afterthought.
9. **Safety-first conflict resolution** — Demote wins over promote. Deny wins over allow. Minimum wins for limits.

---

## The Stack

```
amp new architect --name BrightTower
    ↓ persona JSON (identity + authority + gates)
amp check --strict
    ↓ schema + consistency + action vocab + lint
amp sign --key admin.key
    ↓ JCS-canonicalized ed25519 signature
amp gate --evaluate onboarding --metrics m.json
    ↓ state transition + drift + audit record
amp authority --check write_file
    ↓ Allow / Deny / NeedsApproval (with precedence)
amp prompt --toon
    ↓ TOON system prompt with authority context
amp register --project /data/projects/backend
    ↓ mcp_agent_mail register_agent call
agent coordination (messages, threads, file reservations)
```

---

## What ampersona IS

- A spec for agent identity, authority, and trust gates (JSON Schema v1.0)
- A policy engine with layered precedence and deny-by-default
- A state machine for trust progression with deterministic gate evaluation
- A CLI tool to manage the full lifecycle: init → check → sign → gate → status → prompt → fleet
- Typed Rust traits for consumer integration (`AuthorityEnforcer`, `MetricsProvider`, `AuditSink`)
- A bridge to mcp_agent_mail agent registration
- Built-in archetypes for quick bootstrapping
- Backward-compatible with v0.2 persona format

## What ampersona is NOT

- Not a character creator (no face, body, clothes)
- Not a chat framework (use mcp_agent_mail)
- Not an LLM wrapper (it produces prompts and policy decisions, doesn't run models)
- Not a secrets manager (key material lives outside persona files)
- Not a runtime sandbox (consumers like zeroclaw handle actual enforcement)

---

## Four Test Subjects

| Consumer | Relationship | Integration |
|----------|-------------|-------------|
| **mcp_agent_mail** (Python) | JSON-only consumer | `ext.agent_mail.contact_policy` |
| **mcp_agent_mail_rust** | Optional crate dependency | `ampersona-core` types + `AuditSink` trait |
| **zeroclaw** | Required crate dependency | `AuthorityEnforcer` + `MetricsProvider` traits |
| **odoov19** | JSON + CLI consumer | Compliance markers, phase gates F1→F4 |

---

## Versioning

- **v0.2** — Lean CLI: persona JSON → system prompt (5 sections, 6 commands)
- **v1.0** — Platform: identity + authority + gates (3 pillars, 20+ commands, 4 crates)

Schema evolution: semver MAJOR.MINOR. Minor = additive optional fields. Major = breaking (requires `amp migrate`).

---

## License

MIT
