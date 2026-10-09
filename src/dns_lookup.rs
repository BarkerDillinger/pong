// Unprivileged DNS inspection. Uses the operating system's configured resolver.
use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, ToSocketAddrs};

use crate::dns::reverse_dns;

pub fn forward(name: &str, ipv4: bool, ipv6: bool) -> io::Result<()> {
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DNS name is empty",
        ));
    }
    let addresses: BTreeSet<IpAddr> = (name, 0)
        .to_socket_addrs()?
        .map(|socket| socket.ip())
        .filter(|ip| (!ipv4 || ip.is_ipv4()) && (!ipv6 || ip.is_ipv6()))
        .collect();
    if addresses.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No matching DNS address records found",
        ));
    }
    println!("DNS lookup: {name}");
    for address in addresses {
        println!(
            "{}  {address}",
            if address.is_ipv4() { "A   " } else { "AAAA" }
        );
    }
    Ok(())
}

pub fn reverse(address: &str, ipv4: bool, ipv6: bool) -> io::Result<()> {
    // A scope zone is irrelevant to reverse DNS's ip6.arpa record.
    let bare = address.split_once('%').map_or(address, |(ip, _)| ip);
    let ip: IpAddr = bare.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--reverse requires an IPv4 or IPv6 address",
        )
    })?;
    if (ipv4 && ip.is_ipv6()) || (ipv6 && ip.is_ipv4()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Address conflicts with selected IP version",
        ));
    }
    let name = reverse_dns(ip).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "No reverse DNS PTR record found")
    })?;
    println!("PTR   {ip}  {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_reverse_address() {
        assert!(reverse("not-an-address", false, false).is_err());
        assert!(reverse("127.0.0.1", false, true).is_err());
        assert!(reverse("::1", true, false).is_err());
    }
    #[test]
    fn rejects_empty_forward_name() {
        assert!(forward("", false, false).is_err());
    }
}
