# ampersona — Specification v1.0

## Scope

**Goal:** Deterministic policy engine, trust gates, and audit infrastructure for AI agent identities. ampersona owns the authority/gates/audit runtime contract and exposes it via a CLI (`amp`) and Rust crate API.

**Non-goals:** ampersona does not perform transport, event-source orchestration, scheduling, or peripheral management. Those responsibilities belong to the runtime that consumes ampersona (e.g., zeroclaw handles MQTT/webhook/cron fan-in and orchestration; mcp_agent_mail handles message routing). The boundary is: ampersona evaluates policy and gates deterministically given inputs; the caller is responsible for sourcing those inputs.

## Schema Version

- **Current:** `1.0`
- **Previous:** `0.2` (implicit — no `version` field)
- **$schema URI:** `https://ampersona.dev/schema/v1.0/ampersona.schema.json`
- **Evolution:** semver MAJOR.MINOR. Minor = additive optional fields. Major = breaking (requires `amp migrate`). Deprecated fields warn for 2 minor versions.

## Root Document

```
{
  "$schema": string (optional),
  "version": "1.0" | absent (v0.2),
  "name": string (required, AdjectiveNoun),
  "role": string (required),
  "backstory": string (optional),
  "signature": Signature (optional),
  "psychology": Psychology (required),
  "voice": Voice (required),
  "capabilities": Capabilities (optional),
  "directives": Directives (optional),
  "authority": Authority (optional),
  "gates": Gate[] (optional),
  "audit": AuditConfig (optional)
}
```

Version detection: if `version` field is absent, treat as v0.2. If present, must be `"1.0"`.

---

## Pillar 1: Identity

### Psychology (required)

```
Psychology {
  neural_matrix: NeuralMatrix (required),
  traits: Traits (required),
  moral_compass: MoralCompass (optional),
  emotional_profile: EmotionalProfile (optional)
}

NeuralMatrix {
  creativity: UnitFloat,
  empathy: UnitFloat,
  logic: UnitFloat,
  adaptability: UnitFloat,
  charisma: UnitFloat,
  reliability: UnitFloat
}

Traits {
  ocean: Ocean (required),
  mbti: MbtiType (required),
  temperament: string (optional)
}

Ocean {
  openness: UnitFloat,
  conscientiousness: UnitFloat,
  extraversion: UnitFloat,
  agreeableness: UnitFloat,
  neuroticism: UnitFloat
}

MbtiType = "ISTJ" | "ISFJ" | "INFJ" | "INTJ" | "ISTP" | "ISFP" | "INFP" | "INTP"
         | "ESTP" | "ESFP" | "ENFP" | "ENTP" | "ESTJ" | "ESFJ" | "ENFJ" | "ENTJ"

MoralCompass {
  alignment: Alignment (required),
  core_values: string[] (min 1)
}

Alignment = "lawful-good" | "neutral-good" | "chaotic-good"
          | "lawful-neutral" | "true-neutral" | "chaotic-neutral"
          | "lawful-evil" | "neutral-evil" | "chaotic-evil"

EmotionalProfile {
  base_mood: string (required),
  volatility: UnitFloat (required)
}
```

### Voice (required)

```
Voice {
  style: VoiceStyle (required),
  syntax: VoiceSyntax (optional),
  idiolect: Idiolect (optional),
  tts: TtsConfig (optional)
}

VoiceStyle {
  descriptors: string[] (min 1),
  formality: UnitFloat,
  verbosity: UnitFloat
}

VoiceSyntax {
  structure: string (optional),
  contractions: bool (optional)
}

Idiolect {
  catchphrases: string[] (optional),
  forbidden_words: string[] (optional)
}

TtsConfig {
  provider: string (required),
  voice_id: string (required),
  stability: UnitFloat (optional),
  similarity_boost: UnitFloat (optional),
  speed: UnitFloat (optional)
}
```

### Capabilities (optional)

```
Capabilities {
  skills: Skill[]
}

Skill {
  name: string (required),
  description: string (required),
  priority: integer 1..10 (optional, 1 = highest)
}
```

### Directives (optional)

```
Directives {
  core_drive: string (optional),
  goals: string[] (optional),
  constraints: string[] (optional)
}
```

---

## Pillar 2: Authority

```
Authority {
  autonomy: AutonomyLevel (required),
  scope: Scope (optional),
  actions: Actions (optional),
  limits: Limits (optional),
  elevations: Elevation[] (optional),
  delegation: Delegation (optional),
  ext: map<string, any> (optional)
}

AutonomyLevel = "readonly" | "supervised" | "full"
```

### Scope

```
Scope {
  workspace_only: bool (optional, default true),
  allowed_paths: string[] (optional, glob patterns),
  forbidden_paths: string[] (optional, glob patterns)
}
```

### Actions

```
Actions {
  allow: ActionRef[] (optional),
  deny: DenyEntry[] (optional),
  scoped: map<string, ScopedAction> (optional)
}

ActionRef = string (ActionId)

DenyEntry = string (ActionId)
          | { action: ActionId, reason: string, compliance_ref: string (optional) }

ScopedAction = ScopedShell | ScopedGit | ScopedFileAccess | ScopedCustom
```

#### ActionId

ActionId is either a **builtin** name or a **custom** namespace:

**Builtin actions:** `read_file`, `write_file`, `delete_file`, `run_tests`, `run_command`, `git_commit`, `git_push`, `git_push_main`, `git_pull`, `create_branch`, `delete_branch`, `create_pr`, `merge_pr`, `deploy`, `install_package`, `modify_config`, `access_network`, `send_message`, `approve_change`, `delete_production_data`, `auto_approve_capa`

**Custom actions:** `custom:<vendor>/<action>` (e.g., `custom:zeroclaw/sandbox_escape`, `custom:odoov19/approve_capa`)

Unknown actions (not builtin and not `custom:` prefixed) are validation errors in `--strict` mode and denied by the policy engine.

#### Scoped Types

```
ScopedShell {
  $type: "shell",
  commands: string[] (optional, allowlist),
  block_high_risk: bool (optional),
  block_subshells: bool (optional),
  block_redirects: bool (optional),
  block_background: bool (optional),
  validate_symlinks: bool (optional)
}

ScopedGit {
  $type: "git",
  allowed_operations: string[] (optional),
  push_branches: string[] (optional, glob patterns),
  deny_push_branches: string[] (optional, glob patterns)
}

ScopedFileAccess {
  $type: "file_access",
  read: string[] (optional, glob patterns),
  write: string[] (optional, glob patterns),
  deny_write: string[] (optional, glob patterns)
}

ScopedCustom {
  $type: "custom",
  ... (arbitrary fields)
}
```

### Limits

```
Limits {
  max_actions_per_hour: integer (optional),
  max_cost_per_day_cents: integer (optional),
  require_approval_for: RiskLevel[] (optional)
}

RiskLevel = "low_risk" | "medium_risk" | "high_risk"
```

### Elevations

```
Elevation {
  id: string (required),
  grants: ElevationGrants (required),
  requires: GateApproval (required),
  ttl_seconds: integer (required, > 0),
  reason_required: bool (optional, default false)
}

ElevationGrants {
  "actions.allow": ActionId[] (optional),
  ... (path into Authority to overlay)
}

GateApproval = "auto" | "human" | "quorum"
  - auto: gate fires immediately when criteria pass
  - human: creates pending transition; requires `amp gate --approve` to apply
  - quorum: reserved for v1.1 (returns error in v1.0)
```

### Delegation

```
Delegation {
  can_delegate_to: string[] (optional, role names),
  max_depth: integer (optional, >= 1)
}
```

### Extensions (ext)

Consumer-specific fields under namespaced keys:

```
ext: {
  "agent_mail": { "contact_policy": "auto" | "open" | "contacts_only" | "block_all" },
  "zeroclaw": { "sandbox": "landlock" | "none", "pairing_required": bool },
  "odoov19": { "sign_required_for_policy_changes": bool, "compliance": string[] }
}
```

---

## Pillar 3: Gates

```
Gate {
  id: string (required, unique within persona),
  direction: GateDirection (required),
  enforcement: GateEnforcement (optional, default "enforce"),
  priority: integer (optional, default 0),
  cooldown_seconds: integer (optional, default 0),
  from_phase: string | null (required),
  to_phase: string (required),
  criteria: Criterion[] (required, min 1),
  metrics_schema: map<string, MetricSchema> (optional),
  approval: GateApproval (optional, default "auto"),
  on_pass: GateEffect (optional)
}

GateDirection = "promote" | "demote"
GateEnforcement = "enforce" | "observe"

Criterion {
  metric: string (required),
  op: CriterionOp (required),
  value: any (required),
  window_seconds: integer (optional, minimum 1)
}

CriterionOp = "eq" | "neq" | "gt" | "gte" | "lt" | "lte"

MetricSchema {
  type: "boolean" | "integer" | "number" | "string"
}

GateEffect {
  authority_overlay: partial Authority (optional)
}
```

---

## Audit

```
AuditConfig {
  log_decisions: bool (optional, default false),
  log_gate_transitions: bool (optional, default true),
  retention_days: integer (optional),
  compliance_markers: string[] (optional)
}

AuditEventType = "PolicyDecision" | "GateTransition" | "ElevationChange"
               | "Override" | "SignatureVerify" | "StateChange"
```

---

## Signature

```
Signature {
  algorithm: "ed25519" (required),
  key_id: string (required),
  signer: string (required),
  canonicalization: "JCS-RFC8785" (required),
  signed_fields: string[] (required),
  created_at: ISO8601 datetime (required),
  digest: string (required, "sha256:..."),
  value: string (required, base64-encoded signature)
}
```

**Rules:**
1. `signed_fields` must cover all non-signature top-level fields
2. Canonicalize payload per RFC 8785/JCS before hashing
3. Hash with SHA-256, sign hash with ed25519
4. Verification fails on mismatch of `canonicalization`, `key_id`, or `signed_fields`
5. `key_id` enables key rotation

---

## State Architecture

| File | Purpose | Integrity |
|------|---------|-----------|
| `persona.json` | Persona spec | Signed (ed25519, JCS) |
| `<name>.state.json` | Phase + elevations | Signed, `state_rev` |
| `<name>.state.lock` | Advisory writer lock | Lock file |
| `<name>.audit.jsonl` | Audit events | Hash-chain (append-only) |
| `<name>.drift.jsonl` | Metrics snapshots | Hash-chain (append-only) |
| `<name>.integrity.json` | Signed checkpoint | Signed |
| `.ampersona/defaults.json` | Workspace defaults | Optional signing |

### State File

```
PhaseState {
  name: string,
  current_phase: string | null,
  state_rev: integer (monotonic),
  active_elevations: ActiveElevation[],
  last_transition: TransitionRecord | null,
  pending_transition: PendingTransition | null,
  updated_at: ISO8601 datetime
}

ActiveElevation {
  elevation_id: string,
  granted_at: ISO8601 datetime,
  expires_at: ISO8601 datetime,
  reason: string,
  granted_by: string
}

TransitionRecord {
  gate_id: string,
  from_phase: string | null,
  to_phase: string,
  at: ISO8601 datetime,
  decision_id: string,
  metrics_hash: string | null,
  state_rev: integer
}

PendingTransition {
  gate_id: string,
  from_phase: string | null,
  to_phase: string,
  decision: string,
  metrics_hash: string,
  state_rev: integer,
  created_at: ISO8601 datetime
}
```

### State Update Protocol

1. Acquire advisory lock on `<name>.state.lock`
2. Read current state, verify `state_rev`
3. Apply changes, increment `state_rev`
4. Write to temp file
5. `fsync` temp file
6. `rename` temp to `<name>.state.json` (atomic)
7. Release lock

### Hash-Chain

Each entry in audit/drift logs:
```
{
  "prev_hash": "sha256:..." | "genesis",
  "event_type": AuditEventType,
  ... (event-specific fields)
  "ts": ISO8601 datetime
}
```

`amp audit --verify` walks the chain and validates every `prev_hash`.

---

## Authority Precedence

Highest to lowest:

1. **Explicit deny** — always wins
2. **Active elevation grants** (within TTL)
3. **Gate authority_overlay** (current phase)
4. **Persona authority**
5. **Workspace defaults** (`.ampersona/defaults.json`)

**Merge rules:**
- `deny` = union (all denies from all layers)
- `allow` = intersection of allows, minus deny union
- `limits` = minimum across layers
- `autonomy` = minimum across layers

---

## Gate Conflict Resolution

Deterministic algorithm:

1. Collect candidate gates whose `from_phase` matches current phase
2. Filter out gates still in cooldown
3. Sort candidates by `(direction: demote > promote, priority DESC, id ASC)`
4. Evaluate criteria for each candidate in order
5. First gate where ALL criteria pass wins
6. One transition per evaluation tick
7. Human gates create pending transition; auto gates apply immediately
8. Evaluations are idempotent per `(gate_id, metrics_hash, state_rev)`

---

## Override Mechanism

Distinct from elevation. Override = emergency bypass of a failed gate.

**Requirements:**
- Gate criteria MUST be failing (otherwise normal transition applies)
- Approver must have delegation level higher than gate's approval requirement
- Mandatory `reason` + `approver` identity
- Metrics snapshot captured at override time
- `is_override: true` in decision record
- Audit entry with full context

---

## CLI

### Existing Commands (extended)

| Command | v0.2 | v1.0 additions |
|---------|------|----------------|
| `amp prompt` | Markdown/TOON | + authority/gates sections |
| `amp validate` | Schema check | + auto-detect version |
| `amp new` | Templates | + authority templates |
| `amp templates` | List | unchanged |
| `amp list` | Table | unchanged |
| `amp register` | Bridge | + `--with-authority` |

### New Commands

| Command | Purpose |
|---------|---------|
| `amp init [--workspace]` | Bootstrap persona / `.ampersona/defaults.json` |
| `amp check <file> [--metrics f] [--json] [--strict]` | Unified validation |
| `amp status <file> [--json] [--drift]` | Phase, autonomy, elevations, events, drift |
| `amp authority <file> --check <action>` | Policy check → Allow/Deny/NeedsApproval |
| `amp elevate <file> --elevation <id> --reason "..."` | Temporary auth grant |
| `amp gate <file> --evaluate <gate-id> --metrics <file>` | Gate evaluation (exit 0=transition, 1=no_match, 2=pending_human) |
| `amp gate <file> --approve <gate-id>` | Approve pending human gate |
| `amp gate <file> --override <gate-id> --reason "..." --approver <id>` | Emergency bypass (requires phase match + criteria failing) |
| `amp migrate <files...>` | v0.2 → v1.0 upgrade |
| `amp import <file> --from aieos\|zeroclaw` | Convert external → ampersona |
| `amp export <file> --to aieos\|zeroclaw-config` | Convert ampersona → external |
| `amp compose <base> <overlay>` | Merge personas |
| `amp diff <a> <b>` | Compare personas |
| `amp sign <file> --key <key> [--key-id <id>]` | Sign persona |
| `amp verify <file> --pubkey <key>` | Verify signature |
| `amp audit <file> --verify [--from N]` | Verify hash-chain (from entry N) |
| `amp audit <file> --checkpoint-create [--checkpoint <path>] [--sign-key <key>]` | Create integrity checkpoint |
| `amp audit <file> --checkpoint-verify [--checkpoint <path>] [--verify-key <key>]` | Verify checkpoint |
| `amp fleet <dir> --status` | Fleet summary table |
| `amp fleet <dir> --check [--json]` | Batch validation |
| `amp fleet <dir> --apply-overlay <overlay.json>` | Apply authority overlay |

### Structured Error Output (`amp check --json`)

```json
{
  "file": "persona.json",
  "version": "1.0",
  "pass": false,
  "errors": [
    { "code": "E001", "check": "schema", "message": "...", "path": "$.name" }
  ],
  "warnings": [
    { "code": "W001", "check": "lint", "message": "supervised autonomy without gates" }
  ]
}
```

**Error codes:**
- `E001-E009`: Schema validation errors
- `E010-E019`: Action vocabulary errors
- `E020-E029`: Consistency errors (acyclicity, metrics_schema match)
- `E030-E039`: Signature errors
- `W001-W009`: Lint warnings (missing compliance_ref, autonomy without gates)

---

## Types Summary

```
UnitFloat:       f64 in [0.0, 1.0]
AutonomyLevel:   readonly | supervised | full
CriterionOp:     eq | neq | gt | gte | lt | lte
GateApproval:    auto | human | quorum
GateDirection:   promote | demote
GateEnforcement: enforce | observe
PolicyDecision:  Allow { reason } | Deny { reason } | NeedsApproval { reason }
PolicyError:     InvalidAction | InternalError
MetricError:     NotFound | TypeMismatch | ProviderUnavailable
AuditError:      WriteFailure | ChainCorruption
ScopedType:      shell | git | file_access | custom
ActionId:        builtin enum | custom:<vendor>/<action>
AuditEventType:  PolicyDecision | GateTransition | ElevationChange | Override | SignatureVerify | StateChange
RiskLevel:       low_risk | medium_risk | high_risk
```

---

## Traits

```rust
pub struct MetricQuery {
    pub name: String,
    pub window: Option<Duration>,
}

pub struct MetricSample {
    pub name: String,
    pub value: serde_json::Value,
    pub sampled_at: DateTime<Utc>,
}

pub trait MetricsProvider {
    fn get_metric(&self, query: &MetricQuery) -> Result<MetricSample, MetricError>;
}

pub struct PolicyRequest {
    pub action: Option<ActionId>,
    pub path: Option<String>,
    pub context: HashMap<String, serde_json::Value>,
}

pub struct ResolvedAuthority { /* all layers merged */ }

pub trait AuthorityEnforcer {
    fn evaluate(&self, req: &PolicyRequest, authority: &ResolvedAuthority) -> Result<PolicyDecision, PolicyError>;
}

pub trait AuditSink {
    fn log_decision(&self, entry: &AuditEntry) -> Result<(), AuditError>;
    fn log_gate_transition(&self, event: &GateTransitionEvent) -> Result<(), AuditError>;
    fn log_elevation(&self, event: &ElevationEvent) -> Result<(), AuditError>;
    fn log_override(&self, event: &OverrideEvent) -> Result<(), AuditError>;
}
```

---

## Contract Versioning

All consumers must declare `ampersona_contract`. Incompatible major versions fail fast.

```
ampersona_contract = "1.x"  // consumer declares this
```

Compatibility: `1.0` spec works with any `1.x` consumer. `2.0` spec requires `2.x` consumer.
