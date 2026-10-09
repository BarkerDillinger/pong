# Pong IPv6 unicast peer filter

## Install

This additive patch requires the completed neighbor-cache milestone.
Extract the ZIP outside the Pong repository, then run from the project:

```bash
python3 /path/to/pong-peers-addon/install_peers.py .
cargo fmt
cargo test --locked
cargo build --locked
python3 script/test_peers.py
```

The installer adds `src/ipv6/peers.rs`, this document and a separate peer test
script. It edits CLI, IPv6 module registration, and neighbor output using
validated anchors. Original integration files are backed up under `/tmp` to
the directory printed by the installer. It refuses existing peer files or
unexpected code before writing. Existing ping code, neighbor retrieval, private
addresses, dependencies and the corrected `script/test_neighbors.py` are retained.
Review `git diff` before committing.

## Commands

```bash
# Full diagnostic view: unchanged.
target/debug/pong -6 --neighbors

# Remote unicast candidates on every interface.
target/debug/pong -6 --neighbors --peers

# Restrict to your Ethernet interface.
target/debug/pong -6 --neighbors enp0s20f0u1u4 --peers

target/debug/pong -6 --neighbors enp0s20f0u1u4 --peers --json

# Check filtered results against iproute2 and this host's address assignments.
python3 script/test_peers.py enp0s20f0u1u4
# Run your corrected full-cache comparison too.
python3 script/test_neighbors.py
```

`--peers` requires `--neighbors`, which requires `-6`.
The filter excludes multicast, unspecified, loopback and IPv4-mapped addresses,
all entries on loopback interfaces, proxy bindings, and addresses assigned to
the current host. Link-local ownership checks include the interface index:
an identical link-local value on a different link can represent a remote peer.
Local addresses are read through libc `getifaddrs`; neither probing nor shelling
out to `ip` is part of the application itself.

INCOMPLETE, FAILED and unknown-MAC entries remain visible if their address is a
remote unicast candidate. NOARP is not automatically excluded, because a remote
unicast entry can legitimately have that state. Router entries remain visible.
State and MAC fields are preserved. The filter does not verify reachability.

Multiple IPv6 addresses belonging to the same MAC remain separate candidates.
A count is an address/interface count, not a unique-device count.

## JSON

The default full-cache JSON output is unchanged. With `--peers`, the object adds:

- `filter`: `"unicast_peers"`
- `cached_count`: number of entries returned before peer filtering (already
  restricted to the requested interface, when supplied)
- `count`: number of candidates returned after filtering

`neighbors` contains the filtered entries with the existing address, interface,
MAC, state and scope fields. No matches returns successful JSON with count zero.
An error reading local addresses returns an error rather than an incomplete
filter result. Local address changes during cache retrieval can change the view;
rerun after interface reconfiguration has settled.

## Expected two-host result

Your Ethernet view should retain the target's ULA and link-local addresses,
both associated with `48:4d:7e:e8:fc:a4`, while excluding the multicast mappings
and this host's local/loopback entries. Kernel cache aging can remove entries,
so ping the target first if either address is absent.

## Delivery validation

The integrated project builds. All 14 unit tests pass (including three new
peer-filter tests), and Clippy passes with warnings treated as errors.
CLI requirements/conflicts and installer refusal protections also pass.
This container blocks netlink sockets, so live peer enumeration and local
address retrieval are not verified here. Run the included host comparison
script to verify the complete path on your test environment.
