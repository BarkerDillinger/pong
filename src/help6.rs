// IPv6-specific help, intentionally separate from the default clap help.
pub const HELP: &str = r#"pong — IPv6 diagnostics (Linux)

USAGE:
  pong -6 [OPTIONS] <TARGET>
  pong --help6

IPv6 ICMP ECHO:
  pong -6 ::1
  pong -6 example.com -c 5
  pong -6 fe80::1%ens18 -c 3
  pong -6 ::1 -z                   Continuous until Ctrl+C
  pong -6 ::1 -s 1400 -t 2000    Payload bytes and timeout (ms)
  pong -6 ::1 --json

IPv6 TRACEROUTE:
  pong -6 --route 2606:4700:4700::1111
  pong -6 --route -v -c 3 example.com
  pong -6 --route --resolve --max-hops 64 example.com
  pong --route 2001:db8::1        An IPv6 literal selects IPv6 automatically


IPv6 DNS LOOKUPS:
  pong -6 --dns cloudflare.com       Display AAAA addresses
  pong --dns cloudflare.com          Display A and AAAA addresses
  pong -6 --reverse 2606:4700:4700::1111   Reverse PTR lookup
  pong --reverse 1.1.1.1            IPv4 reverse PTR lookup

OPTIONS:
  -6, --ipv6             Force IPv6 address resolution
  -c, --count <N>        Echo probes; per-hop probes with --route
  -z, --continuous       Continuous ping (not route tracing)
  -s, --size <BYTES>     ICMP Echo payload, default 56 bytes
  -t, --timeout <MS>     Reply timeout, default 2000 ms
  -i, --interval <MS>    Delay between echo probes, default 1000 ms
      --route            Trace hops with ICMPv6 and Hop Limit
      --max-hops <N>     Maximum route hops, default 30
  -v, --verbose          Route per-hop RTT and loss statistics
      --resolve          Reverse DNS for responding route hops
      --json             JSON for ping mode (not route)
      --help6            This IPv6-specific help

NOTE: IPv6 link-local targets require %INTERFACE scope. Route mode does not
support multicast targets. Ordinary echo sockets can work without root if
Linux ping_group_range permits your group. This is a Linux-only utility.
For IPv4 options use: pong --help

IPv6 NETWORK DIAGNOSTICS (unprivileged):
  pong -6 --neighbors             Cached NDP addresses and MACs
  pong -6 --neighbors ens18       NDP entries on interface ens18
  pong -6 --interfaces            IPv6 addresses and default routes
  pong -6 --interfaces ens18      Only ens18 IPv6 addresses and default routes
  pong -6 --gateway               IPv6 default router(s)
  pong --gateway                  IPv4 default gateway(s)
  pong --gateway ens18            IPv4 gateway for ens18
  pong -6 --gateway --json        Raw JSON default-route entries

These commands read the kernel's network state using the `ip` utility.
No sudo or capabilities needed; NDP cache listing does not probe for hosts.
"#;
