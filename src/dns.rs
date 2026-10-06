use std::ffi::{CStr, CString};
use std::io;
use std::mem;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6, ToSocketAddrs};

/// Preserve IPv4 preference for dual-stack names; -6 selects AAAA answers.
/// No automatic retry across addresses or protocols is attempted in this milestone.
pub fn resolve_target(target: &str, ipv4: bool, ipv6: bool) -> io::Result<SocketAddr> {
    let invalid = |text: &str| io::Error::new(io::ErrorKind::InvalidInput, text);
    let address = if let Some((host, zone)) = target.rsplit_once('%') {
        let ip: Ipv6Addr = host
            .parse()
            .map_err(|_| invalid("Interface scopes require an IPv6 literal"))?;
        let scope = if let Ok(index) = zone.parse::<u32>() {
            let mut name = [0; libc::IF_NAMESIZE];
            if index == 0 || unsafe { libc::if_indextoname(index, name.as_mut_ptr()) }.is_null() {
                return Err(invalid("Invalid interface index"));
            }
            index
        } else {
            let name = CString::new(zone).map_err(|_| invalid("Invalid interface name"))?;
            let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
            if index == 0 {
                return Err(invalid("Unknown interface in IPv6 scope"));
            }
            index
        };
        SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, scope))
    } else if let Ok(ip) = target.parse::<IpAddr>() {
        SocketAddr::new(ip, 0)
    } else {
        let addresses: Vec<_> = (target, 0).to_socket_addrs()?.collect();
        addresses
            .iter()
            .copied()
            .find(|a| if ipv6 { a.is_ipv6() } else { a.is_ipv4() })
            .or_else(|| {
                if !ipv4 && !ipv6 {
                    addresses.first().copied()
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    if ipv6 {
                        "No IPv6 address found"
                    } else if ipv4 {
                        "No IPv4 address found"
                    } else {
                        "No IP address found"
                    },
                )
            })?
    };
    if (ipv4 && address.is_ipv6()) || (ipv6 && address.is_ipv4()) {
        return Err(invalid("Target conflicts with selected IP version"));
    }
    if let SocketAddr::V6(a) = address {
        if a.ip().is_multicast() || a.ip().is_unspecified() || a.ip().to_ipv4_mapped().is_some() {
            return Err(invalid("IPv6 ping requires a native unicast address"));
        }
        if a.ip().is_unicast_link_local() && a.scope_id() == 0 {
            return Err(invalid(
                "Link-local IPv6 requires %INTERFACE, for example fe80::1%ens18",
            ));
        }
    }
    Ok(address)
}

pub fn reverse_dns(address: IpAddr) -> Option<String> {
    let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let length = match address {
        IpAddr::V4(ip) => {
            let a = libc::sockaddr_in {
                sin_family: libc::AF_INET as _,
                sin_port: 0,
                sin_addr: libc::in_addr {
                    s_addr: u32::from_ne_bytes(ip.octets()),
                },
                sin_zero: [0; 8],
            };
            unsafe {
                std::ptr::write(
                    (&mut storage as *mut libc::sockaddr_storage).cast::<libc::sockaddr_in>(),
                    a,
                );
            }
            mem::size_of_val(&a)
        }
        IpAddr::V6(ip) => {
            let a = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as _,
                sin6_port: 0,
                sin6_flowinfo: 0,
                sin6_addr: libc::in6_addr {
                    s6_addr: ip.octets(),
                },
                sin6_scope_id: 0,
            };
            unsafe {
                std::ptr::write(
                    (&mut storage as *mut libc::sockaddr_storage).cast::<libc::sockaddr_in6>(),
                    a,
                );
            }
            mem::size_of_val(&a)
        }
    };
    let mut hostname = [0 as libc::c_char; libc::NI_MAXHOST as usize];
    let result = unsafe {
        libc::getnameinfo(
            (&storage as *const libc::sockaddr_storage).cast(),
            length as libc::socklen_t,
            hostname.as_mut_ptr(),
            hostname.len() as _,
            std::ptr::null_mut(),
            0,
            libc::NI_NAMEREQD,
        )
    };
    if result != 0 {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(hostname.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literals_and_family_mismatch() {
        assert!(resolve_target("::1", false, true).unwrap().is_ipv6());
        assert!(resolve_target("127.0.0.1", true, false).unwrap().is_ipv4());
        assert!(resolve_target("::1", true, false).is_err());
        assert!(resolve_target("127.0.0.1", false, true).is_err());
    }
    #[test]
    fn link_local_scope() {
        assert!(resolve_target("fe80::1", false, true).is_err());
        let a = resolve_target("fe80::1%lo", false, true).unwrap();
        assert!(matches!(a, SocketAddr::V6(v) if v.scope_id() != 0));
        assert!(resolve_target("fe80::1%pong_missing_iface", false, true).is_err());
        assert!(resolve_target("fe80::1%0", false, true).is_err());
        assert!(resolve_target("ff02::1%lo", false, true).is_err());
    }
}
