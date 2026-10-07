//! Which address to listen on, and which addresses to tell people to use.
//!
//! Hosting listens on `0.0.0.0` — every network card — but that is not an
//! address another computer can dial: from the joining computer it means
//! "myself", and the result is "connection refused". The hosting screen once
//! showed exactly that address. What it must show is this machine's address on
//! the home network, which is what this module works out.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

/// The port hosting uses unless someone chooses another.
///
/// Fixed rather than picked at random, because a random port changes every time
/// hosting starts: a firewall rule for it stops matching, and computers that
/// already joined can no longer find the host.
pub const DEFAULT_PORT: u16 = 7420;

/// Interface-name prefixes for virtual networks that other computers on the
/// home network cannot reach — container bridges, VM host-only adapters.
/// Showing their addresses would offer people a choice that can only fail.
const VIRTUAL_PREFIXES: &[&str] = &[
    "docker", "br-", "veth", "virbr", "vboxnet", "vmnet", "podman", "cni", "flannel", "lxc",
    "lxd", "vEthernet",
];

fn is_virtual(interface: &str) -> bool {
    VIRTUAL_PREFIXES.iter().any(|p| interface.starts_with(p))
}

fn is_private(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

/// From every (interface name, address) pair on this machine, the addresses
/// another computer on the network could plausibly use, best first.
///
/// Pure so it can be tested without depending on the machine running the test.
pub fn usable_addresses(interfaces: impl IntoIterator<Item = (String, IpAddr)>) -> Vec<Ipv4Addr> {
    let mut out: Vec<Ipv4Addr> = interfaces
        .into_iter()
        .filter(|(name, _)| !is_virtual(name))
        .filter_map(|(_, ip)| match ip {
            // IPv4 only: an address someone reads off one screen and types into
            // another should be the familiar four-number kind.
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .filter(|ip| !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified())
        .collect();

    out.sort_by_key(|ip| (!is_private(*ip), *ip));
    out.dedup();
    out
}

/// This machine's addresses on the network, with the port, ready to show.
pub fn reachable_addresses(port: u16) -> Vec<String> {
    let interfaces = if_addrs::get_if_addrs()
        .map(|list| list.into_iter().map(|i| (i.name.clone(), i.ip())).collect::<Vec<_>>())
        .unwrap_or_default();
    usable_addresses(interfaces)
        .into_iter()
        .map(|ip| format!("{ip}:{port}"))
        .collect()
}

/// Read an address a person typed on the joining computer.
///
/// Accepts `192.168.1.20:7420`, `192.168.1.20` (the default port is assumed),
/// or a host name such as `family-desktop.local`. Refuses the listening
/// address, which can only ever reach the joining computer itself.
pub fn parse_host_address(input: &str) -> Result<SocketAddr, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter the address shown on the computer hosting the budget.".into());
    }

    let with_port = if input.parse::<SocketAddr>().is_ok() || has_port(input) {
        input.to_string()
    } else {
        format!("{input}:{DEFAULT_PORT}")
    };

    let addr = with_port
        .to_socket_addrs()
        .map_err(|_| {
            format!(
                "\"{input}\" is not an address this computer can find. \
                 It should look like 192.168.1.20:{DEFAULT_PORT}."
            )
        })?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| format!("\"{input}\" has no usable network address."))?;

    if addr.ip().is_unspecified() {
        return Err(format!(
            "{} means \"every network card\" on the host — it is where the host \
             listens, not where to reach it. Use the address listed under \
             \"Others can connect to\" on the host's Sharing screen, such as \
             192.168.1.20:{}.",
            addr.ip(),
            addr.port()
        ));
    }
    Ok(addr)
}

fn has_port(input: &str) -> bool {
    input
        .rsplit_once(':')
        .map(|(host, port)| !host.is_empty() && !host.contains(':') && port.parse::<u16>().is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn the_listening_address_is_never_offered() {
        let found = usable_addresses([("eth0".into(), ip("0.0.0.0"))]);
        assert!(found.is_empty());
    }

    #[test]
    fn loopback_link_local_and_ipv6_are_left_out() {
        let found = usable_addresses([
            ("lo".into(), ip("127.0.0.1")),
            ("eth0".into(), ip("169.254.10.2")),
            ("eth0".into(), ip("fe80::1")),
            ("eth0".into(), ip("192.168.1.20")),
        ]);
        assert_eq!(found, vec!["192.168.1.20".parse::<Ipv4Addr>().unwrap()]);
    }

    /// The case most likely on a developer's or enthusiast's machine: Docker or
    /// a VM manager adds private addresses no other computer can reach.
    #[test]
    fn container_and_vm_bridges_are_left_out() {
        let found = usable_addresses([
            ("docker0".into(), ip("172.17.0.1")),
            ("br-3f2a".into(), ip("172.18.0.1")),
            ("virbr0".into(), ip("192.168.122.1")),
            ("vboxnet0".into(), ip("192.168.56.1")),
            ("wlp2s0".into(), ip("192.168.1.42")),
        ]);
        assert_eq!(found, vec!["192.168.1.42".parse::<Ipv4Addr>().unwrap()]);
    }

    #[test]
    fn home_network_addresses_come_before_public_ones() {
        let found = usable_addresses([
            ("eth1".into(), ip("203.0.113.5")),
            ("eth0".into(), ip("10.0.0.7")),
        ]);
        assert_eq!(found[0], "10.0.0.7".parse::<Ipv4Addr>().unwrap());
    }

    #[test]
    fn a_full_address_is_read_as_typed() {
        assert_eq!(
            parse_host_address("192.168.1.20:7500").unwrap(),
            "192.168.1.20:7500".parse().unwrap()
        );
    }

    #[test]
    fn an_address_without_a_port_gets_the_default_one() {
        assert_eq!(
            parse_host_address(" 192.168.1.20 ").unwrap(),
            format!("192.168.1.20:{DEFAULT_PORT}").parse().unwrap()
        );
    }

    /// The exact mistake that produced "connection refused".
    #[test]
    fn the_listening_address_is_refused_with_an_explanation() {
        let err = parse_host_address("0.0.0.0:38295").unwrap_err();
        assert!(err.contains("every network card"), "{err}");
        assert!(err.contains("Others can connect to"), "{err}");
    }

    #[test]
    fn nonsense_is_refused_with_an_example() {
        let err = parse_host_address("not an address").unwrap_err();
        assert!(err.contains("192.168.1.20"), "{err}");
        assert!(parse_host_address("").is_err());
    }

    #[test]
    fn this_machine_lists_no_unusable_addresses() {
        for shown in reachable_addresses(DEFAULT_PORT) {
            let addr: SocketAddr = shown.parse().unwrap();
            assert!(!addr.ip().is_unspecified() && !addr.ip().is_loopback(), "{shown}");
            assert_eq!(addr.port(), DEFAULT_PORT);
        }
    }
}
