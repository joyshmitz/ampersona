# ampersona — Operations

## Key Rotation

### Planned Rotation

1. Generate new ed25519 keypair with new `key_id` (e.g., `k-2026-Q2`)
2. Re-sign all active personas with new key:
   ```bash
   amp fleet personas/ --check  # verify current state
   for f in personas/*.json; do
     amp sign "$f" --key new.key --key-id k-2026-Q2
   done
   ```
3. Update state files (they reference the signing key):
   ```bash
   # State files are re-signed automatically on next gate evaluation
   amp gate personas/agent.json --evaluate onboarding --metrics m.json
   ```
4. Distribute new public key to all consumers
5. After grace period, retire old key from verification trust store

### Emergency Rotation (Key Compromise)

1. Revoke compromised key immediately (remove from trust store)
2. Generate new keypair
3. Re-sign all personas and state files
4. Run `amp audit --verify` on all audit chains to ensure integrity
5. Review audit logs for unauthorized changes during compromise window
6. Document incident in audit log:
   ```bash
   # Override records serve as incident documentation
   amp gate agent.json --override onboarding --reason "Key compromise IR-XXX" --approver admin@co.com
   ```

## Backup and Restore

### What to Back Up

| Item | Priority | Location | Frequency |
|------|----------|----------|-----------|
| Persona files (`*.json`) | Critical | Project directory | On every change |
| State files (`*.state.json`) | High | Project directory | On every transition |
| Audit logs (`*.audit.jsonl`) | High | Project directory | Daily |
| Drift ledgers (`*.drift.jsonl`) | Medium | Project directory | Daily |
| Signing keys (private) | Critical | Secure key store | On rotation |
| Workspace defaults | Medium | `.ampersona/defaults.json` | On change |
| Integrity snapshots | High | `*.integrity.json` | On checkpoint |

### Backup Procedure

```bash
# Full backup
tar czf ampersona-backup-$(date +%Y%m%d).tar.gz \
  personas/*.json \
  personas/*.state.json \
  personas/*.audit.jsonl \
  personas/*.drift.jsonl \
  personas/*.integrity.json \
  .ampersona/

# Verify backup integrity
amp fleet personas/ --check --json > backup-check.json
amp audit personas/*.json --verify
```

### Restore Procedure

1. Extract backup archive
2. Verify all signatures: `amp fleet dir/ --check`
3. Verify all audit chains: `for f in dir/*.json; do amp audit "$f" --verify; done`
4. Verify state_rev continuity (no gaps or rollbacks)
5. Resume operations

### Recovery from Corruption

**Corrupted state file:**
```bash
# State can be reconstructed from audit log
# 1. Check audit chain integrity
amp audit agent.json --verify
# 2. Last valid state is in the audit log's most recent StateChange entry
# 3. Re-evaluate current gate from last known good state
```

**Corrupted audit chain:**
```bash
# 1. Find last valid checkpoint
amp audit agent.json --verify  # reports break point
# 2. Entries before the break are trusted (anchored by signed checkpoint)
# 3. Entries after the break must be manually reviewed
# 4. Create new checkpoint at current state
amp sign agent.state.json --key admin.key
```

## Incident Playbooks

### Playbook 1: Unauthorized State Transition

**Symptoms:** Agent is in a phase it shouldn't be (e.g., trusted without meeting criteria).

**Response:**
1. `amp status agent.json --json` — capture current state
2. `amp audit agent.json --verify` — check for chain tampering
3. If chain intact: review gate decision records for the transition
4. If chain broken: treat as compromise (see Emergency Rotation)
5. Override to correct phase:
   ```bash
   amp gate agent.json --override trust_decay --reason "Unauthorized transition IR-XXX" --approver admin@co.com
   ```

### Playbook 2: Elevation Stuck Active

**Symptoms:** Elevation should have expired but agent still has elevated permissions.

**Response:**
1. `amp status agent.json --json` — check elevation TTL
2. If TTL expired but still active: state file may not have been updated
3. Re-evaluate: `amp gate agent.json --evaluate <any-gate> --metrics m.json`
   (state evaluation always checks and expires TTLs)
4. If still stuck: manually deactivate in state file and re-sign

### Playbook 3: Audit Chain Verification Failure

**Symptoms:** `amp audit --verify` reports hash mismatch.

**Response:**
1. Note the entry number where the chain breaks
2. Compare entries before/after break point
3. If entries were appended out of order: may indicate concurrent writer bug
4. If entries were modified: treat as tampering (see Emergency Rotation)
5. After investigation, create new signed checkpoint to re-anchor the chain

### Playbook 4: Gate Flapping

**Symptoms:** Agent rapidly cycling between phases (promote → demote → promote).

**Response:**
1. `amp status agent.json --drift` — check metrics trend
2. Review cooldown settings on the involved gates
3. If cooldown too short: update gate definition with longer `cooldown_seconds`
4. If metrics genuinely oscillating: switch gate to `enforcement: "observe"` while investigating
5. Consider adding hysteresis (different thresholds for promote vs demote criteria)
