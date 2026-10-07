#!/usr/bin/env python3
# script/test_peers.py
# Check the unicast peer filter against local addresses and iproute2's full cache.
import ipaddress
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
app = str(root / "target/debug/pong")
subprocess.run(["cargo", "test", "--locked"], cwd=root, check=True)
subprocess.run(["cargo", "build", "--locked"], cwd=root, check=True)
interface = sys.argv[1] if len(sys.argv) > 1 else None
arguments = ["-6", "--neighbors"] + ([interface] if interface else [])

def read(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return json.loads(result.stdout)

local = read(["ip", "-j", "-6", "addr", "show"])
loopback = {item["ifname"] for item in local if "LOOPBACK" in item.get("flags", [])}
owned = set()
scoped = set()
for item in local:
    for address in item.get("addr_info", []):
        if address.get("family") == "inet6":
            ip = ipaddress.IPv6Address(address["local"])
            if ip.is_link_local:
                scoped.add((str(ip), item["ifname"]))
            else:
                owned.add(str(ip))

def is_peer(address, device, proxy=False):
    ip = ipaddress.IPv6Address(address)
    return (not ip.is_multicast and not ip.is_unspecified and not ip.is_loopback
            and ip.ipv4_mapped is None and not proxy and device not in loopback
            and str(ip) not in owned and (str(ip), device) not in scoped)

def ip_peers(rows):
    return {(str(ipaddress.IPv6Address(row["dst"])), row["dev"]) for row in rows
            if (interface is None or row["dev"] == interface)
            and is_peer(row["dst"], row["dev"], row.get("proxy", False))}

# ip defaults hide NONE/NOARP; use the complete view for comparison.
before = read(["ip", "-j", "-6", "neigh", "show", "nud", "all"])
peers = read([app, *arguments, "--peers", "--json"])
after = read(["ip", "-j", "-6", "neigh", "show", "nud", "all"])
assert peers["filter"] == "unicast_peers"
assert peers["count"] == len(peers["neighbors"]) <= peers["cached_count"]
assert peers["source"] == "kernel_neighbor_cache"
actual = set()
for entry in peers["neighbors"]:
    assert is_peer(entry["address"], entry["interface"], entry["proxy"]), entry
    assert entry["states"] and entry["interface_index"] > 0
    if interface:
        assert entry["interface"] == interface
    actual.add((str(ipaddress.IPv6Address(entry["address"])), entry["interface"]))
missing = (ip_peers(before) & ip_peers(after)) - actual
extra = actual - (ip_peers(before) | ip_peers(after))
if missing or extra:
    print("Missing from Pong:", sorted(missing))
    print("Only in Pong:", sorted(extra))
    print("Pong:", json.dumps(peers, indent=2))
    print("ip before:", json.dumps(before, indent=2))
    print("ip after:", json.dumps(after, indent=2))
    raise AssertionError("Peer snapshots differ; inspect output above or rerun if cache changed")

# The full view retains the existing JSON schema without filter metadata.
full = read([app, *arguments, "--json"])
assert "filter" not in full and "cached_count" not in full
assert full["count"] == len(full["neighbors"])
lo = read([app, "-6", "--neighbors", "lo", "--peers", "--json"])
assert lo["count"] == 0 and lo["neighbors"] == []
for args in [("-6", "--peers"), ("--peers",), ("-4", "--neighbors", "--peers")]:
    result = subprocess.run([app, *args], capture_output=True)
    assert result.returncode == 2, args
print(f"Peer checks passed: {peers['count']} unicast candidates from {peers['cached_count']} cached entries.")
