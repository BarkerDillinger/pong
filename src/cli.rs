// src/cli.rs
use clap::Parser;
use std::net::Ipv4Addr;

use crate::constants::{DEFAULT_PAYLOAD_SIZE, DEFAULT_SWEEP_CONCURRENCY};

#[derive(Parser, Debug)]
#[command(
    name = "pong",
    after_help = "IPv6 usage and options: pong --help6",
    version,
    about = "Linux ICMP reachability, latency, route tracing, and network discovery utility"
)]
pub struct Cli {
    /// Show only remote unicast candidates from the neighbor cache
    #[arg(long, requires = "neighbors")]
    pub peers: bool,

    /// Resolve only IPv4 addresses
    #[arg(short = '4', long, conflicts_with = "ipv6")]
    pub ipv4: bool,

    /// Show cached IPv6 neighbors
    #[arg(long, requires = "ipv6", conflicts_with_all = [
        "interfaces", "gateway", "route", "broadcast",
        "sweep", "dns", "reverse"
    ])]
    pub neighbors: bool,

    /// Display IPv6 interface addresses and default routes
    #[arg(long, requires = "ipv6", conflicts_with_all = [
        "neighbors", "gateway", "route", "broadcast",
        "sweep", "dns", "reverse"
    ])]
    pub interfaces: bool,

    /// Show the cached IPv4 ARP/neighbor table (TARGET may select an interface)
    #[arg(long, conflicts_with_all = [
        "ipv6", "neighbors", "interfaces", "gateway", "route",
        "broadcast", "sweep", "dns", "reverse", "peers",
        "count", "continuous", "resolve", "verbose"
    ])]
    pub arp: bool,

    /// Show age and location of local IEEE OUI data
    #[arg(long, conflicts_with_all = ["oui_update", "arp", "neighbors", "interfaces", "gateway", "route", "broadcast", "sweep", "dns", "reverse"])]
    pub oui_check: bool,

    /// Refresh IEEE OUI data if missing or at least 90 days old
    #[arg(long, conflicts_with_all = ["oui_check", "arp", "neighbors", "interfaces", "gateway", "route", "broadcast", "sweep", "dns", "reverse"])]
    pub oui_update: bool,

    /// Force an OUI database refresh regardless of age
    #[arg(long, requires = "oui_update")]
    pub oui_force: bool,

    /// Display IPv4 or IPv6 default gateways
    #[arg(long, conflicts_with_all = [
        "neighbors", "interfaces", "route",
        "broadcast", "sweep", "dns", "reverse"
    ])]
    pub gateway: bool,

    /// Select IPv6 for ping, traceroute, and diagnostics
    #[arg(hide = true, short = '6', long,
          conflicts_with_all = ["ipv4", "broadcast", "sweep"])]
    pub ipv6: bool,

    #[arg(short = 'c', long, value_name = "COUNT")]
    pub count: Option<u32>,

    #[arg(short = 'z', long, conflicts_with = "count")]
    pub continuous: bool,

    /// Probe local IPv4 broadcast addresses
    #[arg(
        short = 'b',
        long,
        conflicts_with_all = ["route", "sweep", "json"]
    )]
    pub broadcast: bool,

    /// ICMP payload size in bytes
    #[arg(
        short = 's',
        long = "size",
        value_name = "BYTES",
        default_value_t = DEFAULT_PAYLOAD_SIZE
    )]
    pub size: usize,

    /// Reply timeout in milliseconds
    #[arg(short = 't', long, value_name = "MILLISECONDS")]
    pub timeout: Option<u64>,

    /// Delay between transmitted requests in milliseconds
    #[arg(short = 'i', long = "interval", value_name = "MILLISECONDS")]
    pub interval: Option<u64>,

    /// Host/IP for ping/route, or interface name for -b/--neighbors
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,

    /// Trace the route to the destination
    #[arg(
        long,
        conflicts_with_all = ["broadcast", "sweep", "json"]
    )]
    pub route: bool,

    /// Perform reverse DNS lookups for route hops
    #[arg(long, requires = "route")]
    pub resolve: bool,

    /// Display detailed route information
    #[arg(short = 'v', long, requires = "route")]
    pub verbose: bool,

    /// Maximum number of hops when tracing a route
    #[arg(long = "max-hops", value_name = "HOPS", default_value_t = 30)]
    pub max_hops: u8,

    /// Sweep IPv4 addresses for responding hosts
    #[arg(
        short = 'S',
        long,
        conflicts_with_all = ["broadcast", "route", "json"]
    )]
    pub sweep: bool,

    /// Maximum number of outstanding sweep probes
    #[arg(
        long = "concurrency",
        value_name = "COUNT",
        default_value_t = DEFAULT_SWEEP_CONCURRENCY,
        requires = "sweep"
    )]
    pub concurrency: usize,

    /// Automatically confirm permitted large RFC1918 private-network sweeps
    #[arg(short = 'y', long = "yes", requires = "sweep")]
    pub yes: bool,

    /// Force a sweep regardless of network size or public/private address space
    #[arg(long, requires = "sweep")]
    pub force: bool,

    /// First IPv4 address in a manually specified sweep range
    #[arg(
        long,
        value_name = "ADDRESS",
        requires = "sweep",
        requires = "high",
        conflicts_with = "network"
    )]
    pub low: Option<Ipv4Addr>,

    /// Last IPv4 address in a manually specified sweep range
    #[arg(
        long,
        value_name = "ADDRESS",
        requires = "sweep",
        requires = "low",
        conflicts_with = "network"
    )]
    pub high: Option<Ipv4Addr>,

    /// IPv4 network to sweep
    #[arg(
        long,
        value_name = "NETWORK",
        requires = "sweep",
        conflicts_with_all = ["low", "high"]
    )]
    pub network: Option<String>,

    /// Subnet mask used with --network
    #[arg(long, value_name = "NETMASK", requires = "network")]
    pub mask: Option<Ipv4Addr>,

    /// Output structured results as JSON
    #[arg(long)]
    pub json: bool,
    /// Look up DNS A and AAAA addresses without sending ICMP packets
    #[arg(long, conflicts_with_all = ["reverse", "broadcast", "sweep", "route", "continuous", "json"])]
    pub dns: bool,

    /// Reverse DNS (PTR) lookup for an IPv4 or IPv6 address
    #[arg(long, conflicts_with_all = ["dns", "broadcast", "sweep", "route", "continuous", "json"])]
    pub reverse: bool,
}
