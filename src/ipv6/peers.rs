// src/ipv6/peers.rs
// Select remote unicast candidates from the IPv6 neighbor cache.

use crate::ipv6::neighbor::NeighborEntry;
use std::collections::HashSet;
use std::io;
use std::net::Ipv6Addr;

#[derive(Default)]
struct LocalAddresses {
    unscoped: HashSet<Ipv6Addr>,
    link_local: HashSet<(Ipv6Addr, u32)>,
    loopback_interfaces: HashSet<u32>,
}

struct InterfaceList(*mut libc::ifaddrs);

impl Drop for InterfaceList {
    fn drop(&mut self) {
        unsafe { libc::freeifaddrs(self.0) };
    }
}

impl LocalAddresses {
    fn read() -> io::Result<Self> {
        let mut head = std::ptr::null_mut();
        if unsafe { libc::getifaddrs(&mut head) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let list = InterfaceList(head);
        let mut cursor = list.0;
        let mut result = Self::default();
        while !cursor.is_null() {
            // getifaddrs owns these pointers until InterfaceList is dropped.
            let item = unsafe { &*cursor };
            if !item.ifa_name.is_null() {
                let index = unsafe { libc::if_nametoindex(item.ifa_name) };
                if index == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "Interface disappeared while reading local addresses; retry",
                    ));
                }
                if item.ifa_flags & libc::IFF_LOOPBACK as u32 != 0 {
                    result.loopback_interfaces.insert(index);
                }
                if !item.ifa_addr.is_null()
                    && unsafe { (*item.ifa_addr).sa_family as i32 } == libc::AF_INET6
                {
                    let address = unsafe { &*item.ifa_addr.cast::<libc::sockaddr_in6>() };
                    let ip = Ipv6Addr::from(address.sin6_addr.s6_addr);
                    if ip.is_unicast_link_local() {
                        result.link_local.insert((ip, index));
                    } else {
                        result.unscoped.insert(ip);
                    }
                }
            }
            cursor = item.ifa_next;
        }
        Ok(result)
    }

    fn is_peer(&self, entry: &NeighborEntry) -> bool {
        let address = entry.address;
        !address.is_multicast()
            && !address.is_unspecified()
            && !address.is_loopback()
            && address.to_ipv4_mapped().is_none()
            && !entry.proxy
            && !self.loopback_interfaces.contains(&entry.interface_index)
            && !self.unscoped.contains(&address)
            && !self.link_local.contains(&(address, entry.interface_index))
    }
}

/// Keep state and MAC information intact; this filter performs no active probes.
pub fn filter(entries: Vec<NeighborEntry>) -> io::Result<Vec<NeighborEntry>> {
    let local = LocalAddresses::read()?;
    Ok(entries
        .into_iter()
        .filter(|entry| local.is_peer(entry))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(address: &str, index: u32, state: u16) -> NeighborEntry {
        let address = address.parse::<Ipv6Addr>().unwrap();
        NeighborEntry {
            address,
            interface: format!("eth{index}"),
            interface_index: index,
            scope_id: if address.is_unicast_link_local() {
                index
            } else {
                0
            },
            mac: None,
            states: vec!["TEST".to_owned()],
            state_bits: state,
            router: false,
            proxy: false,
        }
    }

    #[test]
    fn rejects_multicast_unspecified_loopback_and_mapped_addresses() {
        let local = LocalAddresses::default();
        for address in ["ff02::1", "ff05::123", "::", "::1", "::ffff:192.0.2.1"] {
            assert!(!local.is_peer(&entry(address, 2, 64)), "{address}");
        }
        let mut local = LocalAddresses::default();
        local.loopback_interfaces.insert(1);
        assert!(!local.is_peer(&entry("fd12:3456::1", 1, 64)));
    }

    #[test]
    fn excludes_own_addresses_and_respects_link_local_scope() {
        let mut local = LocalAddresses::default();
        local.unscoped.insert("fd12:3456::1".parse().unwrap());
        local.link_local.insert(("fe80::1234".parse().unwrap(), 2));
        assert!(!local.is_peer(&entry("fd12:3456::1", 3, 2)));
        assert!(!local.is_peer(&entry("fe80::1234", 2, 2)));
        // The same link-local value can identify a different host on another link.
        assert!(local.is_peer(&entry("fe80::1234", 3, 2)));
    }

    #[test]
    fn preserves_unresolved_failed_noarp_and_router_candidates() {
        let local = LocalAddresses::default();
        for state in [0, 1, 2, 4, 8, 16, 32, 64, 128] {
            assert!(local.is_peer(&entry("fd12:3456::2", 2, state)));
        }
        let mut router = entry("fe80::2", 2, 4);
        router.router = true;
        assert!(local.is_peer(&router));
        router.proxy = true;
        assert!(!local.is_peer(&router));
    }
}
