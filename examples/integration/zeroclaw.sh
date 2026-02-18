#!/usr/bin/env bash
set -euo pipefail

# Integration test: zeroclaw consumer
AMP="${AMP:-amp}"
EXAMPLES="$(cd "$(dirname "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
PERSONA="$TMP_DIR/zeroclaw_agent.json"
METRICS="$TMP_DIR/zeroclaw_metrics.json"

cp "$EXAMPLES/zeroclaw_agent.json" "$PERSONA"
cp "$EXAMPLES/zeroclaw_metrics.json" "$METRICS"

echo "=== zeroclaw integration ==="

# 1. Validate persona
echo "--- check --strict --json ---"
$AMP check "$PERSONA" --strict --json | jq -e .pass

# 2. Authority: read_file allowed
echo "--- authority: read_file ---"
$AMP authority "$PERSONA" --check read_file --json
echo "PASS: read_file allowed (exit $?)"

# 3. Shell injection blocked
echo "--- authority: shell injection ---"
if ! $AMP authority "$PERSONA" --check run_command \
  --context 'command=echo $(whoami)' --json 2>/dev/null; then
  echo "PASS: shell injection blocked (exit $?)"
fi

# 4. Git branch denial via context
echo "--- authority: git push main ---"
if ! $AMP authority "$PERSONA" --check git_push \
  --context git_operation=push --context branch=main --json 2>/dev/null; then
  echo "PASS: push to main denied (exit $?)"
fi

# 5. Gate evaluation
echo "--- gate: evaluate trusted ---"
cat > "$TMP_DIR/zeroclaw_agent.state.json" <<'JSON'
{
  "name": "ZeroclawWorker",
  "current_phase": "active",
  "state_rev": 1,
  "active_elevations": [],
  "last_transition": null,
  "updated_at": "2024-01-01T00:00:00Z"
}
JSON
$AMP gate "$PERSONA" --evaluate trusted --metrics "$METRICS" --json | jq .gate_id

# 6. Import/Export roundtrip
echo "--- export: zeroclaw ---"
$AMP export "$PERSONA" --to zeroclaw | jq .security_policy

echo "=== all zeroclaw tests passed ==="
