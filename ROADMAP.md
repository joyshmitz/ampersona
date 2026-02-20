# ROADMAP.md — ampersona

> Living document. Updated when phase status changes.
> Principle: **fail in plan, plan to fail.**
> Every phase documents not only what to build — but how it breaks and how to recover.

---

## Current State

**v1.0 complete.** 55 features done, 129 tests green.
Platform is spec-correct, domain-agnostic, auditable.

**What v1.0 is not:**
- Not connected to any real domain
- Not integrated with zeroclaw runtime
- `PhaseInvariant` does not exist — `Inv` in the hybrid automaton is undefined
- `CriteriaLogic` is AND-only — OR is not expressible
- `GateApproval::Quorum` returns `error_quorum_not_supported`

The platform is a formally correct empty shell.
v1.1 closes the gaps. See `THEORY.md` for mathematical context.

---

## Phases

### Phase 0 — Synthetic Test Harness

**Goal:** Ability to test Gate evaluation and future PhaseInvariant
in complete isolation — no zeroclaw, no real hardware, no network.

**Steps:**
1. Extract `TestMetrics(HashMap<String, serde_json::Value>)` from evaluator test helpers
2. Move to `ampersona-core/src/testing.rs` under `#[cfg(test)]` or `cfg(feature = "testing")`
3. Add `SyntheticMetricsProvider` that returns deterministic sequences from a script:
   `vec![(metric, value, tick)]` — allows simulating Gate trigger scenarios

**Success:** Gate evaluation test no longer uses ad-hoc HashMap literals.
Full scenario (`phase: null → active → trusted → demote back`) exercisable in one test.

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| `testing` feature leaks into release binary | `cargo build --release` size increase; `strings amp \| grep TestMetrics` | Move to `#[cfg(test)]` only, remove feature flag |
| `SyntheticMetricsProvider` couples to engine internals | Test compiles only with engine as dependency | Trait must live in core, implementation in test helper — check dependency graph |
| Scenario scripts become maintenance burden | Tests fail on unrelated changes | Scripts are data, not logic — keep them in `tests/fixtures/` as JSON |

---

### Phase 1 — Core Gaps (ADRs first, code second)

**Dependency:** Phase 0 complete.

**Goal:** Close Gaps 1–3 from `THEORY.md` (PhaseInvariant, CriteriaLogic, Quorum).
After this phase, `Inv` is defined, Guard is compositional, Quorum has a protocol.
Gap 4 (zeroclaw seam) is addressed in Phase 2.

#### 1.1 — ADR-007 + `CriteriaLogic`

**Steps:**
1. Write ADR-007: `CriteriaLogic` — only `All` and `Any`, no `Not`, no recursion
2. Update `SPEC.md` Gate section
3. Update JSON Schema (additive, `serde(default)` preserves backward compat)
4. Implement type in `ampersona-core/src/spec/gates.rs`
5. Update engine evaluator
6. Update `amp migrate` to rewrite `criteria: [...]` → `criteria_logic: {"all": [...]}`
7. Tests: existing AND behavior preserved, new OR scenario, migration round-trip

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| `Not` requested mid-implementation | Scope creep, no concrete use case | Decline. Document as explicit non-goal in ADR-007. Reopen only with real use case. |
| Migration silently drops criteria | `backward_compat.rs` fails | Test migration on all `examples/*.json` before merging |
| Schema change breaks `amp check --strict` on existing personas | Conformance tests fail | `serde(default)` must cover all existing fields — verify with golden tests |
| Recursive nesting added "just in case" | Evaluator complexity grows, no tests for depth > 1 | Revert. `All`/`Any` is the ceiling until a concrete Gate requires more. |

#### 1.2 — ADR-008 + `PhaseInvariant`

**Steps:**
1. Write ADR-008: where `PhaseInvariant` lives, `InvariantViolation` semantics, evaluation order
2. Update `SPEC.md`
3. Type in `ampersona-core` (depends on `CriteriaLogic` from 1.1)
4. Engine: check invariant every tick **before** Gate candidates
5. Tests: violation → `AutoDemote`, violation → `Block`, no violation → normal Gate evaluation

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| Invariant check runs after Gate evaluation | Agent promotes then immediately violates invariant in same tick | Fix evaluation order: invariant check is always first. Test with tick-precise scenario. |
| `Block` mode deadlocks agent with no recovery path | Agent stuck, no manual override possible | `amp gate --override` must work even in `Block` mode — document in SPEC, test explicitly |
| `PhaseInvariant` placed in engine instead of core | Consumers cannot declare invariants without engine dependency | Invariant type must be in core. Evaluation logic in engine. Validate dependency graph. |
| ADR-008 left open too long | Code written before ADR accepted | Hard rule: zero code before ADR merged. If blocked on decision, document the blocker in ADR as open question. |

#### 1.3 — ADR-009 + `GateApproval::Quorum`

**Steps:**
1. Write ADR-009: N-of-M protocol, `ApprovalRecord`, TTL, state storage
2. Extend `PendingTransition` with `approvals: Vec<ApprovalRecord>`
3. Gate fires when `approvals.len() >= required`
4. TTL expiry resets pending state
5. Tests: quorum not met → pending, quorum met → transition, TTL expire → reset, duplicate approval idempotent

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| Approval replay attack (same approver submits twice) | `approvals.len()` exceeds required with single approver | Idempotency: `approver_id` must be unique in `approvals` vec. Test explicitly. |
| TTL expiry not enforced | Stale pending transition blocks new Gate evaluation | State load must check TTL at read time, not only at write time. |
| Quorum state lost on crash | `state_rev` mismatch between state file and audit | Crash between state write and audit is a known gap (documented in `THEORY.md`). Detection only, not prevention at this stage. |
| `required > of` silently accepted | Invalid config creates ungrantable Gate | Validate at `amp check` time, not at evaluation time. Schema constraint + test. |

**Phase 1 success criterion:**
`cargo test --workspace` green. All three gaps from `THEORY.md` have corresponding types and tests.
Zero changes to `AuthorityEnforcer`, `MetricsProvider`, `AuditSink` signatures — those are stable.

**Phase 1 failure mode (phase-level):**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| ADRs written but never accepted | Code accumulates without decisions | ADR not accepted = code not written. Blocked ADR gets a deadline or is closed as "won't do". |
| Phase 1 started before Phase 0 complete | Gate tests use ad-hoc fixtures, scenario coverage is blind | Revert to Phase 0. Synthetic harness first. No exceptions. |
| All three sub-phases in flight simultaneously | Merge conflicts in `gates.rs`, `spec/`, `SPEC.md` | Serial only: 1.1 merged before 1.2 starts, 1.2 merged before 1.3 starts. |

---

### Phase 2 — zeroclaw Integration

**Dependency:** Phase 1 complete, all tests green.

**Goal:** One zeroclaw agent changes PhaseState in ampersona through a real SOP execution.

**Partial progress (done):**
- `SopAuditLogger` wired to runtime tools (sop_approve, sop_advance, loop_, scheduler)
- `SopMetricsCollector` implements `MetricsProvider` behind `ampersona-gates` feature
- Warm-start from Memory backend, windowed metrics (_7d/_30d/_90d), push model

**Remaining steps:**
1. `AuthorityEnforcer` called before every Tool invocation in zeroclaw
2. End-to-end test: 5 SOP steps → `tasks_completed = 5` → Gate fires → phase changes → AuditSink writes → hash-chain verified

**Success criterion:**
Cross-repo integration test passes without mocking `MetricsProvider`.
Real SOP data produces real phase transition with real audit record.

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| zeroclaw SOP data format does not match `MetricQuery` schema | `MetricError::TypeMismatch` at runtime | Define adapter layer in zeroclaw, not in ampersona-core. Platform is not modified. |
| `AuthorityEnforcer` called on wrong granularity (per-message, not per-tool) | Deny fires for benign messages | Enforcement must be at Tool invocation boundary, not LLM message boundary. Document boundary explicitly in zeroclaw. |
| Integration test runs only in CI, not locally | Breaks silently on cross-repo changes | Test must be runnable with `cargo test --features integration` locally. Document setup. |
| zeroclaw phase changes ampersona state without audit | `state_rev` advances but `audit.jsonl` has no corresponding entry | `AuditSink` must be called in same transaction as state write. Test: verify chain length = state_rev after integration test. |

---

### Phase 3 — First Real Domain (e-claw)

**Dependency:** Phase 2 complete.

**Goal:** Real Modbus device → `MetricsProvider` → Gate fires → PhaseState changes.
First time the platform touches physical state.

**Steps:**
1. Minimal Modbus Tool in e-claw: one register read → one `MetricSample`
2. `MetricsProvider` implementation backed by real Modbus polling
3. First SOP with Gate criteria over real metrics
4. `PhaseInvariant` over real physical state (e.g. supply temperature out of safe range → `AutoDemote`)

**Success criterion:**
Physical register value change causes phase transition visible in `amp status`.

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| Modbus polling latency exceeds Gate evaluation tick | Stale metrics cause incorrect Gate decisions | `MetricSample` must include `timestamp`. Gate evaluator must reject samples older than configured `max_age_seconds`. Requires addition to `MetricQuery`. |
| Device offline → `MetricError::NotFound` → Gate blocked | Phase progression stops | `PhaseInvariant` with `on_violation: Alert` for connectivity loss. Gate evaluation must distinguish "no data" from "data out of range". |
| `PhaseInvariant` fires on sensor noise | Spurious demotions | Hysteresis in invariant criteria (same `cooldown_seconds` mechanism as Gate). Requires explicit threshold in `PhaseInvariant` spec. |
| e-claw abstractions accumulate before first device works | Code without end-to-end validation | Rule: minimal implementation first. One register, one metric, one Gate. No abstraction layers until first real transition is proven. |

---

### Phase 4 — Verification

**Dependency:** Phase 3 complete.

**Goal:** Formal claim about reachability. Not just tests — a proof or a bounded model check.

**Steps:**
1. Define concrete hybrid automaton in `THEORY.md`: Modbus domain, 3 phases, 5 Gates, exact threshold values
2. Identify `X_unsafe` (e.g. supply temperature > 95°C with agent in `full` autonomy phase)
3. Prove or model-check that `X_unsafe` is unreachable from `Init` given the invariants
4. Fill `DenyEntry::WithReason { compliance_ref }` with real standards: IEC 61511, NERC CIP
5. `amp check --strict` produces verifiable compliance report

**Failure modes:**

| What breaks | Signal | Recovery |
|-------------|--------|----------|
| Automaton defined abstractly, not with numbers | Proof is unfalsifiable | Concrete values required: threshold = 87.5, cooldown = 30s, phase = "trusted". No symbolic variables in the proof target. |
| Verification deferred indefinitely | "We'll do formal verification later" | Phase 4 is a hard gate for any safety-critical deployment. No production use of e-claw without it. |
| dReal or SpaceEx unavailable | Toolchain friction | Fallback: manual Lyapunov argument for simple invariant. Document explicitly as "manual proof, pending tool verification". Not equivalent but honest. |

---

## Non-Goals (permanent)

These will not be built regardless of requests:

- `CriteriaLogic::Not` or recursive nesting — until a concrete Gate requires it
- Byzantine-fault-tolerant quorum — out of threat model
- ampersona managing its own development agents via `.ampersona/` — semantic conflict with production use
- LLM wrapper or chat framework — see `VISION.md`

---

## Ultimate Success Criterion

> An external developer implements a new domain by implementing three traits —
> `MetricsProvider`, `AuthorityEnforcer`, `AuditSink` — without modifying
> a single line in `ampersona-core` or `ampersona-engine`.

Until this is demonstrated with a real external domain, the platform claim is unverified.
