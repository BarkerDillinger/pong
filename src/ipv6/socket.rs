// src/ipv6/socket.rs
// IPv6 socket utilities for sending and receiving ICMPv6 packets.
use std::io;
use std::mem;
use std::net::{Ipv6Addr, SocketAddrV6};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use crate::ipv4::errors::classify_network_error;
use crate::types::PingResult;

#[repr(align(16))]
struct ControlBuffer([u8; 256]);

pub fn create_ping_socket() -> io::Result<OwnedFd> {
    let raw = unsafe {
        libc::socket(
            libc::AF_INET6,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::IPPROTO_ICMPV6,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    for option in [libc::IPV6_RECVHOPLIMIT, libc::IPV6_RECVERR] {
        let enabled: libc::c_int = 1;
        if unsafe {
            libc::setsockopt(
                raw,
                libc::IPPROTO_IPV6,
                option,
                &enabled as *const _ as *const libc::c_void,
                mem::size_of_val(&enabled) as libc::socklen_t,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    let destination = sockaddr(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));
    // Bind an ephemeral ping identifier; the kernel rewrites outgoing headers.
    if unsafe {
        libc::bind(
            raw,
            &destination as *const _ as *const libc::sockaddr,
            mem::size_of_val(&destination) as libc::socklen_t,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

fn sockaddr(target: SocketAddrV6) -> libc::sockaddr_in6 {
    libc::sockaddr_in6 {
        sin6_family: libc::AF_INET6 as libc::sa_family_t,
        sin6_port: 0,
        sin6_flowinfo: 0,
        sin6_addr: libc::in6_addr {
            s6_addr: target.ip().octets(),
        },
        sin6_scope_id: target.scope_id(),
    }
}

pub fn send_echo_request(fd: i32, target: SocketAddrV6, packet: &[u8]) -> io::Result<()> {
    let destination = sockaddr(target);
    let sent = unsafe {
        libc::sendto(
            fd,
            packet.as_ptr().cast(),
            packet.len(),
            0,
            (&destination as *const libc::sockaddr_in6).cast(),
            mem::size_of_val(&destination) as _,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != packet.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "Incomplete ICMPv6 transmission",
        ));
    }
    Ok(())
}

pub fn wait_for_reply(
    fd: &OwnedFd,
    target: SocketAddrV6,
    sequence: u16,
    expected: &[u8],
    start: Instant,
    timeout: Duration,
) -> PingResult {
    let raw = fd.as_raw_fd();
    let mut local: libc::sockaddr_in6 = unsafe { mem::zeroed() };
    let mut length = mem::size_of_val(&local) as libc::socklen_t;
    if unsafe {
        libc::getsockname(
            raw,
            (&mut local as *mut libc::sockaddr_in6).cast(),
            &mut length,
        )
    } < 0
    {
        return classify_network_error(io::Error::last_os_error());
    }
    let identifier = u16::from_be(local.sin6_port).to_be_bytes();
    loop {
        let remaining = match timeout.checked_sub(start.elapsed()) {
            Some(left) if !left.is_zero() => left,
            _ => return PingResult::NoResponse,
        };
        let mut pollfd = libc::pollfd {
            fd: raw,
            events: libc::POLLIN | libc::POLLERR,
            revents: 0,
        };
        let milliseconds = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let polled = unsafe { libc::poll(&mut pollfd, 1, milliseconds) };
        if polled < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return classify_network_error(error);
        }
        if polled == 0 {
            continue;
        }
        if pollfd.revents & libc::POLLERR != 0
            && let Some((seq, result)) = read_error_queue(raw)
        {
            if seq == sequence {
                let mut result = result;
                if let PingResult::TimeExceeded { rtt, .. } = &mut result {
                    *rtt = Some(start.elapsed());
                }
                return result;
            }
            continue;
        }
        let mut buffer = [0u8; 65535];
        let mut source: libc::sockaddr_in6 = unsafe { mem::zeroed() };
        let mut control = ControlBuffer([0; 256]);
        let mut iov = libc::iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: buffer.len(),
        };
        let mut message: libc::msghdr = unsafe { mem::zeroed() };
        message.msg_name = (&mut source as *mut libc::sockaddr_in6).cast();
        message.msg_namelen = mem::size_of_val(&source) as libc::socklen_t;
        message.msg_iov = &mut iov;
        message.msg_iovlen = 1;
        message.msg_control = control.0.as_mut_ptr().cast();
        message.msg_controllen = control.0.len();
        let received = unsafe { libc::recvmsg(raw, &mut message, libc::MSG_DONTWAIT) };
        if received < 0 {
            let error = io::Error::last_os_error();
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return classify_network_error(error);
        }
        let packet = &buffer[..received as usize];
        if message.msg_flags & libc::MSG_TRUNC != 0
            || packet.len() != expected.len()
            || packet.len() < 8
            || packet[0] != 129
            || packet[1] != 0
            || Ipv6Addr::from(source.sin6_addr.s6_addr) != *target.ip()
            || (target.ip().is_unicast_link_local()
                && target.scope_id() != 0
                && source.sin6_scope_id != target.scope_id())
            || packet[4..6] != identifier
            || packet[6..8] != sequence.to_be_bytes()
            || packet[8..] != expected[8..]
        {
            continue;
        }
        let mut hop_limit = None;
        unsafe {
            let mut cmsg = libc::CMSG_FIRSTHDR(&message);
            while !cmsg.is_null() {
                if (*cmsg).cmsg_level == libc::IPPROTO_IPV6
                    && (*cmsg).cmsg_type == libc::IPV6_HOPLIMIT
                    && (*cmsg).cmsg_len
                        >= libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize
                {
                    hop_limit = u8::try_from(std::ptr::read_unaligned(
                        libc::CMSG_DATA(cmsg).cast::<libc::c_int>(),
                    ))
                    .ok();
                }
                cmsg = libc::CMSG_NXTHDR(&message, cmsg);
            }
        }
        return PingResult::Alive {
            rtt: start.elapsed(),
            ttl: hop_limit,
        };
    }
}

fn read_error_queue(fd: i32) -> Option<(u16, PingResult)> {
    let mut data = [0u8; 65535];
    let mut control = ControlBuffer([0; 256]);
    let mut iov = libc::iovec {
        iov_base: data.as_mut_ptr().cast(),
        iov_len: data.len(),
    };
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = control.0.len();
    let received = unsafe { libc::recvmsg(fd, &mut msg, libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT) };
    if received < 8 {
        return None;
    }
    let sequence = u16::from_be_bytes([data[6], data[7]]);
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            let base_len =
                libc::CMSG_LEN(mem::size_of::<libc::sock_extended_err>() as u32) as usize;
            if (*cmsg).cmsg_level == libc::IPPROTO_IPV6
                && (*cmsg).cmsg_type == libc::IPV6_RECVERR
                && (*cmsg).cmsg_len >= base_len
            {
                let ptr = libc::CMSG_DATA(cmsg).cast::<libc::sock_extended_err>();
                let error = std::ptr::read_unaligned(ptr);
                if error.ee_origin == libc::SO_EE_ORIGIN_LOCAL {
                    return Some((
                        sequence,
                        classify_network_error(io::Error::from_raw_os_error(error.ee_errno as i32)),
                    ));
                }
                if error.ee_origin != libc::SO_EE_ORIGIN_ICMP6 {
                    return None;
                }
                let from = if (*cmsg).cmsg_len >= base_len + mem::size_of::<libc::sockaddr_in6>() {
                    let offender = std::ptr::read_unaligned(
                        libc::SO_EE_OFFENDER(ptr).cast::<libc::sockaddr_in6>(),
                    );
                    if offender.sin6_family as i32 == libc::AF_INET6 {
                        Some(Ipv6Addr::from(offender.sin6_addr.s6_addr).into())
                    } else {
                        None
                    }
                } else {
                    None
                };
                let result = match (error.ee_type, error.ee_code) {
                    (1, 0) => PingResult::NetworkUnreachable { from },
                    (1, 1) | (1, 5) | (1, 6) => PingResult::AdministrativelyProhibited { from },
                    (1, 2) | (1, 3) => PingResult::HostUnreachable { from },
                    (1, 4) => PingResult::PortUnreachable { from },
                    (2, 0) => PingResult::PacketTooBig {
                        from,
                        mtu: Some(error.ee_info),
                    },
                    (3, _) => PingResult::TimeExceeded { from, rtt: None },
                    (4, _) => PingResult::ParameterProblem { from },
                    _ => PingResult::IcmpError {
                        from,
                        icmp_type: error.ee_type,
                        icmp_code: error.ee_code,
                    },
                };
                return Some((sequence, result));
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    // Exercise recvmsg, ancillary data, matching and deadlines using ordinary
    // IPv6 datagrams. This does not test kernel ICMPv6 ping socket behavior.
    fn receiver() -> (UdpSocket, OwnedFd, SocketAddrV6) {
        let socket = UdpSocket::bind("[::1]:0").unwrap();
        let fd: OwnedFd = socket.try_clone().unwrap().into();
        let enable: libc::c_int = 1;
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    fd.as_raw_fd(),
                    libc::IPPROTO_IPV6,
                    libc::IPV6_RECVHOPLIMIT,
                    (&enable as *const libc::c_int).cast(),
                    mem::size_of_val(&enable) as _,
                )
            },
            0
        );
        (socket, fd, SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0))
    }

    #[test]
    fn rejects_unrelated_packets_then_accepts_matching_reply() {
        let (socket, fd, target) = receiver();
        let sender = UdpSocket::bind("[::1]:0").unwrap();
        let expected = crate::ipv6::packet::build_echo_request(7, 4);
        let mut reply = expected.clone();
        reply[0] = 129;
        reply[4..6].copy_from_slice(&socket.local_addr().unwrap().port().to_be_bytes());
        let mut wrong = reply.clone();
        wrong[7] = 8;
        sender
            .send_to(&wrong, socket.local_addr().unwrap())
            .unwrap();
        wrong = reply.clone();
        wrong[8] ^= 1;
        sender
            .send_to(&wrong, socket.local_addr().unwrap())
            .unwrap();
        wrong = reply.clone();
        wrong[4] ^= 1;
        sender
            .send_to(&wrong, socket.local_addr().unwrap())
            .unwrap();
        sender
            .send_to(&reply, socket.local_addr().unwrap())
            .unwrap();
        let result = wait_for_reply(
            &fd,
            target,
            7,
            &expected,
            Instant::now(),
            Duration::from_millis(100),
        );
        assert!(matches!(result, PingResult::Alive { ttl: Some(_), .. }));
    }

    #[test]
    fn unrelated_traffic_does_not_extend_deadline() {
        let (socket, fd, target) = receiver();
        let sender = UdpSocket::bind("[::1]:0").unwrap();
        let destination = socket.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            for _ in 0..30 {
                sender
                    .send_to(&[129, 0, 0, 0, 0, 0, 0, 8], destination)
                    .unwrap();
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let start = Instant::now();
        let result = wait_for_reply(
            &fd,
            target,
            7,
            &crate::ipv6::packet::build_echo_request(7, 0),
            start,
            Duration::from_millis(30),
        );
        assert!(matches!(result, PingResult::NoResponse));
        assert!(start.elapsed() < Duration::from_millis(130));
        worker.join().unwrap();
    }
}
