//! Read-only Linux network diagnostics via iproute2's JSON output.
//! No raw sockets, file capabilities, or root permissions required.
use serde_json::Value;
use std::io;
use std::process::Command;

fn invoke(args: &[&str]) -> io::Result<Vec<Value>> {
    let output = Command::new("ip").args(args).output().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to execute `ip` (install iproute2): {err}"),
        )
    })?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "ip {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    serde_json::from_slice::<Vec<Value>>(&output.stdout).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid iproute2 JSON: {err}"),
        )
    })
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("-")
}

fn interface_name(iface: Option<&str>) -> io::Result<Option<&str>> {
    match iface {
        Some(name)
            if name.is_empty() || name.starts_with('-') || name.contains(char::is_whitespace) =>
        {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid interface name",
            ))
        }
        other => Ok(other),
    }
}

fn classify_v6(ip: &str) -> &'static str {
    match ip.parse::<std::net::Ipv6Addr>() {
        Ok(address) if address.is_loopback() => "loopback",
        Ok(address) if address.is_unicast_link_local() => "link-local",
        Ok(address) if address.octets()[0] & 0xfe == 0xfc => "ULA",
        Ok(address) if address.is_multicast() => "multicast",
        Ok(address) if address.is_unspecified() => "unspecified",
        Ok(_) => "global/other",
        Err(_) => "unknown",
    }
}

/// Display the kernel IPv4 neighbor cache (ARP); does not probe the network.
pub fn arp(iface: Option<&str>, json: bool) -> io::Result<()> {
    let iface = interface_name(iface)?;
    let mut args = vec!["-j", "-4", "neigh", "show"];
    if let Some(name) = iface {
        args.extend_from_slice(&["dev", name]);
    }
    let entries = invoke(&args)?;
    // A populated link-layer address is useful even when the entry is STALE.
    // FAILED/INCOMPLETE/NOARP entries are not evidence of an identified host.
    let entries: Vec<Value> = entries
        .into_iter()
        .filter(|entry| {
            entry
                .get("lladdr")
                .and_then(Value::as_str)
                .is_some_and(|mac| crate::oui::parse_mac(mac).is_some())
                && !matches!(
                    entry.get("state"),
                    Some(Value::String(s)) if s == "FAILED" || s == "INCOMPLETE" || s == "NOARP"
                )
                && !entry
                    .get("state")
                    .and_then(Value::as_array)
                    .is_some_and(|states| {
                        states
                            .iter()
                            .any(|s| matches!(s.as_str(), Some("FAILED" | "INCOMPLETE" | "NOARP")))
                    })
        })
        .collect();

    crate::oui_update::reminder_for_arp(json);
    let vendors = crate::oui::VendorDb::load();
    if json {
        let enriched: Vec<Value> = entries
            .into_iter()
            .map(|mut entry| {
                if let Some(mac) = entry.get("lladdr").and_then(Value::as_str) {
                    let vendor = vendors.lookup(mac);
                    if let Some(object) = entry.as_object_mut() {
                        object.insert("vendor".into(), Value::String(vendor));
                    }
                }
                entry
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&enriched).map_err(io::Error::other)?
        );
        return Ok(());
    }
    use console::style;
    println!(
        "{}",
        style(format!(
            "IPv4 ARP cache{}",
            iface.map_or(String::new(), |name| format!(" — {name}"))
        ))
        .cyan()
        .bold()
    );
    println!(
        "{:<16} {:<20} {:<14} {:<12} MANUFACTURER",
        "IPv4 ADDRESS", "MAC ADDRESS", "INTERFACE", "STATE"
    );
    for entry in &entries {
        let state = match entry.get("state") {
            Some(Value::Array(states)) => states
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
            Some(Value::String(s)) => s.clone(),
            _ => "UNKNOWN".into(),
        };
        let mac = str_field(entry, "lladdr");
        // Pad the fields before coloring so ANSI escape codes do not shift columns.
        let ip = format!("{:<16}", str_field(entry, "dst"));
        let state_text = format!("{:<12}", state);
        let (ip_colored, state_colored) = match state.as_str() {
            "REACHABLE" => (style(ip).green().bold(), style(state_text).green().bold()),
            "STALE" => (style(ip).yellow(), style(state_text).yellow()),
            "DELAY" => (style(ip).cyan(), style(state_text).cyan()),
            "PROBE" => (style(ip).magenta(), style(state_text).magenta()),
            _ => (style(ip).white(), style(state_text).white()),
        };
        let dev = match str_field(entry, "dev") {
            "-" => iface.unwrap_or("-"),
            name => name,
        };
        println!(
            "{} {:<20} {:<14} {} {}",
            ip_colored,
            mac,
            dev,
            state_colored,
            vendors.lookup(mac)
        );
    }
    println!(
        "{} cached MAC entr{}; no ARP packets transmitted",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    Ok(())
}

pub fn neighbors(iface: Option<&str>, json: bool) -> io::Result<()> {
    let iface = interface_name(iface)?;
    let mut args = vec!["-j", "-6", "neigh", "show"];
    if let Some(name) = iface {
        args.extend_from_slice(&["dev", name]);
    }
    let entries = invoke(&args)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).map_err(io::Error::other)?
        );
        return Ok(());
    }
    println!(
        "IPv6 neighbor cache (NDP){}",
        iface.map_or(String::new(), |name| format!(" — {name}"))
    );
    println!(
        "{:<39} {:<20} {:<12} STATE",
        "IPv6 ADDRESS", "MAC ADDRESS", "INTERFACE"
    );
    for entry in &entries {
        let state = match entry.get("state") {
            Some(Value::Array(states)) => states
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
            Some(Value::String(state)) => state.clone(),
            _ => "UNKNOWN".into(),
        };
        println!(
            "{:<39} {:<20} {:<12} {}",
            str_field(entry, "dst"),
            str_field(entry, "lladdr"),
            str_field(entry, "dev"),
            state
        );
    }
    println!(
        "{} cached entr{}; this is not an active host scan",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    Ok(())
}

pub fn interfaces(iface: Option<&str>, json: bool) -> io::Result<()> {
    let iface = interface_name(iface)?;
    let mut args = vec!["-j", "-6", "addr", "show"];
    if let Some(name) = iface {
        args.extend_from_slice(&["dev", name]);
    }
    let entries = invoke(&args)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&entries).map_err(io::Error::other)?
        );
        return Ok(());
    }
    println!("IPv6 interface diagnostics");
    for entry in entries {
        let dev = str_field(&entry, "ifname");
        println!("\n{} — {}", dev, str_field(&entry, "operstate"));
        let mut count = 0;
        if let Some(addresses) = entry.get("addr_info").and_then(Value::as_array) {
            for addr in addresses {
                if str_field(addr, "family") != "inet6" {
                    continue;
                }
                count += 1;
                let ip = str_field(addr, "local");
                let prefix = addr.get("prefixlen").and_then(Value::as_u64).unwrap_or(0);
                let scope = str_field(addr, "scope");
                println!("  {ip}/{prefix}  [{}; scope {scope}]", classify_v6(ip));
            }
        }
        if count == 0 {
            println!("  No IPv6 addresses");
        }
    }
    // Show IPv6 default routes as part of interface diagnostics.
    gateways(true, iface, false)
}

fn is_default(route: &Value) -> bool {
    matches!(
        route.get("dst").and_then(Value::as_str),
        Some("default") | Some("0.0.0.0/0") | Some("::/0")
    )
}

pub fn gateways(ipv6: bool, iface: Option<&str>, json: bool) -> io::Result<()> {
    let iface = interface_name(iface)?;
    let args = if ipv6 {
        ["-j", "-6", "route", "show", "table", "all"]
    } else {
        ["-j", "-4", "route", "show", "table", "all"]
    };
    let routes = invoke(&args)?;
    // Do not claim a non-main policy route is the active default: show the table explicitly.
    let defaults: Vec<Value> = routes
        .into_iter()
        .filter(|r| is_default(r) && iface.is_none_or(|name| str_field(r, "dev") == name))
        .collect();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&defaults).map_err(io::Error::other)?
        );
        return Ok(());
    }
    println!(
        "\nIPv{} default gateways{}",
        if ipv6 { 6 } else { 4 },
        iface.map_or(String::new(), |name| format!(" ({name})"))
    );
    if defaults.is_empty() {
        println!("  No default route configured");
        return Ok(());
    }
    for route in defaults {
        let gateway = str_field(&route, "gateway");
        let dev = str_field(&route, "dev");
        let metric = route
            .get("metric")
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".into());
        let table = str_field(&route, "table");
        let protocol = str_field(&route, "protocol");
        println!("  via {gateway} dev {dev} metric {metric} table {table} proto {protocol}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipv6_address_categories() {
        assert_eq!(classify_v6("fe80::1"), "link-local");
        assert_eq!(classify_v6("fd12::1"), "ULA");
        assert_eq!(classify_v6("2606:4700::1111"), "global/other");
        assert_eq!(classify_v6("::1"), "loopback");
    }
    #[test]
    fn accepts_default_route_spellings() {
        assert!(is_default(&serde_json::json!({"dst":"default"})));
        assert!(is_default(&serde_json::json!({"dst":"::/0"})));
        assert!(!is_default(&serde_json::json!({"dst":"fe80::/64"})));
    }
    #[test]
    fn reject_bad_interface() {
        assert!(interface_name(Some("-bad")).is_err());
        assert!(interface_name(Some("ens18")).is_ok());
    }
}
