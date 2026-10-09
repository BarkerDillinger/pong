// src/modes/ping.rs
use std::io;
use std::net::SocketAddr;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::Cli;

use crate::ipv4::errors::classify_network_error;
use crate::ipv4::packet::build_echo_request;
use crate::ipv4::socket::{create_ping_socket, send_echo_request, wait_for_reply};

use crate::json::{JsonPingOutput, JsonPingProbe, ping_result_to_json};

use crate::output::print_result_family;

use crate::types::{PingResult, PingStats};

pub fn run(
    cli: &Cli,
    target_name: &str,
    target: SocketAddr,
    target_display: &str,
    timeout: Duration,
    interval: Duration,
) -> io::Result<()> {
    let socket = match target {
        SocketAddr::V4(_) => {
            create_ping_socket(timeout).map(|raw| unsafe { OwnedFd::from_raw_fd(raw) })
        }
        SocketAddr::V6(_) => crate::ipv6::socket::create_ping_socket(),
    };
    let fd = match socket {
        Ok(fd) => fd,
        Err(error) => {
            eprintln!("pong: unable to create ping socket: {error}");
            if error.kind() == io::ErrorKind::PermissionDenied {
                eprintln!(
                    "Check: sysctl net.ipv4.ping_group_range (controls IPv4 and IPv6 ping sockets)"
                );
            }
            std::process::exit(3);
        }
    };

    let running = Arc::new(AtomicBool::new(true));

    let handler_running = Arc::clone(&running);

    ctrlc::set_handler(move || {
        handler_running.store(false, Ordering::SeqCst);
    })
    .expect("Unable to install Ctrl+C handler");

    let requested_count = cli.count.unwrap_or(1);

    let show_statistics = cli.continuous || cli.count.is_some();

    let mut attempts: u32 = 0;
    let mut sequence: u16 = 1;

    let mut stats = PingStats::new();

    let mut json_probes: Vec<JsonPingProbe> = Vec::new();

    while running.load(Ordering::SeqCst) {
        if !cli.continuous && attempts >= requested_count {
            break;
        }

        attempts += 1;

        let packet = match target {
            SocketAddr::V4(_) => build_echo_request(sequence, cli.size),
            SocketAddr::V6(_) => crate::ipv6::packet::build_echo_request(sequence, cli.size),
        };

        let start = Instant::now();

        let sent = match target {
            SocketAddr::V4(address) => send_echo_request(fd.as_raw_fd(), *address.ip(), &packet),
            SocketAddr::V6(address) => {
                crate::ipv6::socket::send_echo_request(fd.as_raw_fd(), address, &packet)
            }
        };
        let result = match sent {
            Ok(()) => {
                stats.transmitted += 1;
                match target {
                    SocketAddr::V4(address) => {
                        wait_for_reply(fd.as_raw_fd(), *address.ip(), sequence, start)
                    }
                    SocketAddr::V6(address) => crate::ipv6::socket::wait_for_reply(
                        &fd, address, sequence, &packet, start, timeout,
                    ),
                }
            }
            Err(error) => classify_network_error(error),
        };

        if cli.json {
            json_probes.push({
                let mut probe = ping_result_to_json(sequence, &result);
                if target.is_ipv6() {
                    probe.hop_limit = probe.ttl.take();
                }
                probe
            });
        } else {
            print_result_family(target_display, sequence, &result, timeout, target.is_ipv6());
        }

        match &result {
            PingResult::Alive { rtt, ttl } => {
                stats.record_reply(*rtt, *ttl);
            }

            PingResult::NoResponse => {}

            _ => {
                stats.errors += 1;
            }
        }

        sequence = sequence.wrapping_add(1);

        if !cli.continuous && attempts >= requested_count {
            break;
        }

        if !running.load(Ordering::SeqCst) {
            break;
        }

        thread::sleep(interval);
    }

    drop(fd);

    if cli.json {
        let output = JsonPingOutput {
            schema_version: 1,
            mode: "ping",
            target: target_name.to_string(),
            address: target.ip(),
            scope_id: match target {
                SocketAddr::V6(a) if a.scope_id() != 0 => Some(a.scope_id()),
                _ => None,
            },
            transmitted: stats.transmitted,
            received: stats.received,
            probes: json_probes,
        };

        println!(
            "{}",
            serde_json::to_string_pretty(&output).expect("Unable to serialize JSON output")
        );
    } else if show_statistics {
        stats.print_family(target_display, target.is_ipv6());
    }

    if stats.received == 0 {
        std::process::exit(1);
    }

    Ok(())
}
