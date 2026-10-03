# PONG Command Reference

PONG is a Linux network utility for ICMP reachability testing, latency measurement, route tracing, broadcast-interface discovery, and IPv4 host sweeping.

This file describes the currently implemented command-line functions and provides simple examples for each one.

---

## Basic Syntax

```bash
pong [OPTIONS] [TARGET]
```

`TARGET` is normally a hostname or IP address.

When broadcast mode is used, `TARGET` may instead be a network interface name.

Examples:

```bash
pong 192.168.1.1
pong example.com
pong -b eth0
pong -S eth0
```

---

# Basic Ping

Ping a single IPv4 address:

```bash
pong 192.168.1.1
```

Ping a hostname:

```bash
pong example.com
```

PONG resolves hostnames before sending the ICMP request.

Typical output:

```text
192.168.1.1 ALIVE seq=1 ttl=64 hops≈0 time=0.412 ms
```

---

# Packet Count

Use `-c` or `--count` to send a specific number of probes.

```bash
pong -c 4 192.168.1.1
```

Example:

```bash
pong --count 10 example.com
```

When multiple packets are sent, PONG displays statistics including:

- packets transmitted
- packets received
- packet loss
- elapsed time
- minimum RTT
- average RTT
- maximum RTT
- RTT deviation
- TTL statistics
- estimated route distance

---

# Continuous Ping

Use `-z` or `--continuous` to continue sending probes until interrupted.

```bash
pong -z 192.168.1.1
```

Stop the command with:

```text
Ctrl+C
```

`--continuous` cannot be combined with `--count`.

---

# ICMP Payload Size

Use `-s` or `--size` to select the ICMP payload size in bytes.

Default:

```text
56 bytes
```

Example:

```bash
pong -s 100 192.168.1.1
```

Long form:

```bash
pong --size 512 192.168.1.1
```

---

# Reply Timeout

Use `-t` or `--timeout` to specify the reply timeout in milliseconds.

```bash
pong -t 1000 192.168.1.1
```

This example allows up to 1000 milliseconds for a reply.

---

# Probe Interval

Use `-i` or `--interval` to specify the delay between transmitted requests in milliseconds.

```bash
pong -c 5 -i 250 192.168.1.1
```

This sends five probes with a 250 ms delay between requests.

---

# JSON Output

Use `--json` to return structured ping results.

```bash
pong --json 192.168.1.1
```

Example output:

```json
{
  "schema_version": 1,
  "mode": "ping",
  "target": "192.168.1.1",
  "address": "192.168.1.1",
  "transmitted": 1,
  "received": 1,
  "probes": [
    {
      "sequence": 1,
      "status": "alive",
      "rtt_ms": 0.36,
      "ttl": 64
    }
  ]
}
```

JSON output can be piped into tools such as `jq`:

```bash
pong --json 192.168.1.1 | jq
```

`--json` cannot currently be combined with route or sweep mode.

---

# Route Tracing

Use `--route` to trace the path to a destination.

```bash
pong --route 192.168.1.1
```

Remote destination example:

```bash
pong --route example.com
```

Typical output:

```text
Tracing route to 192.168.1.1, maximum 30 hops

 1  192.168.1.1    0.314 ms

Route = 1 hops
```

---

## Reverse DNS During Route Tracing

Use `--resolve` with `--route` to perform reverse DNS lookups for discovered hops.

```bash
pong --route --resolve example.com
```

`--resolve` requires `--route`.

---

## Verbose Route Information

Use `-v` or `--verbose` with `--route`.

```bash
pong --route -v example.com
```

Verbose route mode displays additional per-hop information such as:

- response loss
- minimum latency
- average latency
- maximum latency
- latency deviation
- change in average latency

`--verbose` requires `--route`.

---

## Maximum Route Hops

Use `--max-hops` to limit the number of route hops.

Default:

```text
30 hops
```

Example:

```bash
pong --route --max-hops 15 example.com
```

---

# Broadcast Interface Discovery

Use `-b` or `--broadcast` to display local IPv4 broadcast-capable interfaces.

```bash
pong -b
```

Example output:

```text
Broadcast interfaces:
eth0 address=192.168.1.20 broadcast=192.168.1.255
```

To display a specific interface:

```bash
pong -b eth0
```

Broadcast mode cannot be combined with route or sweep mode.

---

# IPv4 Network Sweep

Use `-S` or `--sweep` to search IPv4 addresses for responding hosts.

PONG supports several ways to define the addresses to scan.

---

## Sweep an Interface

```bash
pong -S eth0
```

PONG determines the IPv4 network associated with the interface and probes addresses on that network.

Example output:

```text
192.168.1.1
192.168.1.10
192.168.1.20
192.168.1.50
```

This output format makes sweep results easy to redirect or pipe into another command.

Example:

```bash
pong -S eth0 > hosts.txt
```

---

## Sweep an Explicit Address Range

Use `--low` and `--high` together.

```bash
pong -S --low 192.168.1.10 --high 192.168.1.50
```

Both options are required when a manual range is used.

The lower address must not be greater than the upper address.

A manual address range cannot be combined with an interface sweep.

---

## Sweep a CIDR Network

Use `--network` with CIDR notation.

```bash
pong -S --network 192.168.1.0/24
```

Smaller example:

```bash
pong -S --network 192.168.1.0/27
```

---

## Sweep a Network With a Netmask

A network can also be specified using `--network` and `--mask`.

```bash
pong -S \
    --network 192.168.1.0 \
    --mask 255.255.255.0
```

The subnet mask must be contiguous.

---

# Sweep Concurrency

Use `--concurrency` to control the maximum number of outstanding sweep probes.

Default:

```text
16
```

Example:

```bash
pong -S eth0 --concurrency 32
```

Higher concurrency can make large sweeps faster but also increases the number of simultaneous probes.

---

# Sweep Timeout and Interval

Sweep mode accepts the normal timeout and interval options.

Example:

```bash
pong -S eth0 -t 750 -i 20
```

This sets:

```text
timeout  = 750 ms
interval = 20 ms
```

---

# Large Sweep Confirmation

PONG includes controls for larger network sweeps.

## Automatically Confirm Permitted Private-Network Sweeps

Use:

```bash
pong -S --network 192.168.0.0/16 --yes
```

Short form:

```bash
pong -S --network 192.168.0.0/16 -y
```

`--yes` automatically confirms large RFC1918 private-network sweeps when they are otherwise permitted.

---

## Force a Sweep

Use `--force` to force a sweep regardless of network size or whether the address space is public or private.

```bash
pong -S --network 192.168.0.0/16 --force
```

Use this option carefully and only on networks you are authorized to test.

---

# Combining Options

Several normal ping options may be combined.

Example:

```bash
pong -c 10 -s 128 -t 1500 -i 250 192.168.1.1
```

This command:

- sends 10 probes
- uses a 128-byte ICMP payload
- waits up to 1500 ms for each reply
- waits 250 ms between transmissions

Route options may also be combined:

```bash
pong --route --resolve -v --max-hops 20 example.com
```

Sweep options may be combined:

```bash
pong -S eth0 --concurrency 32 -t 750 -i 20
```

---

# Important Option Conflicts

PONG rejects incompatible modes rather than attempting to guess the intended operation.

The following combinations are not permitted:

```text
--broadcast + --route
--broadcast + --sweep
--route     + --sweep
--route     + --json
--sweep     + --json
--continuous + --count
```

These options also require route mode:

```text
--resolve
--verbose
```

---

# IPv6

The current version described by this help file does not yet support IPv6 targets.

For example:

```bash
pong ::1
```

will return an unsupported-target error.

IPv6 support can be added in a later release without changing the basic IPv4 command structure described here.

---

# Help

Display command-line help:

```bash
pong --help
```

or:

```bash
pong -h
```

---

# Version

Display the installed version:

```bash
pong --version
```

or:

```bash
pong -V
```

---

# Current Command-Line Options

| Option | Long Form | Description |
|---|---|---|
| `-c COUNT` | `--count COUNT` | Send a specified number of probes |
| `-z` | `--continuous` | Continue probing until interrupted |
| `-b` | `--broadcast` | Display/probe local IPv4 broadcast interfaces |
| `-s BYTES` | `--size BYTES` | Set ICMP payload size |
| `-t MS` | `--timeout MS` | Set reply timeout in milliseconds |
| `-i MS` | `--interval MS` | Set delay between requests |
| | `--route` | Trace the route to a destination |
| | `--resolve` | Reverse-resolve route hops |
| `-v` | `--verbose` | Display detailed route information |
| | `--max-hops HOPS` | Set maximum route hops |
| `-S` | `--sweep` | Sweep IPv4 addresses for responding hosts |
| | `--concurrency COUNT` | Set maximum outstanding sweep probes |
| `-y` | `--yes` | Automatically confirm permitted large private sweeps |
| | `--force` | Force a sweep regardless of network size or address class |
| | `--low ADDRESS` | First address in a manual sweep range |
| | `--high ADDRESS` | Last address in a manual sweep range |
| | `--network NETWORK` | Network or CIDR to sweep |
| | `--mask NETMASK` | Netmask used with `--network` |
| | `--json` | Return structured JSON ping results |
| `-h` | `--help` | Display help |
| `-V` | `--version` | Display version |

---

# Quick Examples

Ping a host:

```bash
pong 192.168.1.1
```

Ping a DNS name:

```bash
pong example.com
```

Send five probes:

```bash
pong -c 5 192.168.1.1
```

Continuous ping:

```bash
pong -z 192.168.1.1
```

Trace a route:

```bash
pong --route example.com
```

Trace a route with DNS resolution:

```bash
pong --route --resolve example.com
```

Verbose route trace:

```bash
pong --route -v example.com
```

List broadcast interfaces:

```bash
pong -b
```

Inspect one broadcast interface:

```bash
pong -b eth0
```

Sweep an interface:

```bash
pong -S eth0
```

Sweep a manual range:

```bash
pong -S --low 192.168.1.10 --high 192.168.1.100
```

Sweep a CIDR network:

```bash
pong -S --network 192.168.1.0/24
```

Save discovered hosts:

```bash
pong -S eth0 > hosts.txt
```

Generate JSON:

```bash
pong --json 192.168.1.1
```

Validate JSON with `jq`:

```bash
pong --json 192.168.1.1 | jq
```

---

PONG is intended to provide familiar network troubleshooting functions with concise output suitable for both direct terminal use and shell-script integration.
