#!/usr/bin/env bash
# Run from any directory: ./script/test_ipv6.sh
# Optional real-network test, with a target supplied only at runtime:
# PONG_IPV6_TARGET='fe80::1%ens18' ./script/test_ipv6.sh
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
cargo test --locked
cargo build --locked
app=./target/debug/pong
"$app" -4 127.0.0.1 -c 3 -i 10
"$app" -6 ::1 -c 3 -i 10
"$app" ::1 -s 0
"$app" -6 ::1 -s 1400
"$app" -6 ::1 --json | python3 -c '
import json, sys
p = json.load(sys.stdin)
assert p["address"] == "::1" and p["received"] == 1
assert p["probes"][0]["status"] == "alive"
assert "hop_limit" in p["probes"][0] and "ttl" not in p["probes"][0]
'
expect_invalid() {
    local status=0
    "$app" "$@" >/dev/null 2>&1 || status=$?
    if [[ $status -ne 2 ]]; then
        printf 'Expected exit 2, got %s: %s\n' "$status" "$*" >&2
        exit 1
    fi
}
expect_invalid -4 ::1
expect_invalid -6 127.0.0.1
expect_invalid -4 -6 ::1
expect_invalid -6 fe80::1
expect_invalid -6 fe80::1%pong_missing_iface
expect_invalid -6 ff02::1%lo
"$app" -6 --route ::1
expect_invalid -6 -S
expect_invalid -6 -b
expect_invalid -6 ::1 -c 0
if [[ -n ${PONG_IPV6_TARGET:-} ]]; then
    "$app" -6 "$PONG_IPV6_TARGET" -c 3
fi
printf 'IPv4/IPv6 ping checks passed.\n'
