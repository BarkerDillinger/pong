#!/usr/bin/env python3
# script/test_neighbors.py
# Compare Pong's read-only IPv6 neighbor dump with iproute2; no sudo required.
import json
import ipaddress
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
app = str(root / "target/debug/pong")
subprocess.run(["cargo", "test", "--locked"], cwd=root, check=True)
subprocess.run(["cargo", "build", "--locked"], cwd=root, check=True)

def pong(*args):
    result = subprocess.run([app, *args], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout

def pairs(p):
    return {(str(ipaddress.IPv6Address(n["address"])), n["interface"]) for n in p}

output = json.loads(pong("-6", "--neighbors", "--json"))
assert output["mode"] == "neighbors"
assert output["count"] == len(output["neighbors"])
assert output["source"] == "kernel_neighbor_cache"
for entry in output["neighbors"]:
    assert entry["interface_index"] > 0 and entry["states"]
    if ipaddress.IPv6Address(entry["address"]).is_link_local:
        assert entry["scope_id"] == entry["interface_index"]

# Include NONE/NOARP, which ip hides by default.
# Read snapshots on either side of Pong to reduce false failures due to aging.
before = json.loads(subprocess.check_output(["ip", "-j", "-6", "neigh", "show", "nud", "all"], text=True))
result = json.loads(pong("-6", "--neighbors", "--json"))
after = json.loads(subprocess.check_output(["ip", "-j", "-6", "neigh", "show", "nud", "all"], text=True))
def ip_pairs(rows):
    return {(str(ipaddress.IPv6Address(row["dst"])), row["dev"]) for row in rows}
expected_before = ip_pairs(before)
expected_after = ip_pairs(after)
actual = pairs(result["neighbors"])
missing = (expected_before & expected_after) - actual
extra = actual - (expected_before | expected_after)
if missing or extra:
    print("Compared against: ip -j -6 neigh show nud all")
    print("Missing from Pong:", sorted(missing))
    print("Only in Pong:", sorted(extra))
    print("Pong snapshot:", json.dumps(result, indent=2))
    print("ip snapshot before:", json.dumps(before, indent=2))
    print("ip snapshot after:", json.dumps(after, indent=2))
    raise AssertionError("Neighbor snapshots differ; inspect the entries above (cache churn is also possible)")
for entry in result["neighbors"]:
    candidates = [row for row in before + after
                  if row["dst"] == entry["address"] and row["dev"] == entry["interface"]]
    if candidates and all("lladdr" in row for row in candidates):
        assert entry["mac"] in {row["lladdr"].lower() for row in candidates}

# Loopback is a valid interface even when its cache is empty.
loopback = json.loads(pong("-6", "--neighbors", "lo", "--json"))
assert all(entry["interface"] == "lo" for entry in loopback["neighbors"])
for arguments in [("--neighbors",), ("-4", "--neighbors"),
                  ("-6", "--neighbors", "-c", "2"),
                  ("-6", "--neighbors", "--route"),
                  ("-6", "--neighbors", "-s", "56")]:
    check = subprocess.run([app, *arguments], capture_output=True)
    assert check.returncode == 2, arguments
unknown = subprocess.run([app, "-6", "--neighbors", "pong_missing_iface", "--json"], capture_output=True)
assert unknown.returncode != 0 and not unknown.stdout
print(f"Neighbor checks passed: {result['count']} cached IPv6 entries.")
