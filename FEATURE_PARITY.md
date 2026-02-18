# ampersona — Feature Parity Matrix

Status legend: `done` = implemented + tested, `partial` = implemented but incomplete or untested, `not_started` = no implementation yet.

| # | Feature | Status | Phase | Notes |
|---|---------|--------|-------|-------|
| 1 | Identity spec (psychology, voice, capabilities, directives) | done | v0.2 | Carried forward |
| 2 | Authority spec (autonomy, scope, actions) | done | P1 | policy::checker — 8 tests (deny_by_default, readonly, allow, shell, file, git) |
| 3 | Well-known scoped types (shell/git/file) | done | P1 | shell_subshell_blocked, file_access_deny_write_lock, git_push_to_denied_branch |
| 4 | Shell injection prevention (subshells, redirects, background) | done | P1 | shell_subshell_blocked, shell_command_not_allowed |
| 5 | Elevations (temporal TTL) | done | P1+P2 | elevation_grants_add_actions, expired_elevation_ignored |
| 6 | Extensions namespace (ext) | done | P1 | Used in zeroclaw converter (ext.zeroclaw) |
| 7 | Action vocabulary + custom namespace | done | P1 | actions::parse_builtin, parse_custom, parse_invalid_custom, suggest_typo |
| 8 | Deny entries with compliance reasons | done | P1 | deny_wins, deny_by_default_unknown, elevation_denied_action_not_granted |
| 9 | Workspace defaults (.ampersona/) | partial | P1+P2 | load_workspace_defaults() exists + used in cmd_authority; `amp init --workspace` scaffolds file |
| 10 | Gates (promote+demote+metrics_schema) | done | P1 | gates::evaluator — 6 tests |
| 11 | Gate enforcement modes (enforce/observe) | done | P1+P3 | observe_mode_does_not_block |
| 12 | Gate deterministic priority + hysteresis | done | P1+P3 | demote_wins_over_promote, cooldown_prevents_reevaluation |
| 13 | Signature spec (JCS, key_id, signed_fields) | done | P1+P4 | ampersona-sign: canonical.rs (3 tests), sign.rs (key_id + signed_fields), verify.rs (canon + key_id match) |
| 14 | Audit spec + event taxonomy | done | P1 | audit_log — append_and_verify_chain, checkpoint_create_and_verify, verify_detects_tampering |
| 15 | State architecture (.state+.drift+.audit+.lock) | done | P1+P3 | state::atomic — 6 tests, audit_log — 3 tests |
| 16 | Authority precedence rules | done | P1+P2 | policy::precedence — 8 tests (intersection, union, min, elevation, deny) |
| 17 | Schema evolution ($schema, versioning) | done | P1 | Schema file, $schema in converters, migrate module |
| 18 | Traits (typed contracts + Result) | done | P1 | AuthorityEnforcer, MetricsProvider, AuditSink traits; UnitFloat, Autonomy types |
| 19 | Contract versioning (ampersona_contract) | not_started | P5 | Spec defined; runtime check deferred to consumer integration |
| 20 | `amp validate` v1.0 | done | P1 | v10_quiet_stone_validates, v10_check_json_passes; auto-detect v0.2/v1.0 |
| 21 | `amp migrate` | done | P1 | migrate::tests — 4 tests (adds_version, idempotent, rejects_non_object, handles_empty) |
| 22 | `amp check` (--json, --strict, action vocab) | done | P1 | v02_check_json_produces_structured_output; --json + --strict + action_vocab + signature + lint |
| 23 | `amp init` (--workspace) | done | P1 | Scaffolds persona.json or .ampersona/defaults.json |
| 24 | DefaultPolicyChecker (layered gates) | done | P2 | policy::checker — 8 tests |
| 25 | Precedence resolver | done | P2 | policy::precedence — 8 tests |
| 26 | Elevation logic (TTL, sliding window) | done | P2 | elevation_grants_add_actions, expired_elevation_ignored |
| 27 | `amp authority --check` | done | P2 | cmd_authority with workspace defaults + persona + gate overlay + elevation precedence |
| 28 | `amp elevate` | done | P2 | cmd_elevate with TTL, atomic state write |
| 29 | `amp import --from aieos` | done | P2 | import_aieos with full v1.1 normalization (14 tests) + CLI wired |
| 30 | Property tests (precedence/merge) | done | P2 | 8 precedence tests cover intersection, union, min, deny-removes-from-allowed |
| 31 | DefaultGateEvaluator (deterministic + hysteresis) | done | P3 | gates::evaluator — 6 tests |
| 32 | Override mechanism (gate bypass + strong audit) | done | P3 | override_gate.rs + cmd_gate --override with reason + approver |
| 33 | Gate decision records (metrics snapshot) | done | P3 | GateDecisionRecord with metrics_snapshot, criteria_results, metrics_hash |
| 34 | Atomic state (temp+fsync+rename+lock+state_rev) | done | P3 | state::atomic — 6 tests (create, idempotent, lock, concurrent, drop) |
| 35 | `amp gate --evaluate` | done | P3 | cmd_gate --evaluate --metrics; writes state + drift + decision record |
| 36 | `amp gate --override` | done | P3 | cmd_gate --override --reason --approver; is_override=true in audit |
| 37 | `amp status` (--json, --drift) | done | P3 | Phase, autonomy, elevations, last events; --drift shows trend |
| 38 | Hash-chain audit + signed checkpoints | done | P3 | append_and_verify_chain, checkpoint_create_and_verify, verify_detects_tampering |
| 39 | `amp audit --verify` | done | P3 | cmd_audit --verify; hash-chain validation |
| 40 | Drift ledger (hash-chain) | done | P3 | append_drift + read_drift_entries + verify_drift_chain |
| 41 | Trust decay | done | P3 | trust_decay_auto_demotes |
| 42 | Concurrency tests (lock+idempotency) | done | P3 | concurrent_writers_with_lock, advisory_lock_blocks_concurrent, advisory_lock_drop_releases |
| 43 | JCS/RFC8785 canonicalization | done | P4 | ampersona-sign — 3 tests (keys_sorted, string_escaping, no_whitespace) |
| 44 | `amp sign/verify` (key_id, signed_fields) | done | P4 | cmd_sign --key --key-id; cmd_verify --pubkey; JCS + ed25519 |
| 45 | Signed integrity checkpoints | done | P4 | create_checkpoint + verify_checkpoint in audit_log |
| 46 | `amp compose` | done | P4 | cmd_compose; merge_personas with deny=union, allow=intersection, limits=min |
| 47 | `amp diff` | done | P4 | cmd_diff; JSON key-by-key comparison |
| 48 | `amp prompt` with authority/gates | done | P4 | v10_prompt_includes_authority_and_gates, golden_prompt_contains_all_sections |
| 49 | `amp export` (--to aieos/zeroclaw) | done | P4 | export_aieos + export_zeroclaw (26 tests) + CLI wired |
| 50 | `amp fleet` (--status/--check/--apply) | done | P4 | cmd_fleet --status/--check/--json/--apply-overlay |
| 51 | Fuzz tests (parse/migrate/import) | not_started | P4 | |
| 52 | zeroclaw integration | partial | P5 | Converter done (import/export + 12 tests); runtime integration TBD |
| 53 | agent_mail integration | not_started | P5 | |
| 54 | odoov19 integration | not_started | P5 | |
| 55 | Consumer conformance tests | not_started | P5 | |

## Summary

- **done:** 49 / 55
- **partial:** 2 / 55 (workspace defaults, zeroclaw runtime)
- **not_started:** 4 / 55 (contract versioning, fuzz tests, agent_mail, odoov19, conformance)
- **Total tests:** 82 across 4 crates (core: 12, engine: 57, sign: 3, amp integration: 10)
- **Phases 0a–4 complete.** Phase 5 (consumer integration) pending.
