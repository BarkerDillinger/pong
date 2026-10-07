# Pong IPv6 neighbor enumeration

## Install

This additive bundle requires the IPv6 ping version already installed in your
project. It avoids replacing your current shared files with older uploads.
It adds `src/ipv6/neighbor.rs`, `src/modes/neighbors.rs`, this help file, and
`script/test_neighbors.py`. The installer edits CLI and main dispatch and
registers both modules. No Cargo dependencies are added.

Extract the ZIP outside your project. From your existing Pong project:

```bash
python3 /path/to/pong-neighbors-addon/install_neighbors.py .
cargo fmt
cargo test --locked
cargo build --locked
python3 script/test_neighbors.py
```

The installer validates all expected integration points before changing files.
It backs up the four existing integration files to a directory under `/tmp`,
and prints that directory. It refuses unexpected anchors or existing neighbor
files rather than overwriting them. Review the changes with `git diff` before
committing. It requires Python 3.8+; the comparison script also requires `ip`.
The existing private test addresses, firewall, network connections, Cargo files
and ping code are not modified.

## Commands

```bash
# Every interface in the current network namespace.
target/debug/pong -6 --neighbors

# One interface.
target/debug/pong -6 --neighbors enp0s20f0u1u4

# JSON, with numeric scope and interface IDs.
target/debug/pong -6 --neighbors enp0s20f0u1u4 --json

# Compare to Linux's view.
ip -6 neigh show dev enp0s20f0u1u4
```

Neighbor mode requires `-6`. The positional target is an optional interface name,
not a hostname. Ping, route, broadcast, sweep, count, size and timing options
are rejected in neighbor mode. A nonexistent interface returns an error.
An empty cache returns a successful result (JSON count 0 and neighbors []).

## What the entries mean

The table reports address, interface, MAC when known, and kernel neighbor state.
Link-local addresses include `%INTERFACE`. State values include INCOMPLETE,
REACHABLE, STALE, DELAY, PROBE, FAILED, NOARP, PERMANENT and NONE.
Unknown state bits remain visible. Router and proxy flags are displayed.
JSON preserves numeric state bits, interface index, scope ID, and those flags.
An unresolved MAC is `unknown` in the table and `null` in JSON.

This is a read-only kernel cache snapshot, not an active scan or complete host
inventory. STALE does not mean offline; REACHABLE is the kernel's NDP state,
not a verified Echo reply. Proxy entries identify proxy bindings, not necessarily
remote hosts. The command sends no ICMP traffic and does not require root or
ping-socket permissions. `SOCK_RAW` here refers to a netlink socket, not a
privileged raw IP socket.

## Test with your two hosts

First populate the cache by pinging your known peer, then compare:

```bash
target/debug/pong -6 fd7a:9c2e:4b61:10::254 -c 3
target/debug/pong -6 --neighbors enp0s20f0u1u4
ip -6 neigh show dev enp0s20f0u1u4
```

Expect the peer's IPv6 address with MAC `48:4d:7e:e8:fc:a4`. Link-local and
private IPv6 addresses may have separate cache entries for the same MAC.
State can change between commands as entries age. Do not commit private test
results if you prefer to keep your network details out of GitHub.

## Implementation

The command opens NETLINK_ROUTE and requests an AF_INET6 RTM_GETNEIGH multipart
dump. It validates kernel sender, sequence, lengths, attributes and completion.
Interrupted dumps, kernel errors, overrun, truncated messages, and a five-second
retrieval timeout return errors instead of presenting a partial inventory.
Interface filtering occurs after the dump for compatibility across kernels.
MAC formatting supports variable-length link-layer addresses, not only Ethernet.

Parser tests cover resolved/unresolved entries, state bits, unknown attributes,
family filtering, multipart replies, sequence filtering, malformed lengths,
kernel errors, and interrupted dumps. Run the existing IPv6 ping test script
as a regression check on your host.

References:
- https://docs.kernel.org/next/netlink/specs/rt-neigh.html
- https://man7.org/linux/man-pages/man7/rtnetlink.7.html

## Validation of this delivery

The integrated IPv6-ping project compiled successfully. All 11 unit tests passed
(the existing five plus six netlink parser tests). Clippy passed with warnings
treated as errors. Six CLI conflict checks and installer repeat protection passed.
This development container forbids AF_NETLINK sockets, including those used by
`ip`, so live kernel enumeration and comparison could not be verified here.
`script/test_neighbors.py` performs that comparison on your Linux host.
