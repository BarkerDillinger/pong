// src/ipv6/neighbor.rs
// Read Linux IPv6 neighbor-cache entries through rtnetlink without sending probes.

use serde::Serialize;
use std::ffi::{CStr, CString};
use std::io;
use std::mem;
use std::net::Ipv6Addr;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

const HEADER_LEN: usize = 16;
const NDMSG_LEN: usize = 12;
const RTM_NEWNEIGH: u16 = 28;
const RTM_GETNEIGH: u16 = 30;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLMSG_OVERRUN: u16 = 4;
const NLM_F_REQUEST: u16 = 1;
const NLM_F_DUMP: u16 = 0x300;
const NLM_F_DUMP_INTR: u16 = 0x10;
const SEQUENCE: u32 = 1;
const DUMP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize)]
pub struct NeighborEntry {
    pub address: Ipv6Addr,
    pub interface: String,
    pub interface_index: u32,
    /// Interface index for link-local addresses, otherwise zero.
    pub scope_id: u32,
    pub mac: Option<String>,
    pub states: Vec<String>,
    pub state_bits: u16,
    pub router: bool,
    pub proxy: bool,
}

#[derive(Debug)]
struct RawNeighbor {
    address: Ipv6Addr,
    interface_index: u32,
    mac: Option<String>,
    state_bits: u16,
    flags: u8,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn interface_index(name: &str) -> io::Result<u32> {
    let name_c = CString::new(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Interface name contains a NUL byte",
        )
    })?;
    let index = unsafe { libc::if_nametoindex(name_c.as_ptr()) };
    if index == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Unknown interface: {name}"),
        ));
    }
    Ok(index)
}

fn interface_name(index: u32) -> String {
    let mut name = [0 as libc::c_char; libc::IF_NAMESIZE];
    if unsafe { libc::if_indextoname(index, name.as_mut_ptr()) }.is_null() {
        // A device can disappear during the dump; preserve the entry's index.
        return format!("if#{index}");
    }
    unsafe { CStr::from_ptr(name.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn state_names(bits: u16) -> Vec<String> {
    let names = [
        (1, "INCOMPLETE"),
        (2, "REACHABLE"),
        (4, "STALE"),
        (8, "DELAY"),
        (16, "PROBE"),
        (32, "FAILED"),
        (64, "NOARP"),
        (128, "PERMANENT"),
    ];
    let mut result: Vec<String> = names
        .into_iter()
        .filter(|(mask, _)| bits & mask != 0)
        .map(|(_, name)| name.to_owned())
        .collect();
    if bits & !0xff != 0 {
        result.push(format!("UNKNOWN(0x{:x})", bits & !0xff));
    }
    if result.is_empty() {
        result.push("NONE".to_owned());
    }
    result
}

/// Read-only snapshot. No root, ping permissions or external `ip` command needed.
pub fn list(interface: Option<&str>) -> io::Result<Vec<NeighborEntry>> {
    let filter = interface.map(interface_index).transpose()?;
    let raw_fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            libc::NETLINK_ROUTE,
        )
    };
    if raw_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SOCK_RAW here is a netlink socket; it is not a privileged IP raw socket.
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let mut local: libc::sockaddr_nl = unsafe { mem::zeroed() };
    local.nl_family = libc::AF_NETLINK as _;
    if unsafe {
        libc::bind(
            fd.as_raw_fd(),
            (&local as *const libc::sockaddr_nl).cast(),
            mem::size_of_val(&local) as _,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut kernel: libc::sockaddr_nl = unsafe { mem::zeroed() };
    kernel.nl_family = libc::AF_NETLINK as _;
    let mut request = [0u8; HEADER_LEN + NDMSG_LEN];
    request[..4].copy_from_slice(&((HEADER_LEN + NDMSG_LEN) as u32).to_ne_bytes());
    request[4..6].copy_from_slice(&RTM_GETNEIGH.to_ne_bytes());
    request[6..8].copy_from_slice(&(NLM_F_REQUEST | NLM_F_DUMP).to_ne_bytes());
    request[8..12].copy_from_slice(&SEQUENCE.to_ne_bytes());
    request[HEADER_LEN] = libc::AF_INET6 as u8;
    let sent = unsafe {
        libc::sendto(
            fd.as_raw_fd(),
            request.as_ptr().cast(),
            request.len(),
            0,
            (&kernel as *const libc::sockaddr_nl).cast(),
            mem::size_of_val(&kernel) as _,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != request.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "Incomplete netlink request",
        ));
    }
    let start = Instant::now();
    let mut entries = Vec::new();
    loop {
        let data = receive(&fd, start)?;
        let (done, batch) = parse_datagram(&data, SEQUENCE)?;
        entries.extend(batch);
        if done {
            break;
        }
    }
    let mut result: Vec<_> = entries
        .into_iter()
        .filter(|entry| filter.is_none_or(|index| entry.interface_index == index))
        .map(|entry| NeighborEntry {
            address: entry.address,
            interface: interface_name(entry.interface_index),
            interface_index: entry.interface_index,
            scope_id: if entry.address.is_unicast_link_local() {
                entry.interface_index
            } else {
                0
            },
            mac: entry.mac,
            states: state_names(entry.state_bits),
            state_bits: entry.state_bits,
            router: entry.flags & 0x80 != 0,
            proxy: entry.flags & 0x08 != 0,
        })
        .collect();
    result.sort_by_key(|entry| (entry.interface_index, entry.address));
    Ok(result)
}

fn receive(fd: &OwnedFd, start: Instant) -> io::Result<Vec<u8>> {
    loop {
        let remaining = DUMP_TIMEOUT
            .checked_sub(start.elapsed())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Neighbor dump timed out"))?;
        let mut poll = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let milliseconds = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut poll, 1, milliseconds) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        if poll.revents & (libc::POLLNVAL | libc::POLLHUP) != 0 {
            return Err(invalid("Netlink socket closed unexpectedly"));
        }
        let mut peek = [0u8; 1];
        let length = unsafe {
            libc::recv(
                fd.as_raw_fd(),
                peek.as_mut_ptr().cast(),
                1,
                libc::MSG_PEEK | libc::MSG_TRUNC | libc::MSG_DONTWAIT,
            )
        };
        if length < 0 {
            let error = io::Error::last_os_error();
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return Err(error);
        }
        if length == 0 || length as usize > 16 * 1024 * 1024 {
            return Err(invalid("Invalid netlink datagram size"));
        }
        let mut data = vec![0u8; length as usize];
        let mut sender: libc::sockaddr_nl = unsafe { mem::zeroed() };
        let mut sender_length = mem::size_of_val(&sender) as libc::socklen_t;
        let received = unsafe {
            libc::recvfrom(
                fd.as_raw_fd(),
                data.as_mut_ptr().cast(),
                data.len(),
                libc::MSG_TRUNC | libc::MSG_DONTWAIT,
                (&mut sender as *mut libc::sockaddr_nl).cast(),
                &mut sender_length,
            )
        };
        if received < 0 {
            let error = io::Error::last_os_error();
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return Err(error);
        }
        if received as usize != data.len() {
            return Err(invalid("Truncated netlink datagram"));
        }
        if sender_length as usize != mem::size_of_val(&sender)
            || sender.nl_family as i32 != libc::AF_NETLINK
            || sender.nl_pid != 0
        {
            return Err(invalid("Netlink reply did not originate from the kernel"));
        }
        return Ok(data);
    }
}

fn u16_ne(bytes: &[u8]) -> u16 {
    u16::from_ne_bytes([bytes[0], bytes[1]])
}
fn u32_ne(bytes: &[u8]) -> u32 {
    u32::from_ne_bytes(bytes[..4].try_into().unwrap())
}
fn i32_ne(bytes: &[u8]) -> i32 {
    i32::from_ne_bytes(bytes[..4].try_into().unwrap())
}

fn advance(data: &[u8], length: usize) -> io::Result<&[u8]> {
    let aligned = (length + 3) & !3;
    if aligned <= data.len() {
        Ok(&data[aligned..])
    } else if length == data.len() {
        Ok(&[])
    } else {
        Err(invalid("Missing netlink alignment padding"))
    }
}

fn parse_datagram(mut data: &[u8], sequence: u32) -> io::Result<(bool, Vec<RawNeighbor>)> {
    let mut result = Vec::new();
    let mut done = false;
    while !data.is_empty() {
        if data.len() < HEADER_LEN {
            return Err(invalid("Truncated netlink header"));
        }
        let length = u32_ne(data) as usize;
        if length < HEADER_LEN || length > data.len() {
            return Err(invalid("Invalid netlink message length"));
        }
        let kind = u16_ne(&data[4..]);
        let flags = u16_ne(&data[6..]);
        let message_sequence = u32_ne(&data[8..]);
        let payload = &data[HEADER_LEN..length];
        if message_sequence == sequence {
            if flags & NLM_F_DUMP_INTR != 0 {
                return Err(invalid(
                    "Neighbor dump changed during retrieval; retry the command",
                ));
            }
            match kind {
                NLMSG_ERROR => {
                    if payload.len() < 4 {
                        return Err(invalid("Truncated netlink error"));
                    }
                    check_error(i32_ne(payload))?;
                }
                NLMSG_DONE => {
                    if !payload.is_empty() {
                        if payload.len() < 4 {
                            return Err(invalid("Truncated dump completion"));
                        }
                        check_error(i32_ne(payload))?;
                    }
                    done = true;
                }
                NLMSG_OVERRUN => return Err(invalid("Neighbor dump overflow; retry the command")),
                RTM_NEWNEIGH => {
                    if let Some(entry) = parse_neighbor(payload)? {
                        result.push(entry);
                    }
                }
                _ => {}
            }
        }
        data = advance(data, length)?;
    }
    Ok((done, result))
}

fn check_error(code: i32) -> io::Result<()> {
    if code == 0 {
        return Ok(());
    }
    let errno = code
        .checked_neg()
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid("Invalid netlink error code"))?;
    Err(io::Error::from_raw_os_error(errno))
}

fn parse_neighbor(payload: &[u8]) -> io::Result<Option<RawNeighbor>> {
    if payload.len() < NDMSG_LEN {
        return Err(invalid("Truncated neighbor message"));
    }
    if payload[0] != libc::AF_INET6 as u8 {
        return Ok(None);
    }
    let index = i32_ne(&payload[4..]);
    if index <= 0 {
        return Err(invalid("Invalid neighbor interface index"));
    }
    let state_bits = u16_ne(&payload[8..]);
    let flags = payload[10];
    let mut attributes = &payload[NDMSG_LEN..];
    let mut address = None;
    let mut mac = None;
    while !attributes.is_empty() {
        if attributes.len() < 4 {
            return Err(invalid("Truncated neighbor attribute"));
        }
        let length = u16_ne(attributes) as usize;
        if length < 4 || length > attributes.len() {
            return Err(invalid("Invalid neighbor attribute length"));
        }
        let kind = u16_ne(&attributes[2..]) & 0x3fff;
        let value = &attributes[4..length];
        match kind {
            1 => {
                if value.len() != 16 || address.is_some() {
                    return Err(invalid("Invalid IPv6 neighbor address"));
                }
                address = Some(Ipv6Addr::from(<[u8; 16]>::try_from(value).unwrap()));
            }
            2 if !value.is_empty() => {
                mac = Some(
                    value
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join(":"),
                );
            }
            _ => {}
        }
        attributes = advance(attributes, length)?;
    }
    let address = address.ok_or_else(|| invalid("IPv6 neighbor entry lacks an address"))?;
    Ok(Some(RawNeighbor {
        address,
        interface_index: index as u32,
        mac,
        state_bits,
        flags,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute(kind: u16, value: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((4 + value.len()) as u16).to_ne_bytes());
        bytes.extend_from_slice(&kind.to_ne_bytes());
        bytes.extend_from_slice(value);
        bytes.resize((bytes.len() + 3) & !3, 0);
        bytes
    }
    fn payload(state: u16, mac: bool) -> Vec<u8> {
        let mut bytes = vec![0; NDMSG_LEN];
        bytes[0] = libc::AF_INET6 as u8;
        bytes[4..8].copy_from_slice(&2i32.to_ne_bytes());
        bytes[8..10].copy_from_slice(&state.to_ne_bytes());
        bytes.extend(attribute(
            1,
            &"fe80::1234".parse::<Ipv6Addr>().unwrap().octets(),
        ));
        if mac {
            bytes.extend(attribute(2, &[0x48, 0x4d, 0x7e, 0xe8, 0xfc, 0xa4]));
        }
        bytes.extend(attribute(99, &[1, 2, 3]));
        bytes
    }
    fn message(kind: u16, flags: u16, sequence: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_LEN];
        bytes[..4].copy_from_slice(&((HEADER_LEN + payload.len()) as u32).to_ne_bytes());
        bytes[4..6].copy_from_slice(&kind.to_ne_bytes());
        bytes[6..8].copy_from_slice(&flags.to_ne_bytes());
        bytes[8..12].copy_from_slice(&sequence.to_ne_bytes());
        bytes.extend_from_slice(payload);
        bytes.resize((bytes.len() + 3) & !3, 0);
        bytes
    }
    #[test]
    fn parses_address_mac_state_and_unknown_attribute() {
        let entry = parse_neighbor(&payload(4, true)).unwrap().unwrap();
        assert_eq!(entry.address, "fe80::1234".parse::<Ipv6Addr>().unwrap());
        assert_eq!(entry.mac.as_deref(), Some("48:4d:7e:e8:fc:a4"));
        assert_eq!(state_names(entry.state_bits), ["STALE"]);
    }
    #[test]
    fn preserves_failed_and_incomplete_without_mac() {
        for state in [1, 32] {
            let entry = parse_neighbor(&payload(state, false)).unwrap().unwrap();
            assert!(entry.mac.is_none());
            assert_eq!(entry.state_bits, state);
        }
        assert_eq!(state_names(0), ["NONE"]);
        assert_eq!(state_names(0x102), ["REACHABLE", "UNKNOWN(0x100)"]);
    }
    #[test]
    fn parses_multipart_and_sequence_filtering() {
        let mut bytes = message(RTM_NEWNEIGH, 2, 99, &payload(2, true));
        bytes.extend(message(RTM_NEWNEIGH, 2, SEQUENCE, &payload(2, true)));
        bytes.extend(message(NLMSG_DONE, 2, SEQUENCE, &0i32.to_ne_bytes()));
        let (done, entries) = parse_datagram(&bytes, SEQUENCE).unwrap();
        assert!(done);
        assert_eq!(entries.len(), 1);
        assert!(
            parse_datagram(&message(NLMSG_DONE, 2, SEQUENCE, &[]), SEQUENCE)
                .unwrap()
                .0
        );
    }
    #[test]
    fn rejects_errors_and_interrupted_dumps() {
        let error = parse_datagram(
            &message(NLMSG_ERROR, 0, SEQUENCE, &(-libc::EPERM).to_ne_bytes()),
            SEQUENCE,
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        assert!(
            parse_datagram(
                &message(NLMSG_DONE, NLM_F_DUMP_INTR, SEQUENCE, &[]),
                SEQUENCE
            )
            .is_err()
        );
        assert!(parse_datagram(&message(NLMSG_OVERRUN, 0, SEQUENCE, &[]), SEQUENCE).is_err());
    }
    #[test]
    fn rejects_truncated_or_malformed_lengths() {
        assert!(parse_datagram(&[0; 15], SEQUENCE).is_err());
        let mut p = payload(2, true);
        p[12..14].copy_from_slice(&3u16.to_ne_bytes());
        assert!(parse_neighbor(&p).is_err());
        assert!(parse_neighbor(&payload(2, true)[..31]).is_err());
        assert!(parse_neighbor(&[0; 11]).is_err());
        let mut p = payload(2, false);
        p.extend(attribute(1, &[0; 16]));
        assert!(parse_neighbor(&p).is_err());
    }
    #[test]
    fn ignores_non_ipv6_entries() {
        let mut p = payload(2, true);
        p[0] = libc::AF_INET as u8;
        assert!(parse_neighbor(&p).unwrap().is_none());
    }
}
