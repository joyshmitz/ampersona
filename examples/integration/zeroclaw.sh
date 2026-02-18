#!/usr/bin/env bash
set -euo pipefail

# Integration test: zeroclaw consumer
AMP="${AMP:-amp}"
EXAMPLES="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== zeroclaw integration ==="

# 1. Validate persona
echo "--- check --strict --json ---"
$AMP check "$EXAMPLES/zeroclaw_agent.json" --strict --json | jq -e .pass

# 2. Authority: read_file allowed
echo "--- authority: read_file ---"
$AMP authority "$EXAMPLES/zeroclaw_agent.json" --check read_file --json
echo "PASS: read_file allowed (exit $?)"

# 3. Shell injection blocked
echo "--- authority: shell injection ---"
if ! $AMP authority "$EXAMPLES/zeroclaw_agent.json" --check run_command \
  --context 'command=echo $(whoami)' --json 2>/dev/null; then
  echo "PASS: shell injection blocked (exit $?)"
fi

# 4. Git branch denial via context
echo "--- authority: git push main ---"
if ! $AMP authority "$EXAMPLES/zeroclaw_agent.json" --check git_push \
  --context git_operation=push --context branch=main --json 2>/dev/null; then
  echo "PASS: push to main denied (exit $?)"
fi

# 5. Gate evaluation
echo "--- gate: evaluate trusted ---"
$AMP gate "$EXAMPLES/zeroclaw_agent.json" --evaluate trusted \
  --metrics "$EXAMPLES/zeroclaw_metrics.json" --json | jq .gate_id

# 6. Import/Export roundtrip
echo "--- export: zeroclaw ---"
$AMP export --to zeroclaw "$EXAMPLES/zeroclaw_agent.json" | jq .security_policy

echo "=== all zeroclaw tests passed ==="
