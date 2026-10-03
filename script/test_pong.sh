#!/usr/bin/env bash

#
# PONG - manual functional and error test suite
#
# This script performs a clean build of PONG followed by functional,
# command-line, network, sweep, route, JSON, and expected-error tests.
#
# Run from anywhere inside the PONG project:
#
#     ./scripts/test_PONG.sh
#
#
# ------------------------------------------------------------
# Local Test Configuration
# ------------------------------------------------------------
#
# Network-specific test values are stored in:
#
#     scripts/test.txt
#
# This file is intentionally excluded from Git using .gitignore so
# local IP addresses and network configuration are not published to
# the GitHub repository.
#
# Create the file:
#
#     vi scripts/test.txt
#
# Add the following variables and change the example values to match
# the network where PONG is being tested:
#
#     INTERFACE="eth0"
#
#     LOCAL_TARGET="192.168.254.10"
#     GATEWAY_TARGET="192.168.254.1"
#     DNS_TARGET="google.com"
#
#     SWEEP_LOW="192.168.254.10"
#     SWEEP_HIGH="192.168.254.50"
#
# INTERFACE
#     Network interface used for broadcast and interface sweep tests.
#
# LOCAL_TARGET
#     Known reachable host used for basic positive ping testing.
#
# GATEWAY_TARGET
#     Known reachable router/gateway used for ping, route, JSON,
#     packet-count, timeout, interval, and payload tests.
#
# DNS_TARGET
#     Known resolvable hostname used to test DNS resolution.
#
# SWEEP_LOW / SWEEP_HIGH
#     Small known address range used for explicit sweep testing.
#     Keep this range reasonably small so the test suite completes
#     quickly.
#
# Verify that Git is ignoring the configuration file with:
#
#     git check-ignore -v scripts/test.txt
#
# The test suite will stop immediately with an error if test.txt is
# missing or any required configuration variable is undefined.
#

set -u
set -o pipefail

# ------------------------------------------------------------
# Configuration
# ------------------------------------------------------------

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PONG="${PROJECT_ROOT}/target/debug/pong"
TEST_CONFIG="${PROJECT_ROOT}/script/test.txt"

#
# Network-specific test values are intentionally stored outside
# the tracked test script so private/local network information is
# not committed to GitHub.
#

if [[ ! -f "${TEST_CONFIG}" ]]; then
    printf '\033[1;31m[ERROR]\033[0m Test configuration not found:\n'
    printf '        %s\n\n' "${TEST_CONFIG}"
    printf 'Create scripts/test.txt before running this test suite.\n'
    printf 'See the comments at the beginning of this script for an example.\n'
    exit 1
fi

# shellcheck source=/dev/null
source "${TEST_CONFIG}"

#
# Verify that all required configuration variables exist.
#

REQUIRED_CONFIG=(
    INTERFACE
    LOCAL_TARGET
    GATEWAY_TARGET
    DNS_TARGET
    SWEEP_LOW
    SWEEP_HIGH
)

for var in "${REQUIRED_CONFIG[@]}"; do
    if [[ -z "${!var:-}" ]]; then
        printf '\033[1;31m[ERROR]\033[0m Required configuration variable is missing: %s\n' "${var}"
        exit 1
    fi
done

PASS=0
FAIL=0
EXPECTED_FAIL_PASS=0
EXPECTED_FAIL_FAIL=0

cd "${PROJECT_ROOT}" || exit 1

# ------------------------------------------------------------
# Display helpers
# ------------------------------------------------------------

heading()
{
    printf '\n\033[1;35m============================================================\033[0m\n'
    printf '\033[1;35m%s\033[0m\n' "$1"
    printf '\033[1;35m============================================================\033[0m\n\n'
}

info()
{
    printf '\033[1;36m[INFO]\033[0m %s\n' "$1"
}

pass()
{
    printf '\033[1;32m[PASS]\033[0m %s\n' "$1"
    ((PASS+=1))
}

fail()
{
    printf '\033[1;31m[FAIL]\033[0m %s\n' "$1"
    ((FAIL+=1))
}

run_positive()
{
    local description="$1"
    shift

    printf '\n\033[1;34m[TEST]\033[0m %s\n' "${description}"
    printf 'Command:'
    printf ' %q' "$@"
    printf '\n\n'

    if "$@"; then
        pass "${description}"
    else
        fail "${description}"
    fi
}

run_expected_failure()
{
    local description="$1"
    shift

    printf '\n\033[1;34m[TEST]\033[0m %s\n' "${description}"
    printf 'Command:'
    printf ' %q' "$@"
    printf '\n\n'

    "$@"
    local status=$?

    if (( status != 0 )); then
        printf '\033[1;32m[PASS]\033[0m %s returned expected non-zero exit status (%d)\n' \
            "${description}" "${status}"

        ((EXPECTED_FAIL_PASS+=1))
    else
        printf '\033[1;31m[FAIL]\033[0m %s unexpectedly returned success\n' \
            "${description}"

        ((EXPECTED_FAIL_FAIL+=1))
    fi
}

# ------------------------------------------------------------
# Environment
# ------------------------------------------------------------

heading "PONG Test Environment"

printf 'Project:     %s\n' "${PROJECT_ROOT}"
printf 'Interface:   %s\n' "${INTERFACE}"
printf 'Local host:  %s\n' "${LOCAL_TARGET}"
printf 'Gateway:     %s\n' "${GATEWAY_TARGET}"
printf 'DNS target:  %s\n' "${DNS_TARGET}"
printf 'Date:        %s\n' "$(date)"

printf '\n'

rustc --version
cargo --version

# ------------------------------------------------------------
# Clean build
# ------------------------------------------------------------

heading "Clean Build Tests"

info "Removing all previous Cargo build artifacts..."

if cargo clean; then
    pass "cargo clean"
else
    fail "cargo clean"
fi

printf '\n'
info "Checking Rust formatting..."

if cargo fmt --check; then
    pass "cargo fmt --check"
else
    fail "cargo fmt --check"
fi

printf '\n'
info "Running cargo check..."

if cargo check; then
    pass "cargo check"
else
    fail "cargo check"
fi

printf '\n'
info "Running Clippy with warnings promoted to errors..."

if cargo clippy --all-targets --all-features -- -D warnings; then
    pass "cargo clippy"
else
    fail "cargo clippy"
fi

printf '\n'
info "Running Rust tests..."

if cargo test; then
    pass "cargo test"
else
    fail "cargo test"
fi

printf '\n'
info "Building fresh debug binary..."

if cargo build; then
    pass "cargo build"
else
    fail "cargo build"
fi

if [[ ! -x "${PONG}" ]]; then
    fail "PONG debug binary was not created"

    printf '\n\033[1;31mCannot continue functional testing.\033[0m\n'
    exit 1
fi

# ------------------------------------------------------------
# Basic program tests
# ------------------------------------------------------------

heading "Basic Program Tests"

run_positive \
    "Display version" \
    "${PONG}" --version

run_positive \
    "Display help" \
    "${PONG}" --help

# ------------------------------------------------------------
# Positive ping tests
# ------------------------------------------------------------

heading "Positive Ping Tests"

run_positive \
    "Ping local host" \
    "${PONG}" "${LOCAL_TARGET}"

run_positive \
    "Ping gateway" \
    "${PONG}" "${GATEWAY_TARGET}"

run_positive \
    "Resolve DNS name and ping" \
    "${PONG}" "${DNS_TARGET}"

run_positive \
    "Three-packet count" \
    "${PONG}" -c 3 "${GATEWAY_TARGET}"

run_positive \
    "Custom payload size" \
    "${PONG}" -s 100 "${GATEWAY_TARGET}"

run_positive \
    "Custom timeout" \
    "${PONG}" -t 1000 "${GATEWAY_TARGET}"

run_positive \
    "Custom interval with count" \
    "${PONG}" -c 3 -i 250 "${GATEWAY_TARGET}"

# ------------------------------------------------------------
# JSON tests
# ------------------------------------------------------------

heading "JSON Tests"

run_positive \
    "JSON ping output" \
    "${PONG}" --json "${GATEWAY_TARGET}"

if command -v jq >/dev/null 2>&1; then
    printf '\n\033[1;34m[TEST]\033[0m Validate JSON with jq\n\n'

    if "${PONG}" --json "${GATEWAY_TARGET}" | jq empty; then
        pass "JSON validates with jq"
    else
        fail "JSON validates with jq"
    fi
else
    info "jq not installed -- JSON syntax validation skipped."
fi

# ------------------------------------------------------------
# Route tests
# ------------------------------------------------------------

heading "Route Tests"

run_positive \
    "Basic route trace" \
    "${PONG}" --route "${GATEWAY_TARGET}"

run_positive \
    "Verbose route trace" \
    "${PONG}" --route -v "${GATEWAY_TARGET}"

run_positive \
    "Route trace with DNS resolution" \
    "${PONG}" --route --resolve "${GATEWAY_TARGET}"

run_positive \
    "Route with explicit maximum hops" \
    "${PONG}" --route --max-hops 5 "${GATEWAY_TARGET}"

# ------------------------------------------------------------
# Broadcast/interface tests
# ------------------------------------------------------------

heading "Broadcast / Interface Tests"

run_positive \
    "List all broadcast interfaces" \
    "${PONG}" -b

run_positive \
    "List selected broadcast interface" \
    "${PONG}" -b "${INTERFACE}"

# ------------------------------------------------------------
# Sweep tests
# ------------------------------------------------------------

heading "Sweep Tests"

run_positive \
    "Sweep selected interface" \
    "${PONG}" -S "${INTERFACE}"

run_positive \
    "Sweep small explicit address range" \
    "${PONG}" -S \
        --low "${SWEEP_LOW}" \
        --high "${SWEEP_HIGH}"

run_positive \
    "Sweep network using CIDR" \
    "${PONG}" -S \
        --network "${SWEEP_LOW}/27"

run_positive \
    "Sweep with increased concurrency" \
    "${PONG}" -S "${INTERFACE}" \
        --concurrency 32

run_positive \
    "Sweep with custom timeout and interval" \
    "${PONG}" -S "${INTERFACE}" \
        -t 750 \
        -i 20

# ------------------------------------------------------------
# Sweep pipeline/output test
# ------------------------------------------------------------

heading "Sweep Output Tests"

TEMP_HOSTS="$(mktemp)"

printf '\n\033[1;34m[TEST]\033[0m Redirect sweep results to file\n\n'

if "${PONG}" -S "${INTERFACE}" > "${TEMP_HOSTS}"; then
    if [[ -s "${TEMP_HOSTS}" ]]; then
        pass "Sweep produced redirected host list"

        printf '\nDiscovered hosts:\n'
        cat "${TEMP_HOSTS}"
    else
        fail "Sweep completed but redirected host list is empty"
    fi
else
    fail "Sweep redirection test"
fi

rm -f "${TEMP_HOSTS}"

# ------------------------------------------------------------
# Expected command-line errors
# ------------------------------------------------------------

heading "Expected Error Tests"

run_expected_failure \
    "Missing target" \
    "${PONG}"

run_expected_failure \
    "COUNT cannot be zero" \
    "${PONG}" -c 0 "${GATEWAY_TARGET}"

run_expected_failure \
    "TIMEOUT cannot be zero" \
    "${PONG}" -t 0 "${GATEWAY_TARGET}"

run_expected_failure \
    "INTERVAL cannot be zero" \
    "${PONG}" -i 0 "${GATEWAY_TARGET}"

run_expected_failure \
    "Sweep concurrency cannot be zero" \
    "${PONG}" -S "${INTERFACE}" --concurrency 0

run_expected_failure \
    "Payload exceeds IPv4 maximum" \
    "${PONG}" -s 65508 "${GATEWAY_TARGET}"

run_expected_failure \
    "Invalid DNS name" \
    "${PONG}" this-host-should-not-exist.invalid

run_expected_failure \
    "IPv6 target is unsupported" \
    "${PONG}" ::1

# ------------------------------------------------------------
# Clap conflict tests
# ------------------------------------------------------------

heading "CLI Conflict Tests"

run_expected_failure \
    "Broadcast and route conflict" \
    "${PONG}" -b --route "${INTERFACE}"

run_expected_failure \
    "Broadcast and sweep conflict" \
    "${PONG}" -b -S "${INTERFACE}"

run_expected_failure \
    "Route and sweep conflict" \
    "${PONG}" --route -S "${GATEWAY_TARGET}"

run_expected_failure \
    "Route and JSON conflict" \
    "${PONG}" --route --json "${GATEWAY_TARGET}"

run_expected_failure \
    "Sweep and JSON conflict" \
    "${PONG}" -S --json "${INTERFACE}"

run_expected_failure \
    "Continuous and count conflict" \
    "${PONG}" -z -c 3 "${GATEWAY_TARGET}"

run_expected_failure \
    "--resolve without --route" \
    "${PONG}" --resolve "${GATEWAY_TARGET}"

run_expected_failure \
    "--verbose without --route" \
    "${PONG}" -v "${GATEWAY_TARGET}"

# ------------------------------------------------------------
# Sweep validation errors
# ------------------------------------------------------------

heading "Sweep Validation Tests"

run_expected_failure \
    "LOW greater than HIGH" \
    "${PONG}" -S \
        --low 192.168.254.100 \
        --high 192.168.254.20

run_expected_failure \
    "LOW without HIGH" \
    "${PONG}" -S \
        --low 192.168.254.20

run_expected_failure \
    "Non-contiguous subnet mask" \
    "${PONG}" -S \
        --network 192.168.254.0 \
        --mask 255.0.255.0

run_expected_failure \
    "Unsupported /31 sweep" \
    "${PONG}" -S \
        --network 192.168.254.0/31

run_expected_failure \
    "Invalid CIDR prefix" \
    "${PONG}" -S \
        --network 192.168.254.0/33

run_expected_failure \
    "Manual range and interface conflict" \
    "${PONG}" -S "${INTERFACE}" \
        --low 192.168.254.1 \
        --high 192.168.254.10

# ------------------------------------------------------------
# Final results
# ------------------------------------------------------------

heading "Test Results"

printf 'Positive tests passed:          %d\n' "${PASS}"
printf 'Positive tests failed:          %d\n' "${FAIL}"
printf 'Expected failures passed:       %d\n' "${EXPECTED_FAIL_PASS}"
printf 'Expected failures failed:       %d\n' "${EXPECTED_FAIL_FAIL}"

printf '\n'

if (( FAIL == 0 && EXPECTED_FAIL_FAIL == 0 )); then
    printf '\033[1;32mALL AUTOMATED TESTS PASSED\033[0m\n'
    exit 0
else
    printf '\033[1;31mONE OR MORE TESTS FAILED\033[0m\n'
    exit 1
fi