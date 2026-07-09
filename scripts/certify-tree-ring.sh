#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT_DIR="$ROOT/target/tree-ring-certification"
TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/tree-ring-cert.XXXXXX")
BIN="$ROOT/target/release/tree-ring"
IMPORT_COUNT=${TREE_RING_CERT_IMPORT_COUNT:-10000}
EXTENDED=${TREE_RING_CERT_EXTENDED:-0}
AGENT_ZERO_ROOT=${TREE_RING_AGENT_ZERO_ROOT:-}

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

log() {
  printf '==> %s\n' "$*"
}

fail() {
  printf 'Tree Ring certification failed: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [ -f "$1" ] || fail "missing expected file: $1"
}

run() {
  log "$*"
  "$@"
}

run_logged() {
  log "$*"
  printf '==> %s\n' "$*" >> "$LOG"
  "$@" >> "$LOG" 2>&1 || fail "command failed: $*"
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

size_bytes() {
  wc -c < "$1" | tr -d ' '
}

size_kb_path() {
  du -sk "$1" | awk '{print $1}'
}

now_utc() {
  date -u '+%Y-%m-%dT%H:%M:%SZ'
}

generate_import_jsonl() {
  output=$1
  count=$2
  index=0
  : > "$output"
  while [ "$index" -lt "$count" ]; do
    id=$(printf 'mem_cert_%06d' "$index")
    if [ $((index % 50)) -eq 0 ]; then
      ring="scar"
      event_type="warning"
      summary="Avoid stale deployment rollback cache failure $index."
      tags='["deployment","cache"]'
    else
      ring="cambium"
      event_type="lesson"
      summary="Implementation note $index for subsystem $((index % 17))."
      tags='["implementation"]'
    fi
    printf '{"id":"%s","created_at":"2026-07-05T00:00:00+00:00","updated_at":"2026-07-05T00:00:00+00:00","project":"certification","agent_profile":null,"scope":"project","ring":"%s","event_type":"%s","summary":"%s","details":"","source":{"type":"manual","ref":"certification","quote":""},"tags":%s,"salience":0.5,"confidence":0.5,"sensitivity":"normal","retention":"normal","expires_at":null,"supersedes":[],"superseded_by":null,"links":[],"review":{"needs_review":false,"review_reason":null,"reviewed_at":null,"reviewed_by":null}}\n' \
      "$id" "$ring" "$event_type" "$summary" "$tags" >> "$output"
    index=$((index + 1))
  done
}

extract_metrics_json() {
  sed -n 's/^METRICS_JSON=//p' "$1" | tail -n 1
}

assert_recall_quality_evidence() {
  command -v python3 >/dev/null 2>&1 \
    || fail "python3 is required for recall quality evidence validation"
  python3 - "$1" "$2" <<'PY'
import json
import sys

report_path, index_path = sys.argv[1:3]
with open(report_path, encoding="utf-8") as handle:
    payload = json.load(handle)
with open(index_path, encoding="utf-8") as handle:
    index = json.load(handle)

report = payload.get("report") or {}
summary = report.get("summary") or {}
recall_quality = index.get("recall_quality") or {}
errors = []

if report.get("status") != "pass":
    errors.append(f"report.status={report.get('status')!r}")
if summary.get("fail_count") != 0:
    errors.append(f"summary.fail_count={summary.get('fail_count')!r}")
if summary.get("private_payloads_used") is not False:
    errors.append(
        f"summary.private_payloads_used={summary.get('private_payloads_used')!r}"
    )
if recall_quality.get("status") != "pass":
    errors.append(f"index.recall_quality.status={recall_quality.get('status')!r}")
if recall_quality.get("path") != "recall-quality/default-fixture-v1.json":
    errors.append(f"index.recall_quality.path={recall_quality.get('path')!r}")
if index.get("missing") != []:
    errors.append(f"index.missing={index.get('missing')!r}")

if errors:
    print("; ".join(errors), file=sys.stderr)
    sys.exit(1)
PY
}

mkdir -p "$OUT_DIR"
SUMMARY="$OUT_DIR/summary.md"
METRICS="$OUT_DIR/metrics.json"
INDEX="$OUT_DIR/evidence-index.json"
LOG="$OUT_DIR/certification.log"
: > "$LOG"

log "certification output: $OUT_DIR" | tee -a "$LOG"

run_logged cargo fmt --check
run_logged cargo test --locked
run_logged cargo clippy --locked --all-targets
run_logged cargo build --release --locked

require_file "$BIN"
binary_bytes=$(size_bytes "$BIN")
[ "$binary_bytes" -le 8388608 ] || fail "release binary exceeds 8 MB: $binary_bytes bytes"

project_root="$TMP_DIR/project-install"
mkdir -p "$project_root"
(cd "$project_root" && sh "$ROOT/install.sh" --project --init --source "$ROOT" --no-animation) \
  > "$OUT_DIR/project-install.out" 2> "$OUT_DIR/project-install.err"
project_install_kb=$(size_kb_path "$project_root/.tree-ring")
[ "$project_install_kb" -le 8192 ] || fail "project install exceeds 8 MB: ${project_install_kb}KB"
"$project_root/.tree-ring/bin/tree-ring" --root "$project_root/.tree-ring" --json recall "startup warnings" \
  > "$OUT_DIR/project-install-recall.json"

global_root="$TMP_DIR/global-install"
mkdir -p "$global_root/home"
HOME="$global_root/home" SHELL=/bin/zsh sh "$ROOT/install.sh" --global --source "$ROOT" --no-onboarding \
  > "$OUT_DIR/global-install.out" 2> "$OUT_DIR/global-install.err"
global_install_kb=$(size_kb_path "$global_root/home/.local")
"$global_root/home/.local/bin/tree-ring" --version > "$OUT_DIR/global-version.out"
grep -F 'Tree Ring Memory PATH' "$global_root/home/.zshrc" > /dev/null \
  || fail "global install did not write PATH block"

smoke_root="$TMP_DIR/cli-smoke/.tree-ring"
"$BIN" --root "$smoke_root" --json init > "$OUT_DIR/cli-init.json"
"$BIN" --root "$smoke_root" --json remember \
  "Use project-scoped recall before changing release behavior." \
  --event-type lesson --scope project --project certification --tag release \
  > "$OUT_DIR/cli-remember.json"
"$BIN" --root "$smoke_root" --json evidence \
  "Recall found release workflow guardrail in certification." \
  --outcome promoted --evidence-ref certification/harness --project certification --score 0.91 \
  > "$OUT_DIR/cli-evidence.json"
"$BIN" --root "$smoke_root" --json recall "release workflow guardrail" --project certification \
  > "$OUT_DIR/cli-recall.json"
"$BIN" --root "$smoke_root" --json audit --audit-type all > "$OUT_DIR/cli-audit.json"
grep -F 'release workflow guardrail' "$OUT_DIR/cli-recall.json" > /dev/null \
  || fail "CLI recall smoke did not return expected memory"

adapter_root="$TMP_DIR/adapters"
mkdir -p "$adapter_root/sub" "$adapter_root/revolve"
printf '# Project Contract\n\n## Safety\nAlways run tests before shipping.\n' > "$adapter_root/AGENTS.md"
printf '# Nested Contract\n\n## Runtime\nUse local-first memory only.\n' > "$adapter_root/sub/AGENTS.md"
printf '# Eval\n\nOutcome: promoted\nEvidence: tests/run-42\nSummary: Recall bridge worked in certification.\n' \
  > "$adapter_root/revolve/result.md"
adapter_memory_root="$adapter_root/.tree-ring"
"$BIN" --root "$adapter_memory_root" --json dox sync --source-root "$adapter_root" --dry-run \
  > "$OUT_DIR/dox-dry-run.json"
"$BIN" --root "$adapter_memory_root" --json dox sync --source-root "$adapter_root" \
  > "$OUT_DIR/dox-write.json"
"$BIN" --root "$adapter_memory_root" --json recall "run tests" > "$OUT_DIR/dox-recall.json"
"$BIN" --root "$adapter_memory_root" --json revolve sync --source-root "$adapter_root/revolve" --dry-run \
  > "$OUT_DIR/revolve-dry-run.json"
"$BIN" --root "$adapter_memory_root" --json revolve sync --source-root "$adapter_root/revolve" \
  > "$OUT_DIR/revolve-write.json"
"$BIN" --root "$adapter_memory_root" --json recall "promoted evidence" > "$OUT_DIR/revolve-recall.json"
grep -F 'Always run tests' "$OUT_DIR/dox-recall.json" > /dev/null \
  || fail "DOX recall smoke did not return expected memory"
grep -F 'promoted evidence' "$OUT_DIR/revolve-recall.json" > /dev/null \
  || fail "Revolve recall smoke did not return expected memory"

scan_root="$TMP_DIR/integration-scan"
scan_home="$TMP_DIR/integration-home"
mkdir -p "$scan_root/.codex" "$scan_root/.claude" "$scan_root/usr/plugins" \
  "$scan_root/revolve" "$scan_root/.opencode" "$scan_root/.goose" "$scan_home/.claude" \
  "$scan_home/.pi"
printf '# Agent contract\n' > "$scan_root/AGENTS.md"
printf '# Claude instructions\n' > "$scan_root/CLAUDE.md"
mkdir -p "$scan_root/.tree-ring"
cat > "$scan_root/.tree-ring/SKILL.md" <<'EOF'
Use `tree-ring recall` before acting on project assumptions.
Use `tree-ring remember` only for durable, non-secret project facts.
EOF
cat > "$scan_root/.tree-ring/CLI.md" <<'EOF'
The portable command surface is `tree-ring recall` and `tree-ring remember`.
EOF
cat > "$scan_root/.tree-ring/AGENTS.md" <<'EOF'
Project harnesses should reference SKILL.md and CLI.md for Tree Ring Memory.
EOF
HOME="$scan_home" "$BIN" --json integrations scan --source-root "$scan_root" \
  > "$OUT_DIR/integrations-scan.json"
grep -F '"origin":"project"' "$OUT_DIR/integrations-scan.json" > /dev/null \
  || fail "integration scan did not include project-origin markers"
grep -F '"origin":"home"' "$OUT_DIR/integrations-scan.json" > /dev/null \
  || fail "integration scan did not include home-origin markers"

import_root="$TMP_DIR/import-bench/.tree-ring"
import_jsonl="$TMP_DIR/import-bench.jsonl"
generate_import_jsonl "$import_jsonl" "$IMPORT_COUNT"
"$BIN" --root "$import_root" --json init > /dev/null
import_start=$(date +%s)
"$BIN" --root "$import_root" --json import "$import_jsonl" > "$OUT_DIR/import-bench.json"
import_end=$(date +%s)
import_seconds=$((import_end - import_start))
[ "$import_seconds" -le 0 ] && import_seconds=1
import_rate=$((IMPORT_COUNT / import_seconds))
[ "$import_rate" -ge 1500 ] || fail "import throughput below 1500 events/s: $import_rate"

perf_10k_out="$OUT_DIR/performance-10000.out"
perf_30k_out="$OUT_DIR/performance-30000.out"
run cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 10000 \
  > "$perf_10k_out" 2>&1
run cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 30000 \
  > "$perf_30k_out" 2>&1
perf_10k_json=$(extract_metrics_json "$perf_10k_out")
perf_30k_json=$(extract_metrics_json "$perf_30k_out")
[ -n "$perf_10k_json" ] || fail "missing 10k performance metrics"
[ -n "$perf_30k_json" ] || fail "missing 30k performance metrics"

perf_50k_json=null
if [ "$EXTENDED" = "1" ]; then
  perf_50k_out="$OUT_DIR/performance-50000.out"
  run cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 50000 \
    > "$perf_50k_out" 2>&1
  perf_50k_json=$(extract_metrics_json "$perf_50k_out")
  [ -n "$perf_50k_json" ] || fail "missing 50k performance metrics"
fi

agent_zero_status='"skipped"'
agent_zero_note='"TREE_RING_AGENT_ZERO_ROOT not set"'
if [ -n "$AGENT_ZERO_ROOT" ]; then
  if [ ! -d "$AGENT_ZERO_ROOT/usr/plugins/tree_ring_memory" ]; then
    fail "Agent Zero plugin checkout not found: $AGENT_ZERO_ROOT"
  fi
  if command -v node >/dev/null 2>&1; then
    node --check "$AGENT_ZERO_ROOT/usr/plugins/tree_ring_memory/webui/memory-store.js" \
      > "$OUT_DIR/agent-zero-node-check.out" 2>&1
  else
    fail "node is required for Agent Zero plugin check"
  fi
  if command -v python3 >/dev/null 2>&1; then
    (cd "$AGENT_ZERO_ROOT" && PYTHONPATH="$AGENT_ZERO_ROOT" pytest -q -p no:cacheprovider usr/plugins/tree_ring_memory/tests) \
      > "$OUT_DIR/agent-zero-pytest.out" 2>&1
    az_data="$TMP_DIR/agent-zero-data"
    (cd "$AGENT_ZERO_ROOT" && TREE_RING_MEMORY_DATA_DIR="$az_data" PYTHONPATH="$AGENT_ZERO_ROOT" python3 usr/plugins/tree_ring_memory/execute.py status) \
      > "$OUT_DIR/agent-zero-status.json"
    (cd "$AGENT_ZERO_ROOT" && TREE_RING_MEMORY_DATA_DIR="$az_data" PYTHONPATH="$AGENT_ZERO_ROOT" python3 usr/plugins/tree_ring_memory/execute.py audit) \
      > "$OUT_DIR/agent-zero-audit.json"
    (cd "$AGENT_ZERO_ROOT" && TREE_RING_MEMORY_DATA_DIR="$az_data" PYTHONPATH="$AGENT_ZERO_ROOT" python3 usr/plugins/tree_ring_memory/execute.py export) \
      > "$OUT_DIR/agent-zero-export.json"
  else
    fail "python3 is required for Agent Zero plugin check"
  fi
  agent_zero_status='"passed"'
  agent_zero_note='"Agent Zero plugin smoke passed"'
fi

created_at=$(now_utc)
cat > "$METRICS" <<EOF
{
  "ok": true,
  "created_at": "$created_at",
  "release_binary_bytes": $binary_bytes,
  "project_install_kb": $project_install_kb,
  "global_install_kb": $global_install_kb,
  "cli_import": {
    "memory_count": $IMPORT_COUNT,
    "seconds": $import_seconds,
    "events_per_second": $import_rate
  },
  "performance": {
    "records_10000": $perf_10k_json,
    "records_30000": $perf_30k_json,
    "records_50000": $perf_50k_json
  },
  "agent_zero": {
    "status": $agent_zero_status,
    "note": $agent_zero_note
  }
}
EOF

cat > "$SUMMARY" <<EOF
# Tree Ring Certification Summary

Generated: $created_at

- release binary: $binary_bytes bytes
- project install: ${project_install_kb}KB
- global install: ${global_install_kb}KB
- CLI import: $IMPORT_COUNT memories in ${import_seconds}s (${import_rate}/s)
- 10k performance metrics: recorded in \`performance-10000.out\`
- 30k performance metrics: recorded in \`performance-30000.out\`
- 50k extended metrics: $([ "$EXTENDED" = "1" ] && printf 'recorded in `performance-50000.out`' || printf 'skipped')
- Agent Zero plugin smoke: $(printf '%s' "$agent_zero_status" | tr -d '"')

Machine-readable metrics: \`metrics.json\`
EOF

cat > "$INDEX" <<EOF
{
  "generated_at": "$created_at",
  "overall_status": "pass",
  "certification": {
    "category": "certification",
    "status": "pass",
    "label": "Local certification",
    "path": "metrics.json",
    "summary_path": "summary.md",
    "generated_at": "$created_at"
  },
  "harness": {},
  "recall_quality": null,
  "missing": ["harness", "recall_quality"],
  "stale": []
}
EOF

HOME="$scan_home" "$BIN" --json integrations certify --source-root "$scan_root" --out-dir "$OUT_DIR" \
  > "$OUT_DIR/harness-certification.json"
require_file "$OUT_DIR/harness/codex.json"
require_file "$OUT_DIR/harness/claude-code.json"
require_file "$OUT_DIR/harness/opencode.json"
require_file "$OUT_DIR/harness/goose.json"
require_file "$OUT_DIR/harness/pi.json"
require_file "$OUT_DIR/harness/agent-zero.json"
grep -E '"pass_count"[[:space:]]*:[[:space:]]*5' "$OUT_DIR/harness-certification.json" > /dev/null \
  || fail "harness certification did not report pass_count 5"
grep -E '"fail_count"[[:space:]]*:[[:space:]]*0' "$OUT_DIR/harness-certification.json" > /dev/null \
  || fail "harness certification did not report fail_count 0"
grep -E '"skip_count"[[:space:]]*:[[:space:]]*1' "$OUT_DIR/harness-certification.json" > /dev/null \
  || fail "harness certification did not report skip_count 1"
grep -E '"status"[[:space:]]*:[[:space:]]*"pass"' "$OUT_DIR/harness/codex.json" > /dev/null \
  || fail "codex harness did not report pass status"
grep -E '"status"[[:space:]]*:[[:space:]]*"skip"' "$OUT_DIR/harness/pi.json" > /dev/null \
  || fail "pi harness did not report skip status"
grep -F '"harness"' "$INDEX" > /dev/null \
  || fail "evidence index did not include harness records"
grep -F '"codex"' "$INDEX" > /dev/null \
  || fail "evidence index did not include Codex harness record"
"$BIN" --json recall-quality --source-root "$scan_root" --out-dir "$OUT_DIR" \
  > "$OUT_DIR/recall-quality.json"
require_file "$OUT_DIR/recall-quality/default-fixture-v1.json"
assert_recall_quality_evidence "$OUT_DIR/recall-quality.json" "$INDEX" \
  || fail "recall quality evidence validation failed"

log "certification passed"
printf 'Summary: %s\n' "$SUMMARY"
printf 'Metrics: %s\n' "$METRICS"
printf 'Evidence index: %s\n' "$INDEX"
printf 'Harness evidence: %s\n' "$OUT_DIR/harness"
printf 'Recall quality evidence: %s\n' "$OUT_DIR/recall-quality/default-fixture-v1.json"
