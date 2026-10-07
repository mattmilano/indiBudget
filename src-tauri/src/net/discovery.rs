//! Finding a hosting computer on the home network without typing its address.
//!
//! The host announces itself over multicast DNS (DNS-SD, the same mechanism
//! printers and AirPlay speakers use), and a joining computer lists what it
//! hears as "indiBudget on STUDY-DESKTOP" rather than asking for a number.
//!
//! Nothing heard here is trusted. The announced identity is a hint for the
//! screen; pairing is still what proves which computer was reached, because
//! the pairing proof is bound to the certificate actually presented.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use super::addresses::reachable_ips;
use super::identity::Fingerprint;

/// The DNS-SD service type indiBudget hosts announce.
pub const SERVICE_TYPE: &str = "_indibudget._tcp.local.";

/// How long a joining computer listens before showing what it found. Hosts
/// answer a query within a second on a quiet home network.
pub const LISTEN_FOR: Duration = Duration::from_secs(2);

/// This computer's name, as people know it.
pub fn computer_name() -> String {
    let name = gethostname::gethostname().to_string_lossy().trim().to_string();
    let name = name.trim_end_matches(".local").to_string();
    if name.is_empty() {
        "This computer".into()
    } else {
        name
    }
}

/// A name usable as an mDNS host label: letters, digits and hyphens.
fn host_label(name: &str) -> String {
    let label: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let label = label.trim_matches('-');
    if label.is_empty() {
        "indibudget-host".into()
    } else {
        label.to_string()
    }
}

/// A host announcing itself. Dropping it withdraws the announcement.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    /// Announce a host listening on `port`.
    ///
    /// Failure is not fatal to hosting — a network that blocks multicast still
    /// lets people type the address — so callers show the error, if anything,
    /// and carry on.
    pub fn start(port: u16, fingerprint: &Fingerprint) -> Result<Self, String> {
        let computer = computer_name();
        let ips = reachable_ips();
        if ips.is_empty() {
            return Err("This computer has no network address to announce.".into());
        }

        let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
        // Named by identity rather than by computer name, so two computers
        // with the same name (two family laptops, both "MacBook-Air") do not
        // collide.
        let hex = fingerprint.to_hex();
        let instance = format!("indiBudget {}", &hex[..12.min(hex.len())]);
        let host_name = format!("{}.local.", host_label(&computer));
        let properties = [("computer", computer.as_str()), ("fp", hex.as_str())];
        let ips: Vec<std::net::IpAddr> = ips.into_iter().map(Into::into).collect();

        let info = ServiceInfo::new(SERVICE_TYPE, &instance, &host_name, &ips[..], port, &properties[..])
            .map_err(|e| e.to_string())?;
        let fullname = info.get_fullname().to_string();
        daemon.register(info).map_err(|e| e.to_string())?;
        Ok(Advertisement { daemon, fullname })
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        // Say goodbye so joining computers stop listing this one at once,
        // rather than when the announcement's lifetime runs out.
        if let Ok(done) = self.daemon.unregister(&self.fullname) {
            let _ = done.recv_timeout(Duration::from_secs(1));
        }
        let _ = self.daemon.shutdown();
    }
}

/// A host heard on the network.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FoundHost {
    /// The hosting computer's name.
    pub computer: String,
    /// Where to reach it, ready for the address box.
    pub address: String,
    /// The identity code it announced, grouped for reading aloud. A hint only:
    /// pairing proves it.
    pub fingerprint_groups: String,
}

/// Prefer an address on a private home network, as people's routers hand out.
fn best_address(ips: impl IntoIterator<Item = Ipv4Addr>) -> Option<Ipv4Addr> {
    let mut ips: Vec<Ipv4Addr> = ips
        .into_iter()
        .filter(|ip| !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified())
        .collect();
    ips.sort_by_key(|ip| (!ip.is_private(), *ip));
    ips.into_iter().next()
}

/// Listen for hosts for `wait`, and list what answered, one entry per host.
pub fn browse(wait: Duration) -> Result<Vec<FoundHost>, String> {
    let daemon = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let events = daemon.browse(SERVICE_TYPE).map_err(|e| e.to_string())?;

    // Keyed by identity: a host answering on two interfaces is one host.
    let mut found: HashMap<String, FoundHost> = HashMap::new();
    let deadline = Instant::now() + wait;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        let ServiceEvent::ServiceResolved(service) = event else {
            continue;
        };
        let Some(fp) = service.get_property_val_str("fp").map(str::to_string) else {
            continue;
        };
        let Some(ip) = best_address(service.get_addresses_v4()) else {
            continue;
        };
        let computer = service
            .get_property_val_str("computer")
            .map(str::to_string)
            .unwrap_or_else(|| service.get_hostname().trim_end_matches(".local.").to_string());
        let fingerprint_groups = Fingerprint::from_hex(&fp)
            .map(|f| f.display_groups())
            .unwrap_or_default();
        found.insert(
            fp,
            FoundHost {
                computer,
                address: SocketAddr::from((ip, service.get_port())).to_string(),
                fingerprint_groups,
            },
        );
    }

    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.shutdown();

    let mut hosts: Vec<FoundHost> = found.into_values().collect();
    hosts.sort_by(|a, b| a.computer.cmp(&b.computer));
    Ok(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_labels_are_safe_for_mdns() {
        assert_eq!(host_label("Sam's MacBook Air"), "Sam-s-MacBook-Air");
        assert_eq!(host_label("STUDY-DESKTOP"), "STUDY-DESKTOP");
        assert_eq!(host_label("!!!"), "indibudget-host");
    }

    #[test]
    fn a_home_network_address_is_preferred() {
        let chosen = best_address([
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(169, 254, 3, 4),
            Ipv4Addr::new(100, 64, 0, 9),
            Ipv4Addr::new(192, 168, 1, 20),
        ]);
        assert_eq!(chosen, Some(Ipv4Addr::new(192, 168, 1, 20)));
        assert_eq!(best_address([Ipv4Addr::LOCALHOST]), None);
    }
}
