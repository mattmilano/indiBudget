//! A host announces itself and a joining computer hears it, over the real
//! network stack.
//!
//! Multicast is not available everywhere tests run (some containers and CI
//! runners have none), so this is ignored by default. Run it on a real
//! computer with `cargo test --test discovery -- --ignored`.

use indibudget_lib::net::discovery::{browse, Advertisement};
use indibudget_lib::net::identity::HostIdentity;
use std::time::Duration;

#[test]
#[ignore = "needs a network that carries multicast"]
fn a_hosting_computer_can_be_found_on_the_network() {
    let identity = HostIdentity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let _advert = Advertisement::start(7499, &fingerprint).expect("announce");

    let found = browse(Duration::from_secs(3)).expect("listen");
    let me = found
        .iter()
        .find(|h| h.fingerprint_groups == fingerprint.display_groups())
        .unwrap_or_else(|| panic!("not heard; found {found:?}"));
    assert!(me.address.ends_with(":7499"), "{}", me.address);
}
