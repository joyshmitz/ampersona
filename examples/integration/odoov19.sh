#!/usr/bin/env bash
set -euo pipefail

# Integration test: odoov19 consumer
AMP="${AMP:-amp}"
EXAMPLES="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== odoov19 integration ==="

# 1. Validate persona
echo "--- check --strict --json ---"
$AMP check "$EXAMPLES/odoov19_quality.json" --strict --json | jq -e .pass

# 2. Deny with compliance_ref
echo "--- authority: auto_approve_capa ---"
RESULT=$($AMP authority "$EXAMPLES/odoov19_quality.json" --check auto_approve_capa --json || true)
echo "$RESULT" | jq -e '.deny_entry.compliance_ref'
echo "PASS: auto_approve_capa denied with compliance ref"

# 3. Deny with compliance_ref (21 CFR Part 11)
echo "--- authority: delete_historical_data ---"
RESULT=$($AMP authority "$EXAMPLES/odoov19_quality.json" --check delete_historical_data --json || true)
echo "$RESULT" | jq -e '.deny_entry.compliance_ref'
echo "PASS: delete_historical_data denied with compliance ref"

# 4. F2 gate evaluation
echo "--- gate: f1_to_f2 ---"
$AMP gate "$EXAMPLES/odoov19_quality.json" --evaluate f1_to_f2 \
  --metrics "$EXAMPLES/odoov19_metrics_f2.json" --json | jq .gate_id

# 5. Audit verify
echo "--- audit verify ---"
$AMP audit "$EXAMPLES/odoov19_quality.json" --verify --json | jq .valid

echo "=== all odoov19 tests passed ==="
