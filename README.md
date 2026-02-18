# ampersona

Platform for AI agent identity, authority, and trust progression.

```
amp new architect --name Ada           # → persona JSON (identity + authority + gates)
amp check ada.json --strict            # → schema + consistency + action vocab + lint
amp sign ada.json --key admin.key      # → JCS-canonicalized ed25519 signature
amp gate ada.json --evaluate trusted --metrics m.json  # → phase transition + audit
amp authority ada.json --check deploy  # → Allow / Deny / NeedsApproval
amp prompt ada.json --toon             # → system prompt with authority context
amp fleet personas/ --status           # → fleet summary table
```

## Three Pillars

| Pillar | Question | What it covers |
|--------|----------|---------------|
| **Identity** | Who? | Psychology, voice, capabilities, directives |
| **Authority** | What? | Actions, scopes, limits, elevations, deny-by-default |
| **Gates** | When? | Promote/demote criteria, enforcement modes, drift tracking |

## Install

```sh
cargo install --path crates/amp
```

```sh
cargo build --release    # → target/release/amp
```

## Spec

An ampersona v1.0 file has three pillars. Only `name`, `role`, `psychology`, and `voice` are required — authority and gates are optional.

```json
{
  "version": "1.0",
  "name": "QuietStone",
  "role": "Senior DevOps Engineer",
  "psychology": { "neural_matrix": {}, "traits": {} },
  "voice": { "style": {} },
  "capabilities": { "skills": [] },
  "directives": { "core_drive": "..." },
  "authority": {
    "autonomy": "supervised",
    "scope": { "workspace_only": true, "allowed_paths": ["src/**"] },
    "actions": { "allow": ["read_file"], "deny": [{ "action": "delete_production_data", "reason": "..." }] },
    "limits": { "max_actions_per_hour": 50 },
    "elevations": [{ "id": "release-deploy", "grants": {}, "requires": "human", "ttl_seconds": 3600 }]
  },
  "gates": [
    { "id": "trusted", "direction": "promote", "from_phase": "active", "to_phase": "trusted",
      "criteria": [{ "metric": "tasks_completed", "op": "gte", "value": 20 }] }
  ],
  "audit": { "log_decisions": true, "retention_days": 90 }
}
```

v0.2 files (without `version`, `authority`, `gates`) remain valid — `amp validate` auto-detects the version.

### Identity

| Section | Contents |
|---------|----------|
| `psychology` | Neural matrix (6 dims), MBTI, OCEAN, moral compass, mood |
| `voice` | Style, formality, verbosity, syntax, catchphrases, TTS |
| `capabilities` | Skills with priority |
| `directives` | Core drive, goals, constraints |

### Authority

| Field | Purpose |
|-------|---------|
| `autonomy` | `readonly` / `supervised` / `full` |
| `scope` | Workspace paths (allowed/forbidden) |
| `actions` | Allow/deny lists with scoped types (shell, git, file_access) |
| `limits` | Rate limits, cost caps, approval thresholds |
| `elevations` | Temporary permission grants (TTL, human/auto approval) |
| `delegation` | Who the agent can delegate to, max depth |
| `ext` | Consumer-specific extensions (agent_mail, zeroclaw, odoov19) |

### Gates

| Field | Purpose |
|-------|---------|
| `direction` | `promote` (gain trust) or `demote` (lose trust) |
| `enforcement` | `enforce` (apply) or `observe` (log only) |
| `criteria` | Metrics conditions (metric, op, value) |
| `cooldown_seconds` | Minimum time between transitions |
| `on_pass` | Authority overlay applied when gate fires |

## Commands

### Core

```sh
amp prompt persona.json                     # Markdown system prompt
amp prompt persona.json --toon              # TOON format (~29% fewer tokens)
amp validate personas/*.json                # Schema validation (auto-detect version)
amp new architect --name Ada                # Generate from template
amp templates                               # List archetypes
amp list personas/                          # Directory table summary
amp register persona.json --project /path   # mcp_agent_mail bridge
```

### Authority & Gates

```sh
amp init --workspace                        # Bootstrap .ampersona/defaults.json
amp check persona.json --strict --json      # Full validation (schema+actions+lint)
amp authority persona.json --check deploy   # Policy check → Allow/Deny/NeedsApproval
amp elevate persona.json --elevation release-deploy --reason "v2.1 release"
amp gate persona.json --evaluate trusted --metrics m.json
amp gate persona.json --override trust_decay --reason "incident" --approver admin
amp status persona.json --json --drift      # Phase, elevations, drift trend
amp audit persona.json --verify             # Hash-chain integrity check
```

### Signing & Composition

```sh
amp sign persona.json --key admin.key --key-id k-2026-02
amp verify persona.json --pubkey admin.pub
amp compose base.json overlay.json          # Merge with precedence rules
amp diff a.json b.json                      # Compare personas
amp migrate old.json                        # v0.2 → v1.0
amp import external.json --from aieos       # Convert external format
amp export persona.json --to zeroclaw-config
```

### Fleet

```sh
amp fleet personas/ --status                # Summary table
amp fleet personas/ --check --json          # Batch validation report
amp fleet personas/ --apply-overlay auth.json  # Apply authority overlay to all
```

## Architecture

Four Rust crates in a workspace:

| Crate | Purpose |
|-------|---------|
| `ampersona-core` | Types, traits, schema, migration |
| `ampersona-engine` | Policy checker, gate evaluator, state machine |
| `ampersona-sign` | JCS/RFC8785 canonicalization, ed25519 |
| `amp` | Binary crate (CLI) |

Consumers depend on `ampersona-core` for types and traits:

```
zeroclaw → ampersona-core (AuthorityEnforcer + MetricsProvider)
agent_mail_rust → ampersona-core (AuditSink)
```

## Key Design Rules

- **Deny-by-default**: unknown actions denied, unknown fields rejected in strict mode
- **Authority precedence**: explicit deny → elevation → gate overlay → persona → workspace defaults
- **Gate evaluation**: demote wins over promote, deterministic priority sorting, cooldown prevents flapping
- **State updates**: atomic write (temp+fsync+rename), advisory lock, monotonic state_rev
- **Audit**: hash-chain with signed checkpoints, tamper-evident

## License

MIT
