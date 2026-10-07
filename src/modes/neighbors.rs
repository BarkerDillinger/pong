// src/modes/neighbors.rs
// Display IPv6 neighbor-cache snapshots as a table or structured JSON.

use crate::cli::Cli;
use crate::ipv6::neighbor::{self, NeighborEntry};
use serde::Serialize;
use std::io;

#[derive(Serialize)]
struct JsonNeighbors<'a> {
    schema_version: u32,
    mode: &'static str,
    source: &'static str,
    interface: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_count: Option<usize>,
    count: usize,
    neighbors: &'a [NeighborEntry],
}

pub fn run(cli: &Cli) -> io::Result<()> {
    let interface = cli.target.as_deref();
    let entries = neighbor::list(interface)?;
    let cached_count = entries.len();
    let entries = if cli.peers {
        crate::ipv6::peers::filter(entries)?
    } else {
        entries
    };
    if cli.json {
        let output = JsonNeighbors {
            schema_version: 1,
            mode: "neighbors",
            source: "kernel_neighbor_cache",
            interface,
            filter: cli.peers.then_some("unicast_peers"),
            cached_count: cli.peers.then_some(cached_count),
            count: entries.len(),
            neighbors: &entries,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(io::Error::other)?
        );
        return Ok(());
    }
    println!(
        "{}{}",
        if cli.peers {
            "IPv6 unicast peer candidates"
        } else {
            "IPv6 neighbor cache"
        },
        interface
            .map(|name| format!(" — {name}"))
            .unwrap_or_default()
    );
    if entries.is_empty() {
        if cli.peers {
            println!("No remote unicast candidates found in {cached_count} cached entries.");
        } else {
            println!("No cached IPv6 neighbors found. Ping a known peer to populate the cache.");
        }
        return Ok(());
    }
    println!("{:<44} {:<20} {:<23} STATE", "ADDRESS", "INTERFACE", "MAC");
    for entry in &entries {
        let address = if entry.scope_id != 0 {
            format!("{}%{}", entry.address, entry.interface)
        } else {
            entry.address.to_string()
        };
        let mut states = entry.states.join("|");
        if entry.router {
            states.push_str(" router");
        }
        if entry.proxy {
            states.push_str(" proxy");
        }
        println!(
            "{:<44} {:<20} {:<23} {}",
            address,
            entry.interface,
            entry.mac.as_deref().unwrap_or("unknown"),
            states
        );
    }
    if cli.peers {
        println!(
            "\n{} unicast candidates from {cached_count} cached entries; ping reachability is unverified.",
            entries.len()
        );
    } else {
        println!(
            "\n{} cached entries; cache state does not confirm ping reachability.",
            entries.len()
        );
    }
    Ok(())
}
