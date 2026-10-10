# pong
The project began as an exercise in understanding how ping works at the packet and socket level and has grown into a small network diagnostic tool supporting ICMP reachability testing, latency measurement, route tracing, Linux extended ICMP error reporting, and structured JSON output.

Unlike a wrapper around the system ping command, pong constructs ICMP Echo Request packets and communicates directly with Linux ICMP sockets.

# pong

**pong** is a fast, lightweight Linux network diagnostics and discovery tool written in Rust. It brings IPv4 and IPv6 ICMP echo testing, traceroute, DNS lookup, network sweeps, gateway and interface inspection, and ARP/IPv6 neighbor-cache inspection into one command-line utility.

Unlike a wrapper around the system `ping`, pong builds ICMP Echo Requests and uses Linux ICMP datagram sockets. Its networking diagnostics are designed to operate as an **ordinary user**, without raw sockets, `sudo`, or `CAP_NET_RAW`, on systems that permit unprivileged ICMP ping sockets.

**Platform:** Linux · **Language:** Rust (edition 2024) · **License:** MIT

> **Release note:** This README documents the features developed for the **v1.0.0 release**. Before tagging the release, update `version` in `Cargo.toml` from `0.1.0` to `1.0.0` and rebuild so `pong --version` reports the intended version.

## Features at a glance

| Area | Functionality |
| --- | --- |
| IPv4 | Ping, IPv4 traceroute, interface/broadcast inspection, concurrent IPv4 host sweeping, default-gateway detection |
| IPv6 | Ping, IPv6 traceroute, link-local scope support, address/interface inspection, default-router detection |
| Neighbor discovery | Read-only IPv4 ARP cache and IPv6 Neighbor Discovery (NDP) cache; IPv6 peer candidates |
| Device identification | Offline manufacturer lookup using the IEEE MA-L OUI registry |
| DNS | Forward A and AAAA lookups; reverse PTR lookups; optional reverse resolution of route hops |
| Output | Color-coded diagnostic output, latency/loss statistics, and JSON for supported modes |
| OUI maintenance | Local CSV, 90-day age reminder, explicit check/update commands |

## Quick start

```bash
# IPv4 and IPv6 reachability
pong 192.168.1.1
pong -6 ::1
pong -c 5 example.com

# Routes
pong --route 1.1.1.1
pong -6 --route 2606:4700:4700::1111

# Discover responding IPv4 hosts
pong -S eth0

# Inspect IP-to-MAC mappings
pong --arp eth0
pong -6 --neighbors eth0
pong -6 --neighbors --peers eth0

# Interfaces and gateways
pong --gateway
pong -6 --gateway
pong -6 --interfaces eth0

# DNS
pong --dns example.com
pong -6 --dns example.com
pong --reverse 1.1.1.1

# Help
pong --help
pong --help6
```

Replace `eth0` with the name of your own network interface (for example, `enp1s0` or `ens18`).

## IPv4 diagnostics

### Ping and latency measurements

Ping a host by IPv4 address or hostname:

```bash
pong 192.168.1.1
pong -4 example.com
pong -c 5 192.168.1.1
```

A successful probe reports the sequence number, round-trip time (RTT), received TTL, and an estimated return-path hop count. Multiple probes provide packet transmission/reception counts, packet loss, elapsed time, and RTT minimum/average/maximum/deviation statistics. The hop estimate is calculated from the reply's TTL; it is **not** a measured forward-route length.

Useful options:

| Option | Purpose |
| --- | --- |
| `-4`, `--ipv4` | Select IPv4 for hostname resolution |
| `-c N`, `--count N` | Send N echo probes |
| `-z`, `--continuous` | Keep probing until Ctrl+C (cannot be combined with `-c`) |
| `-s BYTES`, `--size BYTES` | Set ICMP payload size (default: 56 bytes) |
| `-t MS`, `--timeout MS` | Set per-probe timeout (default: 2000 ms) |
| `-i MS`, `--interval MS` | Set probe interval (default: 1000 ms) |
| `--json` | Structured results for ping mode |

Example:

```bash
pong -c 10 -s 128 -t 1500 -i 250 192.168.1.1
```

Linux extended ICMP socket error reporting helps distinguish problems such as unreachable routes or destinations from ordinary timeouts. An unanswered ICMP request does **not** necessarily mean a device is offline; firewalls may drop probes silently.

### IPv4 traceroute

Trace a network path using a changing IPv4 TTL and ICMP responses:

```bash
pong --route 1.1.1.1
pong --route --max-hops 20 example.com
pong --route -v -c 5 example.com
pong --route --resolve example.com
```

A `*` indicates that no usable hop response was received within the timeout; intermediate routers may suppress ICMP responses. `-v` enables per-hop latency/loss statistics, and `--resolve` requests reverse DNS names for responding hops. Reverse lookups can increase runtime. The default hop limit is 30.

### IPv4 network sweep

The sweep mode sends concurrent probes to identify responding IPv4 addresses. It supports automatic network detection, named interfaces, explicit address ranges, and CIDR/netmask definitions.

```bash
pong -S eth0
pong -S --low 192.168.1.20 --high 192.168.1.80
pong -S --network 192.168.1.0/24
pong -S --network 192.168.1.0 --mask 255.255.255.0
pong -S eth0 --concurrency 32 -t 750 -i 20
```

By default the sweep concurrency limit is 16 outstanding probes. Range-size and address-space safety checks are enforced; `--yes` approves eligible large private-network sweeps and `--force` is required for certain otherwise restricted ranges. Use sweeps only on networks you own or are authorized to test.

Sweep output lists responding IP addresses and can be redirected to a file:

```bash
pong -S eth0 > hosts.txt
```

### IPv4 broadcast-capable interfaces

```bash
pong -b
pong -b eth0
```

This mode lists local IPv4 interfaces with their addresses and broadcast addresses. **It is interface enumeration, not an active broadcast host scan.**

### IPv4 default gateway

```bash
pong --gateway
pong --gateway eth0
pong --gateway --json
```

Shows default-route entries from the kernel routing tables, including next hop, interface, metric, and routing protocol where available. On systems with policy routing, multiple tables or defaults may appear; listing a route does not guarantee it is selected for every packet.

## IPv6 diagnostics

### IPv6 ping

Use `-6` to select AAAA records when resolving a hostname, or supply an IPv6 literal directly:

```bash
pong -6 ::1
pong -6 example.com -c 5
pong -6 2606:4700:4700::1111 -c 3
pong -6 ::1 -z
```

IPv6 link-local destinations require an interface scope because the same `fe80::/10` address can exist on more than one link:

```bash
pong -6 fe80::1%eth0 -c 3
```

IPv6 output includes the received hop limit (`hlim`), round-trip time, and packet statistics. The same count, size, timeout, interval, and JSON ping options are available.

**DNS and reachability are separate:** a successful AAAA lookup does not demonstrate that the computer has an IPv6 address or default route capable of reaching the destination.

### IPv6 traceroute

```bash
pong -6 --route 2606:4700:4700::1111
pong -6 --route -v -c 3 example.com
pong -6 --route --resolve --max-hops 64 example.com
pong --route 2001:db8::1
```

Uses ICMPv6 probes with increasing Hop Limits to identify intermediate hops. IPv6 literals select IPv6 automatically when `-4` is not specified. Local routing failures are reported instead of being mistaken for a succession of unanswered probes. Unresponsive intermediate hops can still appear as `*`.

### IPv6 interfaces and addresses

```bash
pong -6 --interfaces
pong -6 --interfaces eth0
pong -6 --interfaces --json
```

Shows IPv6 interface addresses and categorizes them as link-local, Unique Local Address (ULA), loopback, multicast, or global/other. It also shows IPv6 default-route information. This is read-only kernel-state inspection.

### IPv6 default routers

```bash
pong -6 --gateway
pong -6 --gateway eth0
pong -6 --gateway --json
```

Displays IPv6 default-route entries. A default router is often represented by a link-local `fe80::` next-hop address associated with a particular interface. An interface with only a link-local address and no default route cannot normally reach the public IPv6 internet.

### IPv6 Neighbor Discovery (NDP)

IPv6 uses Neighbor Discovery instead of ARP. pong reads known neighbors from the local kernel cache:

```bash
pong -6 --neighbors
pong -6 --neighbors eth0
pong -6 --neighbors --json
```

Entries may include IPv6 addresses, MAC addresses, interfaces, and neighbor states. They describe **previously learned neighbors**, not every host physically present on the network.

To show remote unicast candidates while excluding unsuitable or local addresses:

```bash
pong -6 --neighbors --peers eth0
```

Peer candidacy is not proof of current reachability. Cache inspection does not send active discovery packets.

## ARP cache and device identification

### Read the IPv4 ARP table

```bash
pong --arp
pong --arp eth0
pong --arp --json
```

ARP mode reads the Linux IPv4 neighbor cache and shows:

- IPv4 address and associated MAC address
- Network interface
- Linux neighbor reachability state
- Possible manufacturer from the offline IEEE OUI database

Only entries with usable MAC mappings are displayed. Failed or incomplete ARP resolution attempts are omitted. **No ARP packets are transmitted by the cache-inspection command.**

Typical states:

| State | Meaning | Terminal color |
| --- | --- | --- |
| `REACHABLE` | The kernel recently verified neighbor reachability | Green |
| `STALE` | A MAC mapping is known, but reachability has not been verified recently | Yellow |
| `DELAY` | The kernel is briefly delaying its next verification probe | Cyan |
| `PROBE` | The kernel is actively verifying an existing neighbor mapping | Magenta |

`STALE` and `DELAY` do **not** mean that a device is offline. They describe the age and verification status of the kernel's neighbor entry. Likewise, an ARP cache entry without an ICMP reply is not proof of a responsive host.

### MAC address manufacturer lookup

pong matches the first 24 bits of a universally administered MAC address to an **IEEE MA-L OUI** assignment in the local registry. The result identifies the registered organization, which may be a network adapter vendor, board manufacturer, or original equipment manufacturer—not necessarily the retail device brand or model.

Randomized or locally administered MAC addresses cannot generally be identified reliably from the OUI prefix. Missing matches are reported as `Unknown` or the appropriate address category.

The OUI database lives at:

```text
~/.local/share/pong/oui.csv
```

To use a custom location:

```bash
PONG_OUI_FILE=/path/to/oui.csv pong --arp
```

### OUI database maintenance

Manufacturer lookups are fully offline. Normal ARP commands **never download a database or perform a connectivity check**. Once the database is 90 days old, interactive ARP output shows a short reminder to update it; JSON remains free of reminders.

```bash
pong --oui-check              # Show database path, age, and status
pong --oui-update             # Download only if missing or 90+ days old
pong --oui-update --oui-force # Download regardless of age
```

Explicit updates download IEEE's published CSV over HTTPS using `curl`, validate it, and replace the local file only after validation. A failed update leaves the existing database intact. The database is not included in Git and should not contain local network inventory data.

## DNS lookups

```bash
pong --dns example.com               # A and AAAA addresses
pong -4 --dns example.com            # IPv4 A addresses only
pong -6 --dns example.com            # IPv6 AAAA addresses only
pong --reverse 1.1.1.1               # PTR lookup for IPv4
pong -6 --reverse 2606:4700:4700::1111  # PTR lookup for IPv6
```

DNS lookups use the system's configured name-resolution services. Reverse lookup of responding traceroute hops uses the separate `--route --resolve` combination. Results can be absent even when an address is reachable.

## JSON and shell integration

Supported ping and read-only diagnostic modes can produce machine-readable JSON. For example:

```bash
pong --json 192.168.1.1 | jq '.probes'
pong --arp --json | jq '.[] | {ip: .dst, mac: .lladdr, vendor}'
pong -6 --neighbors --json | jq .
pong --gateway --json | jq .
```

JSON support is mode-specific: route tracing and IPv4 sweeping do not currently expose the same JSON interface. Avoid relying on colored human-readable output for automated parsing.

## Installation

### Build from source

Install a Rust toolchain (Rust edition 2024 support required), `git`, and Linux `iproute2`. The OUI download command additionally requires `curl`; `jq` is optional for working with JSON.

```bash
git clone https://github.com/BarkerDillinger/pong.git
cd pong
cargo build --release

mkdir -p ~/.local/bin
install -m 755 target/release/pong ~/.local/bin/pong
```

Ensure `~/.local/bin` is on your `PATH`, then verify:

```bash
pong --version
pong --help
pong --help6
```

The repository's source-build procedure is supported on common Linux distributions such as Debian/Ubuntu, Fedora, and Arch. Binary compatibility depends on architecture and linked libraries; no universal precompiled installer is assumed here.

### Permissions and dependencies

pong uses unprivileged Linux ICMP datagram sockets for echo diagnostics. Whether they work for a given user depends on the system's ping socket policy, including `ping_group_range`:

```bash
sysctl net.ipv4.ping_group_range
sysctl net.ipv6.ping_group_range
```

Passive ARP, NDP, interface, and route diagnostics use Linux kernel state; some invoke the `ip` utility from `iproute2`. They do not require root, raw packet capture, or special file capabilities. OUI updates require outbound HTTPS access and `curl` only when explicitly requested.

## Safety and limitations

- **Linux only** in this release. macOS and Windows require platform-specific socket and networking backends.
- Ping and traceroute responses can be filtered or rate-limited by devices and firewalls. A timeout is not proof of device failure.
- ARP/NDP views reflect kernel caches, not complete live-network inventories.
- OUI assignments identify registered organizations, not definitive device makes or models.
- IPv6 link-local targets need a scope identifier such as `%eth0`.
- IPv6 DNS resolution may work over IPv4 even when no usable IPv6 route is configured.
- IPv4 sweep is active probing. Use it only on networks where you have permission.
- Normal operation does not require `sudo` or raw sockets; local OS settings may restrict unprivileged ICMP.

## Development and testing

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Project test scripts, where available, can be run from the repository root:

```bash
script/test_pong.sh
script/test_ipv6.sh
```

Local network-specific test settings should remain in `script/test.txt` and are ignored by Git. Do not commit private network inventory, locally downloaded OUI databases, or temporary installer backup files.

## Project and license

Source code: [github.com/BarkerDillinger/pong](https://github.com/BarkerDillinger/pong)

pong was created as a Rust networking and systems-programming project, with an emphasis on understandable socket-level behavior, useful diagnostics, and straightforward terminal operation.

Distributed under the **MIT License**. See [LICENSE](LICENSE).
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
