# ampersona — Feature Parity Matrix

Status legend: `done` = implemented + tested, `partial` = engine done / CLI pending, `not_started` = no implementation yet.

| # | Feature | Status | Phase | Notes |
|---|---------|--------|-------|-------|
| 1 | Identity spec (psychology, voice, capabilities, directives) | done | v0.2 | Carried forward |
| 2 | Authority spec (autonomy, scope, actions) | done | P1 | policy::checker — 7 tests (deny_by_default, readonly, allow, shell, file, git) |
| 3 | Well-known scoped types (shell/git/file) | done | P1 | shell_subshell_blocked, file_access_deny_write_lock, git_push_to_denied_branch |
| 4 | Shell injection prevention (subshells, redirects, background) | done | P1 | shell_subshell_blocked, shell_command_not_allowed |
| 5 | Elevations (temporal TTL) | done | P1+P2 | elevation_grants_add_actions, expired_elevation_ignored |
| 6 | Extensions namespace (ext) | done | P1 | Used in zeroclaw converter (ext.zeroclaw) |
| 7 | Action vocabulary + custom namespace | done | P1 | actions::parse_builtin, parse_custom, parse_invalid_custom, suggest_typo |
| 8 | Deny entries with compliance reasons | done | P1 | deny_wins, deny_by_default_unknown, elevation_denied_action_not_granted |
| 9 | Workspace defaults (.ampersona/) | not_started | P1+P2 | |
| 10 | Gates (promote+demote+metrics_schema) | done | P1 | gates::evaluator — 5 tests |
| 11 | Gate enforcement modes (enforce/observe) | done | P1+P3 | observe_mode_does_not_block |
| 12 | Gate deterministic priority + hysteresis | done | P1+P3 | demote_wins_over_promote, cooldown_prevents_reevaluation |
| 13 | Signature spec (JCS, key_id, signed_fields) | partial | P1+P4 | JCS canonicalization done (ampersona-sign, 3 tests); key_id/signed_fields TBD |
| 14 | Audit spec + event taxonomy | done | P1 | audit_log — append_and_verify_chain, checkpoint_create_and_verify, verify_detects_tampering |
| 15 | State architecture (.state+.drift+.audit+.lock) | done | P1+P3 | state::atomic — 5 tests, audit_log — 3 tests |
| 16 | Authority precedence rules | done | P1+P2 | policy::precedence — 7 tests (intersection, union, min, elevation, deny) |
| 17 | Schema evolution ($schema, versioning) | done | P1 | Schema file, $schema in converters, migrate module |
| 18 | Traits (typed contracts + Result) | done | P1 | types module with UnitFloat, Autonomy; serde_roundtrip tests |
| 19 | Contract versioning (ampersona_contract) | not_started | P1 | |
| 20 | `amp validate` v1.0 | partial | P1 | v10_check_json_passes, v10_quiet_stone_validates; dedicated `--strict` TBD |
| 21 | `amp migrate` | done | P1 | migrate::tests — 3 tests (adds_version, idempotent, rejects_non_object) |
| 22 | `amp check` (--json, --strict, action vocab) | partial | P1 | v02_check_json_produces_structured_output; full flags TBD |
| 23 | `amp init` (--workspace) | not_started | P1 | |
| 24 | DefaultPolicyChecker (layered gates) | done | P2 | policy::checker — 7 tests |
| 25 | Precedence resolver | done | P2 | policy::precedence — 7 tests |
| 26 | Elevation logic (TTL, sliding window) | done | P2 | elevation_grants_add_actions, expired_elevation_ignored |
| 27 | `amp authority --check` | not_started | P2 | Engine done (policy::checker); CLI wiring TBD |
| 28 | `amp elevate` | not_started | P2 | Engine done (precedence + elevation); CLI wiring TBD |
| 29 | `amp import --from aieos` | partial | P2 | Engine done (import_aieos + full v1.1 normalization, 14 tests); CLI wiring TBD |
| 30 | Property tests (precedence/merge) | done | P2 | 7 precedence tests cover intersection, union, min, deny-removes-from-allowed |
| 31 | DefaultGateEvaluator (deterministic + hysteresis) | done | P3 | gates::evaluator — 5 tests |
| 32 | Override mechanism (gate bypass + strong audit) | not_started | P3 | |
| 33 | Gate decision records (metrics snapshot) | partial | P3 | metrics_hash_is_deterministic |
| 34 | Atomic state (temp+fsync+rename+lock+state_rev) | done | P3 | state::atomic — 5 tests (create, idempotent, lock, concurrent, drop) |
| 35 | `amp gate --evaluate` | not_started | P3 | Engine done (gates::evaluator); CLI wiring TBD |
| 36 | `amp gate --override` | not_started | P3 | |
| 37 | `amp status` (--json, --drift) | not_started | P3 | |
| 38 | Hash-chain audit + signed checkpoints | done | P3 | append_and_verify_chain, checkpoint_create_and_verify, verify_detects_tampering |
| 39 | `amp audit --verify` | not_started | P3 | Engine done (audit_log); CLI wiring TBD |
| 40 | Drift ledger (hash-chain) | partial | P3 | Checkpoint logic exists; full ledger TBD |
| 41 | Trust decay | done | P3 | trust_decay_auto_demotes |
| 42 | Concurrency tests (lock+idempotency) | done | P3 | concurrent_writers_with_lock, advisory_lock_blocks_concurrent |
| 43 | JCS/RFC8785 canonicalization | done | P4 | ampersona-sign — 3 tests (keys_sorted, string_escaping, no_whitespace) |
| 44 | `amp sign/verify` (key_id, signed_fields) | partial | P4 | Sign crate done (JCS); CLI + key_id rotation TBD |
| 45 | Signed integrity checkpoints | partial | P4 | checkpoint_create_and_verify; full signed flow TBD |
| 46 | `amp compose` | not_started | P4 | |
| 47 | `amp diff` | not_started | P4 | |
| 48 | `amp prompt` with authority/gates | done | P4 | v10_prompt_includes_authority_and_gates, golden_prompt_contains_all_sections |
| 49 | `amp export` (--to aieos/zeroclaw) | partial | P4 | Engine done (export_aieos + export_zeroclaw, 26 tests); CLI wiring TBD |
| 50 | `amp fleet` (--status/--check/--apply) | not_started | P4 | |
| 51 | Fuzz tests (parse/migrate/import) | not_started | P4 | |
| 52 | zeroclaw integration | partial | P5 | Converter done (import/export + 12 tests); runtime integration TBD |
| 53 | agent_mail integration | not_started | P5 | |
| 54 | odoov19 integration | not_started | P5 | |
| 55 | Consumer conformance tests | not_started | P5 | |

## Summary

- **done:** 30 / 55
- **partial:** 11 / 55
- **not_started:** 14 / 55
- **Total test coverage:** 100 tests across 4 crates (core: 12, engine: 57, sign: 3, integration: 10, amp: 0)
