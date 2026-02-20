# THEORY.md — ampersona Mathematical Foundation

> Reference document. Not a tutorial.
> Defines what the platform guarantees and what it does not.

---

## Formal Model

ampersona implements a **hybrid automaton** — a system with both discrete transitions
and continuous state.

```
H = (Q, X, Guard, Reset, Inv, Policy)

Q      discrete states        PhaseState.current_phase      (e.g. "active", "trusted")
X      continuous state        MetricSample.value            (domain metrics, serde_json::Value)
Guard  transition condition    Gate.criteria → Vec<Criterion> evaluated as AND
Reset  transition result       Reset(q→q') = { phase: q', overlay: gate.on_pass, rev: rev+1 }
Inv    phase invariant         PhaseInvariant.must_hold      [NOT YET IMPLEMENTED — see Gaps]
Policy phase → authority       Q → P(A): phase-dependent authority via active_overlay
```

The hybrid automaton is **parameterized** by domain:

```
Platform<X>  where X: domain metric type (supplied via MetricsProvider)
```

The platform does not know what X is. A boiler room and a CI pipeline are both valid X.

---

## Authority Resolution — Meet-Semilattice

`resolve_authority(layers: &[&Authority]) -> ResolvedAuthority`

is a **monotone function** over the permission lattice:

```
deny     = union  (⊔)    adding a layer never removes a denial
allow    = intersection (⊓)   adding a layer never adds a permission
autonomy = min           most restrictive wins
limits   = min           most restrictive wins
```

**Absolute invariant (proven, not assumed):**
Explicit `deny` is unconditional. No elevation, no gate overlay, no phase transition
can override an explicit deny entry.

Proof: `test elevation_denied_action_not_granted` in `policy/precedence.rs`.

**Consequence:** the authority lattice is **fail-closed**. Unknown action → Deny.
Unknown field in strict mode → reject. There is no fail-open path.

### Authority Overlay — Post-Resolution Patch (ADR-010)

Gate transitions can carry `on_pass.authority_overlay` that modifies effective authority
for the target phase. Overlay uses **patch-replace** semantics, not meet-semilattice merge:

```
apply_overlay(resolved, overlay):
  if overlay.autonomy:    result.autonomy = overlay.autonomy     // REPLACE
  if overlay.actions.deny: result.denied ∪= overlay.deny        // UNION (additive)
  if overlay.actions.allow: result.allowed = overlay.allow \ denied // REPLACE minus deny
  if overlay.scope:       result.scope = overlay.scope           // REPLACE
  if overlay.limits:      result.limits = overlay.limits         // REPLACE
```

**Why not meet-semilattice?** A meet-semilattice merge is mathematically incapable
of expanding permissions: `min(supervised, full) = supervised`. Overlay's purpose is
to grant phase-appropriate authority (e.g., promote → full autonomy). Patch-replace
is the only correct semantics for this.

**Deny preservation invariant:** `deny(persona) ⊆ deny(effective)`.
Overlay can add deny entries but never remove them.
Proof: `test overlay_cannot_override_deny` in `policy/precedence.rs`.

**Lifecycle:** stored in `PhaseState.active_overlay`. Second overlay replaces first
completely (no stacking). Cleared on override.

---

## Gate Evaluation — Deterministic Algorithm

Sorting key: `(direction: Demote=0 < Promote=1, priority DESC, id ASC)`

Properties:
- One transition per evaluation tick
- Demote wins over promote when both criteria pass simultaneously
- Cooldown prevents Zeno condition (infinite transitions in finite time)
- Idempotency: same `(gate_id, metrics_hash, state_rev)` → same output

`metrics_hash` is `sha256(sorted key:value pairs)` — deterministic across insertion order.

**Consequence:** gate evaluation is a **pure function** of `(gates, PhaseState, MetricsProvider)`.
Given the same inputs, the output is always the same. This is the basis for audit verification.

---

## Audit Chain — Witnessed Proof Transcript

Each `AuditEntry` contains `prev_hash: sha256(previous_entry_canonical_json)`.

`GateTransitionEvent` contains:
- `metrics_snapshot: HashMap<String, Value>` — state of X at transition time
- `metrics_hash: String` — deterministic hash of snapshot
- `criteria_results: Vec<CriteriaResult>` — per-criterion pass/fail with actual vs expected
- `state_rev: u64` — monotonic state version

Together these form a **tamper-evident proof transcript**:
`amp audit --verify` walks the chain and validates every hash link.
Signed checkpoints anchor the chain with ed25519 (JCS/RFC8785 canonicalization).

---

## Proven Properties

| Property | Statement | Evidence |
|----------|-----------|----------|
| Authority monotonicity | Adding a layer never increases permissions | `precedence.rs::allow_is_intersection` |
| Deny wins unconditionally | No mechanism overrides explicit deny | `precedence.rs::elevation_denied_action_not_granted` |
| Overlay deny preservation | Overlay can never weaken explicit deny | `precedence.rs::overlay_cannot_override_deny` |
| Overlay expansion | Promote overlay can increase autonomy | `precedence.rs::overlay_expands_autonomy` |
| Overlay deny additive | Overlay deny entries union with base deny | `precedence.rs::overlay_deny_additive` |
| Demote priority | Demote gate fires before promote when both pass | `evaluator.rs::demote_wins_over_promote` |
| Cooldown anti-Zeno | Gate cannot re-fire within cooldown_seconds | `evaluator.rs::cooldown_prevents_reevaluation` |
| Gate idempotency | Same inputs → same gate decision | `evaluator.rs::metrics_hash_is_deterministic` |
| Atomic state writes | State update is temp+fsync+rename, never partial | `state/atomic.rs` |
| Tamper detection | Modified audit chain fails verification | `audit_log.rs::verify_detects_tampering` |
| Trust decay | Accumulated violations trigger automatic demote | `evaluator.rs::trust_decay_auto_demotes` |

---

## Current Gaps — v1.1 Targets

### Gap 1 — `PhaseInvariant` (critical)

**Problem:** `Inv` in the hybrid automaton is undefined.
The system is event-driven: invariants are only checked at Gate evaluation ticks,
not continuously. An agent in phase `trusted` can violate promote conditions
between ticks without consequence.

**Required:**
```rust
pub struct PhaseInvariant {
    pub phase: String,
    pub must_hold: CriteriaLogic,   // requires Gap 2
    pub on_violation: InvariantViolation,
}

pub enum InvariantViolation {
    AutoDemote { to_phase: String },
    Alert,
    Block,
}
```

Engine: check `must_hold` every tick **before** Gate candidate evaluation.
Where it lives (core vs engine layer) — open question, requires ADR-008.

### Gap 2 — `CriteriaLogic`

**Problem:** `Gate.criteria: Vec<Criterion>` is implicit AND.
OR is not expressible. The Guard function is not compositional.

**Required:**
```rust
pub enum CriteriaLogic {
    All(Vec<Criterion>),   // AND — backward compatible
    Any(Vec<Criterion>),   // OR
    // Not and recursive nesting: defer until concrete use case
}
```

Backward compat: `Vec<Criterion>` deserializes as `CriteriaLogic::All(...)`.
Requires ADR-007 before implementation.

### Gap 3 — `GateApproval::Quorum`

**Problem:** `Quorum` exists in types but returns `error_quorum_not_supported`.
For safety-critical promote transitions, single-approver human gates are insufficient.

**Required:** N-of-M approval protocol with TTL and `ApprovalRecord`.
Requires ADR-009 before implementation.

### Gap 4 — zeroclaw ↔ ampersona seam (partially closed)

**Status:** `zeroclaw → ampersona-core` dependency exists behind `ampersona-gates` feature flag.
`SopAuditLogger` is wired to runtime tools (sop_approve, sop_advance, loop_, scheduler).
`SopMetricsCollector` implements `MetricsProvider` with warm-start and windowed metrics.

**Remaining:**
- `AuthorityEnforcer` not yet called in zeroclaw tool invocation path
- No end-to-end test: SOP step → `MetricSample` → Gate fires → PhaseState changes → audit verified
- See `ROADMAP.md` Phase 2

---

## What This Model Does Not Guarantee

**Reachability of X_unsafe is not proven.**
The platform enforces the authority lattice and the gate algebra.
It does not prevent a domain from supplying a `MetricsProvider` that returns
values which lead to unsafe states. Safety of the physical domain is the
responsibility of the domain implementor.

**Completeness of audit is not guaranteed under crash.**
Atomic writes (temp+fsync+rename) prevent partial state, but a crash between
state write and audit append produces a state transition with no audit record.
This is a known gap. Detection: `state_rev` mismatch between state file and audit chain.

**Quorum is not Byzantine-fault-tolerant.**
The planned quorum implementation assumes honest approvers.
Collusion or compromise of N approvers is not in the threat model.

**Overlay ordering is last-write-wins.**
Multiple rapid gate transitions will each overwrite the previous overlay.
There is no overlay stacking or accumulation. The effective authority at any
point is `apply_overlay(resolve_authority(layers), state.active_overlay)`.
If concurrent transitions are possible, the final overlay depends on which
gate fires last.
