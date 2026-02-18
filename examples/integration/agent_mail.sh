#!/usr/bin/env bash
set -euo pipefail

# Integration test: agent_mail consumer
AMP="${AMP:-amp}"
EXAMPLES="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== agent_mail integration ==="

# 1. Validate persona
echo "--- check --strict --json ---"
$AMP check "$EXAMPLES/agent_mail_worker.json" --strict --json | jq -e .pass

# 2. Register: MCP payload
echo "--- register --rpc ---"
$AMP register "$EXAMPLES/agent_mail_worker.json" \
  --project /data/projects/test --prompt --rpc \
  | jq -e '.params.arguments.name'

# 3. Authority: send_message allowed
echo "--- authority: send_message ---"
$AMP authority "$EXAMPLES/agent_mail_worker.json" --check send_message --json
echo "PASS: send_message allowed (exit $?)"

# 4. Status
echo "--- status ---"
$AMP status "$EXAMPLES/agent_mail_worker.json" --json | jq .name

echo "=== all agent_mail tests passed ==="
