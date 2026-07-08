#!/usr/bin/env bash
set -euo pipefail

# Tree Ring Memory - Agent Memory Stress Test
# Tests: init, remember, recall, forget, project-scoped recall
# Reports: failure points with context for debugging

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="${SCRIPT_DIR}/test-memory-$(date +%s)"
RESULTS_FILE="${TEST_DIR}/results.json"
PASS_COUNT=0
FAIL_COUNT=0
ERRORS=()

log() {
    echo "[$(date +%H:%M:%S)] $*"
}

fail() {
    local test_name="$1"
    local expected="$2"
    local actual="$3"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    ERRORS+=("{\"test\":\"${test_name}\",\"expected\":\"${expected}\",\"actual\":\"${actual}\"}")
    log "FAIL: ${test_name}"
}

pass() {
    local test_name="$1"
    PASS_COUNT=$((PASS_COUNT + 1))
    log "PASS: ${test_name}"
}

cleanup() {
    if [ -d "${TEST_DIR}" ]; then
        rm -rf "${TEST_DIR}"
    fi
}

trap cleanup EXIT

mkdir -p "${TEST_DIR}"
cd "${TEST_DIR}"

log "Starting Tree Ring Memory stress test"
log "Test directory: ${TEST_DIR}"

# Test 1: Basic initialization
log "Test 1: Basic initialization"
if tree-ring init 2>&1; then
    pass "init - basic initialization"
else
    fail "init - basic initialization" "exit code 0" "non-zero exit"
fi

# Test 2: Remember a simple fact
log "Test 2: Remember a simple fact"
if tree-ring remember "The sky is blue" 2>&1; then
    pass "remember - simple fact"
else
    fail "remember - simple fact" "success" "failure"
fi

# Test 3: Recall the remembered fact
log "Test 3: Recall the remembered fact"
RECALL_OUTPUT=$(tree-ring recall 2>&1)
if echo "${RECALL_OUTPUT}" | grep -q "sky is blue"; then
    pass "recall - simple fact"
else
    fail "recall - simple fact" "output containing 'sky is blue'" "${RECALL_OUTPUT}"
fi

# Test 4: Project-scoped recall (from issue description)
log "Test 4: Project-scoped recall"
if tree-ring remember "Use project-scoped recall before risky release changes." 2>&1; then
    pass "remember - project-scoped recall"
else
    fail "remember - project-scoped recall" "success" "failure"
fi

PROJECT_RECALL=$(tree-ring recall --project 2>&1)
if echo "${PROJECT_RECALL}" | grep -q "project-scoped recall"; then
    pass "recall --project - project-scoped recall"
else
    fail "recall --project - project-scoped recall" "output containing 'project-scoped recall'" "${PROJECT_RECALL}"
fi

# Test 5: Multiple memory storage
log "Test 5: Multiple memory storage"
for i in $(seq 1 10); do
    if ! tree-ring remember "Memory number ${i}" 2>&1; then
        fail "remember - memory ${i}" "success" "failure"
        break
    fi
done
pass "remember - multiple memories (1-10)"

# Test 6: Recall all memories
log "Test 6: Recall all memories"
ALL_RECALL=$(tree-ring recall --all 2>&1)
MEMORY_COUNT=$(echo "${ALL_RECALL}" | grep -c "Memory number" || true)
if [ "${MEMORY_COUNT}" -eq 10 ]; then
    pass "recall --all - all memories present"
else
    fail "recall --all - all memories present" "10 memories" "${MEMORY_COUNT} memories"
fi

# Test 7: Forget specific memory
log "Test 7: Forget specific memory"
if tree-ring forget "Memory number 5" 2>&1; then
    pass "forget - specific memory"
else
    fail "forget - specific memory" "success" "failure"
fi

# Test 8: Verify memory was forgotten
log "Test 8: Verify memory was forgotten"
AFTER_FORGET=$(tree-ring recall --all 2>&1)
if echo "${AFTER_FORGET}" | grep -q "Memory number 5"; then
    fail "verify forget - memory still present" "memory removed" "memory still exists"
else
    pass "verify forget - memory removed"
fi

# Test 9: Context window management
log "Test 9: Context window management"
LARGE_MEMORY=$(python3 -c "print('x' * 10000)")
if tree-ring remember "${LARGE_MEMORY}" 2>&1; then
    pass "remember - large memory (10KB)"
else
    fail "remember - large memory (10KB)" "success" "failure"
fi

# Test 10: Special characters
log "Test 10: Special characters"
SPECIAL_MEMORY="Test with special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?~"
if tree-ring remember "${SPECIAL_MEMORY}" 2>&1; then
    pass "remember - special characters"
else
    fail "remember - special characters" "success" "failure"
fi

# Test 11: Empty memory
log "Test 11: Empty memory"
if tree-ring remember "" 2>&1; then
    fail "remember - empty memory" "error" "success"
else
    pass "remember - empty memory rejected"
fi

# Test 12: Memory persistence (simulate restart)
log "Test 12: Memory persistence"
tree-ring init --force 2>&1 || true
PERSIST_RECALL=$(tree-ring recall --all 2>&1)
if echo "${PERSIST_RECALL}" | grep -q "sky is blue"; then
    pass "persistence - memory survived reinit"
else
    fail "persistence - memory survived reinit" "memory present" "memory lost"
fi

# Test 13: Concurrent memory operations
log "Test 13: Concurrent memory operations"
for i in $(seq 1 5); do
    tree-ring remember "Concurrent memory ${i}" &
done
wait
CONCURRENT_RECALL=$(tree-ring recall --all 2>&1)
CONCURRENT_COUNT=$(echo "${CONCURRENT_RECALL}" | grep -c "Concurrent memory" || true)
if [ "${CONCURRENT_COUNT}" -eq 5 ]; then
    pass "concurrent - all memories stored"
else
    fail "concurrent - all memories stored" "5 memories" "${CONCURRENT_COUNT} memories"
fi

# Test 14: Memory with timestamps
log "Test 14: Memory with timestamps"
TIMESTAMP_MEMORY="Event at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
if tree-ring remember "${TIMESTAMP_MEMORY}" 2>&1; then
    pass "remember - timestamped memory"
else
    fail "remember - timestamped memory" "success" "failure"
fi

# Test 15: Recall with context
log "Test 15: Recall with context"
CONTEXT_RECALL=$(tree-ring recall --context "testing" 2>&1)
if [ -n "${CONTEXT_RECALL}" ]; then
    pass "recall --context - returns results"
else
    fail "recall --context - returns results" "non-empty output" "empty output"
fi

# Generate results
log "Generating results..."
cat > "${RESULTS_FILE}" << EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "total_tests": $((PASS_COUNT + FAIL_COUNT)),
  "passed": ${PASS_COUNT},
  "failed": ${FAIL_COUNT},
  "errors": [
    $(IFS=,; echo "${ERRORS[*]}")
  ],
  "summary": "$([ ${FAIL_COUNT} -eq 0 ] && echo 'ALL TESTS PASSED' || echo "${FAIL_COUNT} TEST(S) FAILED")"
}
EOF

log "Results written to: ${RESULTS_FILE}"
log "Passed: ${PASS_COUNT}, Failed: ${FAIL_COUNT}"

if [ ${FAIL_COUNT} -gt 0 ]; then
    log "FAILURE POINTS:"
    for error in "${ERRORS[@]}"; do
        echo "  ${error}" | python3 -m json.tool 2>/dev/null || echo "  ${error}"
    done
    exit 1
else
    log "All tests passed successfully!"
    exit 0
fi