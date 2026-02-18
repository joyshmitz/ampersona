# ampersona — Feature Parity Matrix

| # | Feature | Status | Phase | Notes |
|---|---------|--------|-------|-------|
| 1 | Identity spec (psychology, voice, capabilities, directives) | done | v0.2 | Carried forward |
| 2 | Authority spec (autonomy, scope, actions) | not_started | P1 | |
| 3 | Well-known scoped types (shell/git/file) | not_started | P1 | |
| 4 | Shell injection prevention (subshells, redirects, background) | not_started | P1 | |
| 5 | Elevations (temporal TTL) | not_started | P1+P2 | |
| 6 | Extensions namespace (ext) | not_started | P1 | |
| 7 | Action vocabulary + custom namespace | not_started | P1 | |
| 8 | Deny entries with compliance reasons | not_started | P1 | |
| 9 | Workspace defaults (.ampersona/) | not_started | P1+P2 | |
| 10 | Gates (promote+demote+metrics_schema) | not_started | P1 | |
| 11 | Gate enforcement modes (enforce/observe) | not_started | P1+P3 | |
| 12 | Gate deterministic priority + hysteresis | not_started | P1+P3 | |
| 13 | Signature spec (JCS, key_id, signed_fields) | not_started | P1+P4 | |
| 14 | Audit spec + event taxonomy | not_started | P1 | |
| 15 | State architecture (.state+.drift+.audit+.lock) | not_started | P1+P3 | |
| 16 | Authority precedence rules | not_started | P1+P2 | |
| 17 | Schema evolution ($schema, versioning) | not_started | P1 | |
| 18 | Traits (typed contracts + Result) | not_started | P1 | |
| 19 | Contract versioning (ampersona_contract) | not_started | P1 | |
| 20 | `amp validate` v1.0 | not_started | P1 | |
| 21 | `amp migrate` | not_started | P1 | |
| 22 | `amp check` (--json, --strict, action vocab) | not_started | P1 | |
| 23 | `amp init` (--workspace) | not_started | P1 | |
| 24 | DefaultPolicyChecker (layered gates) | not_started | P2 | |
| 25 | Precedence resolver | not_started | P2 | |
| 26 | Elevation logic (TTL, sliding window) | not_started | P2 | |
| 27 | `amp authority --check` | not_started | P2 | |
| 28 | `amp elevate` | not_started | P2 | |
| 29 | `amp import --from aieos` | not_started | P2 | |
| 30 | Property tests (precedence/merge) | not_started | P2 | |
| 31 | DefaultGateEvaluator (deterministic + hysteresis) | not_started | P3 | |
| 32 | Override mechanism (gate bypass + strong audit) | not_started | P3 | |
| 33 | Gate decision records (metrics snapshot) | not_started | P3 | |
| 34 | Atomic state (temp+fsync+rename+lock+state_rev) | not_started | P3 | |
| 35 | `amp gate --evaluate` | not_started | P3 | |
| 36 | `amp gate --override` | not_started | P3 | |
| 37 | `amp status` (--json, --drift) | not_started | P3 | |
| 38 | Hash-chain audit + signed checkpoints | not_started | P3 | |
| 39 | `amp audit --verify` | not_started | P3 | |
| 40 | Drift ledger (hash-chain) | not_started | P3 | |
| 41 | Trust decay | not_started | P3 | |
| 42 | Concurrency tests (lock+idempotency) | not_started | P3 | |
| 43 | JCS/RFC8785 canonicalization | not_started | P4 | |
| 44 | `amp sign/verify` (key_id, signed_fields) | not_started | P4 | |
| 45 | Signed integrity checkpoints | not_started | P4 | |
| 46 | `amp compose` | not_started | P4 | |
| 47 | `amp diff` | not_started | P4 | |
| 48 | `amp prompt` with authority/gates | not_started | P4 | |
| 49 | `amp export` (--to aieos/zeroclaw) | not_started | P4 | |
| 50 | `amp fleet` (--status/--check/--apply) | not_started | P4 | |
| 51 | Fuzz tests (parse/migrate/import) | not_started | P4 | |
| 52 | zeroclaw integration | not_started | P5 | |
| 53 | agent_mail integration | not_started | P5 | |
| 54 | odoov19 integration | not_started | P5 | |
| 55 | Consumer conformance tests | not_started | P5 | |
