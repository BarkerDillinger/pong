# Pong IPv6 ping — first milestone

This source bundle integrates IPv6 ping with the uploaded IPv4 application.
It contains the complete `src/` tree, the original Cargo manifest and lockfile,
and a new `script/test_ipv6.sh`. It does not include private test addresses,
previous documentation, existing test scripts, a `.gitignore`, or `target/`.

## Install into your existing project

Keep your IPv4 baseline committed or backed up first. From your existing pong
project, create a working branch and copy the extracted bundle's files:

```bash
git switch -c ipv6-ping
# Adjust this path to the extracted bundle.
bundle=/path/to/pong-ipv6
cp -a "$bundle/src/." src/
cp "$bundle/Cargo.toml" "$bundle/Cargo.lock" .
cp "$bundle/IPV6_PING.md" .
mkdir -p script
cp "$bundle/script/test_ipv6.sh" script/
chmod +x script/test_ipv6.sh
cargo test --locked
cargo build --locked
./script/test_ipv6.sh
```

Your existing `script/test.txt`, test script, README, help file and `.gitignore`
are not replaced by these commands. Run your existing IPv4 tests as well.

## Commands

```bash
# Literal addresses select their own protocol.
target/debug/pong ::1
target/debug/pong -4 127.0.0.1
target/debug/pong -6 ::1 -c 5

# AAAA resolution; requires DNS and an IPv6 route.
target/debug/pong -6 example.com

# Link-local addresses require an interface name or numeric interface index.
# Substitute an actual neighbor address and interface.
target/debug/pong -6 fe80::1%ens18 -c 3

# Continuous operation; Ctrl+C ends the loop and prints statistics.
target/debug/pong -6 ::1 -z

# Payload size, timeout and inter-probe delay (both timings in milliseconds).
target/debug/pong -6 ::1 -s 1400 -t 2000 -i 500 -c 3

target/debug/pong -6 ::1 --json

# Optional live target without storing addresses in the source tree.
PONG_IPV6_TARGET='fe80::1%ens18' ./script/test_ipv6.sh
```

One probe is the default. `-z` retains the existing continuous flag;
`-t` means timeout, not continuous mode. `-4` and `-6` conflict. A literal that
conflicts with the selected version is rejected. With neither flag, DNS names
prefer IPv4 if available and otherwise use IPv6. There is no route-aware
selection or automatic retry across DNS answers in this milestone.

IPv6 multicast, unspecified and IPv4-mapped targets are rejected. IPv6 route,
broadcast and sweep modes are not implemented; IPv4 modes retain their behavior.
The uploaded broadcast mode lists IPv4 broadcast interfaces; it does not send
broadcast probes.

## Output and errors

IPv6 replies show `hlim=` instead of `ttl=`. The statistics display Hop Limit.
The estimated hop count still assumes a conventional initial limit; it is a
heuristic, not a route measurement. Internally the existing `Alive.ttl` and
statistics storage are reused for either protocol.

Shared ping error addresses, route-hop addresses, JSON and reverse DNS now use
`IpAddr`. IPv4 interfaces and sweep ranges remain strongly typed as `Ipv4Addr`.
The resolver keeps IPv6 `scope_id` alongside the IP address.

IPv6 JSON replies use `hop_limit`, omit `ttl`, and include `scope_id` for scoped
targets. Address fields remain strings. Packet Too Big errors use the
`packet_too_big` status and include the advertised MTU. The existing
`schema_version: 1` is retained; the IPv6 fields are additive.

The Linux ICMPv6 error queue decodes destination unreachable, administrative
prohibition, Packet Too Big, time exceeded and parameter problem messages.
Unrecognized ICMPv6 errors retain their type and code. Local send failures
use the existing network/permission/local-error classification.

Exit status: 0 if at least one reply arrived; 1 if no replies arrived;
2 for invalid arguments or target resolution; 3 for socket creation failure.
Socket creation failures appear on stderr, including with `--json`.

## Linux ping socket permission

Both protocols use datagram ping sockets, not raw sockets. Inspect:

```bash
sysctl net.ipv4.ping_group_range
id -g
```

Despite its name, this setting also controls Linux IPv6 ping sockets. Your group
must fall within the configured range. If the range excludes your group, a
host administrator can permit that group (this replaces the existing range):

```bash
pong_gid=$(id -g)
sudo sysctl -w "net.ipv4.ping_group_range=$pong_gid $pong_gid"
```

A container may forbid socket creation or expose this setting read-only.
Giving the binary raw-socket capabilities does not replace this requirement;
the implementation does not fall back to raw sockets.

## Implementation and verification

`ipv6/packet.rs` builds ICMPv6 Echo Requests (type 128). Linux supplies the
ping identifier and pseudo-header checksum. `ipv6/socket.rs` uses an owned file
descriptor, preserves scope, receives Hop Limit ancillary data, and validates
reply source, identifier, sequence, length and payload. A monotonic deadline
limits waiting even when unrelated replies arrive.

Verified in the development environment:

- `cargo build --locked`
- `cargo test --locked`: five tests passed
- `cargo clippy --locked --all-targets -- -D warnings`
- CLI validation and socket-permission error handling

The receive-path tests use ordinary IPv6 UDP sockets to exercise `recvmsg`,
Hop Limit reception, reply filtering and deadline handling. They do not test
kernel ping identifier rewriting, ICMPv6 checksums or live error queues.
Live IPv4/IPv6 Echo tests could not run in the development container because
ping sockets were disabled for every group and the sysctl was read-only.
Run `script/test_ipv6.sh` on your host, then test a real scoped neighbor and a
routed IPv6 destination. Live router-generated ICMPv6 errors also need validation.

Continuous JSON mode stores probes until interrupted, as in the existing IPv4
implementation. Ctrl+C can wait for the current timeout or interval to finish.

## Linux API references

- [IPv6 socket API](https://www.man7.org/linux/man-pages/man7/ipv6.7.html)
- [Linux ping socket permission checks](https://github.com/torvalds/linux/blob/master/net/ipv4/ping.c)
