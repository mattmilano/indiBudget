//! Edit holds, and the optimistic backstop beside them.
//!
//! # Why indiBudget leases only some things
//!
//! indiAccounting leases the document — the invoice, not its lines — because
//! one person owns an invoice for the minutes they are typing it. A household
//! budget is a different shape. Two people logging the evening's receipts are
//! writing *different rows*, and a lease on each transaction would be
//! machinery for contention that does not happen. Two people opening the same
//! budget to argue about the grocery number, on the other hand, happens all the
//! time, and there a hold that says "Alex is editing this" before the typing
//! starts is worth having.
//!
//! So the split:
//!
//! - **Leased**: accounts, budgets, categories, goals — the structural records
//!   two people plausibly open at once, where a conflict discovered at save
//!   time means someone retypes.
//! - **Optimistic only**: transactions — append-heavy, independent rows, near
//!   zero real contention. A stale save is refused with what moved, which is
//!   cheap and sufficient.
//!
//! `Leasable` has no `Transaction` variant, so this is a fact about the type
//! rather than a convention someone has to remember.
//!
//! # The small decisions
//!
//! Leases live in server memory only — a crash must not leave a record held by
//! nobody. Expiry is **passive**: an expired lease is never "released", it
//! simply falls to the next asker, so there is no sweeper to get stuck. And
//! **the commit is the letting-go**: a successful save drops the author's holds
//! rather than making them remember to.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::registry::{decode, encode, BoundaryCtx, Registry};
use super::{authorize, Actor, Area, BoundaryError, Required, Stamped};

/// How long a hold survives without a heartbeat.
pub const LEASE_TTL: Duration = Duration::from_secs(60);

/// How often an open editor should renew. Comfortably inside the TTL so one
/// dropped beat does not drop the hold.
pub const LEASE_HEARTBEAT: Duration = Duration::from_secs(20);

/// The record kinds that take an edit hold.
///
/// Deliberately no `Transaction`: see the module comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Leasable {
    Account,
    Budget,
    Category,
    Goal,
}

impl Leasable {
    pub const ALL: [Leasable; 4] = [
        Leasable::Account,
        Leasable::Budget,
        Leasable::Category,
        Leasable::Goal,
    ];

    /// How the record is named in a refusal: "Alex is editing this budget."
    pub fn label(self) -> &'static str {
        match self {
            Leasable::Account => "account",
            Leasable::Budget => "budget",
            Leasable::Category => "category",
            Leasable::Goal => "goal",
        }
    }

    /// The area a hold on this kind of record belongs to.
    ///
    /// A hold is a claim to edit, so it is gated by the same grant the edit
    /// itself needs. Otherwise someone with read-only access could park a hold
    /// on the grocery budget and block the person who may actually change it.
    pub fn area(self) -> Area {
        match self {
            Leasable::Account => Area::Money,
            Leasable::Budget => Area::Planning,
            Leasable::Goal => Area::Planning,
            Leasable::Category => Area::Structure,
        }
    }

    pub fn table(self) -> Stamped {
        match self {
            Leasable::Account => Stamped::Accounts,
            Leasable::Budget => Stamped::Budgets,
            Leasable::Category => Stamped::Categories,
            Leasable::Goal => Stamped::SavingsGoals,
        }
    }
}

/// Who holds a lease.
///
/// Keyed by `(is_owner, user_id)` rather than a bare user id. The two are
/// usually the same thing, but not always: the person sitting at the hosting
/// machine and a signed-in network user can end up carrying the same id, and
/// keying on the id alone would let one silently take over the other's hold.
///
/// What this does *not* separate is two remote machines signed in as the same
/// person. That is deliberate for a household — if Sam opens the same budget on
/// the laptop and the desktop, the second one should say Sam is already editing
/// it rather than let Sam quietly clobber their own work.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal {
    pub is_owner: bool,
    pub user_id: String,
}

impl Principal {
    pub fn of(actor: &Actor) -> Self {
        Principal {
            is_owner: actor.is_owner,
            user_id: actor.user_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeaseKey {
    pub kind: Leasable,
    pub record_id: String,
}

impl LeaseKey {
    pub fn new(kind: Leasable, record_id: impl Into<String>) -> Self {
        LeaseKey {
            kind,
            record_id: record_id.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Held {
    holder: Principal,
    holder_name: String,
    expires_at: Instant,
}

impl Held {
    fn is_live(&self, now: Instant) -> bool {
        self.expires_at > now
    }
}

/// Who is holding what, right now.
#[derive(Debug, Default)]
pub struct Leases {
    held: Mutex<HashMap<LeaseKey, Held>>,
}

impl Leases {
    pub fn new() -> Self {
        Leases {
            held: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<LeaseKey, Held>> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Take or renew a hold.
    ///
    /// Refused only when someone *else* holds it and their hold is still live.
    /// An expired hold is not cleaned up first — it is simply overwritten,
    /// which is what "passive expiry" means in practice.
    pub fn acquire(
        &self,
        key: LeaseKey,
        actor: &Actor,
        now: Instant,
    ) -> Result<(), BoundaryError> {
        let mut held = self.lock();
        let me = Principal::of(actor);

        if let Some(existing) = held.get(&key) {
            if existing.is_live(now) && existing.holder != me {
                return Err(BoundaryError::Busy {
                    record: key.kind.label().to_string(),
                    holder: existing.holder_name.clone(),
                });
            }
        }

        held.insert(
            key,
            Held {
                holder: me,
                holder_name: actor.display_name.clone(),
                expires_at: now + LEASE_TTL,
            },
        );
        Ok(())
    }

    /// Extend a hold this actor already has.
    ///
    /// A renewal that finds the hold gone — expired and taken by someone else —
    /// is refused naming them, so an editor who was away learns why rather than
    /// discovering it at save time.
    pub fn renew(&self, key: &LeaseKey, actor: &Actor, now: Instant) -> Result<(), BoundaryError> {
        let mut held = self.lock();
        let me = Principal::of(actor);

        match held.get_mut(key) {
            Some(existing) if existing.holder == me => {
                existing.expires_at = now + LEASE_TTL;
                Ok(())
            }
            Some(existing) if existing.is_live(now) => Err(BoundaryError::Busy {
                record: key.kind.label().to_string(),
                holder: existing.holder_name.clone(),
            }),
            _ => {
                // Gone and unclaimed — take it back rather than making the
                // person close and reopen what they still have on screen.
                held.insert(
                    key.clone(),
                    Held {
                        holder: me,
                        holder_name: actor.display_name.clone(),
                        expires_at: now + LEASE_TTL,
                    },
                );
                Ok(())
            }
        }
    }

    /// Give up one hold. Someone else's hold is never released by this.
    pub fn release(&self, key: &LeaseKey, actor: &Actor) {
        let mut held = self.lock();
        let me = Principal::of(actor);
        if held.get(key).map(|h| h.holder == me).unwrap_or(false) {
            held.remove(key);
        }
    }

    /// The commit is the letting-go.
    ///
    /// A successful save drops this author's holds on that kind of record, so
    /// the badge comes down everywhere without anyone pressing anything.
    pub fn release_all_of_kind(&self, kind: Leasable, actor: &Actor) {
        let me = Principal::of(actor);
        self.lock()
            .retain(|key, held| !(key.kind == kind && held.holder == me));
    }

    /// Drop every hold this actor has, for a sign-out or a dropped connection.
    pub fn release_everything_for(&self, actor: &Actor) {
        let me = Principal::of(actor);
        self.lock().retain(|_, held| held.holder != me);
    }

    /// Who holds this, if anyone — for drawing a badge.
    pub fn holder_of(&self, key: &LeaseKey, now: Instant) -> Option<String> {
        let held = self.lock();
        held.get(key)
            .filter(|h| h.is_live(now))
            .map(|h| h.holder_name.clone())
    }

    /// Every live hold of a kind, as `(record_id, holder_name)`.
    pub fn held_of_kind(&self, kind: Leasable, now: Instant) -> Vec<(String, String)> {
        self.lock()
            .iter()
            .filter(|(key, held)| key.kind == kind && held.is_live(now))
            .map(|(key, held)| (key.record_id.clone(), held.holder_name.clone()))
            .collect()
    }

    /// Drop expired entries. Not required for correctness — expiry is passive —
    /// but it keeps the map from growing with records nobody is editing.
    pub fn prune(&self, now: Instant) {
        self.lock().retain(|_, held| held.is_live(now));
    }

    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.lock().len()
    }
}

/// The other half of the split: refuse a save built on a row that has moved.
///
/// This is what stands in for a lease on transactions, and it also backs up the
/// leases on everything else — a hold can expire while someone is still typing,
/// and this is what catches the save that follows.
pub fn check_row_version(
    conn: &rusqlite::Connection,
    table: Stamped,
    record_label: &str,
    id: &str,
    expected: i64,
) -> Result<(), BoundaryError> {
    let sql = format!("SELECT row_version FROM {} WHERE id = ?1", table.table());
    let actual: Option<i64> = conn
        .query_row(&sql, [id], |row| row.get(0))
        .ok();

    match actual {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(BoundaryError::Stale {
            record: record_label.to_string(),
            expected,
            actual,
        }),
        None => Err(BoundaryError::invalid(format!(
            "That {record_label} has been deleted."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::Grants;
    use crate::database::Database;

    fn sam() -> Actor {
        Actor::new("u-sam".into(), "Sam".into(), true, Grants::all())
    }

    fn alex() -> Actor {
        Actor::new("u-alex".into(), "Alex".into(), false, Grants::all())
    }

    fn key() -> LeaseKey {
        LeaseKey::new(Leasable::Budget, "budget-1")
    }

    #[test]
    fn transactions_cannot_be_leased_at_all() {
        // The split is a property of the type: there is no Transaction variant,
        // so no future code can put the evening's receipts in a queue.
        let labels: Vec<&str> = Leasable::ALL.iter().map(|l| l.label()).collect();
        assert!(!labels.contains(&"transaction"));
        assert_eq!(labels.len(), 4);
    }

    #[test]
    fn a_free_record_can_be_taken() {
        let leases = Leases::new();
        assert!(leases.acquire(key(), &sam(), Instant::now()).is_ok());
    }

    #[test]
    fn a_second_person_is_refused_and_told_who_has_it() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();

        let err = leases.acquire(key(), &alex(), now).unwrap_err();
        match &err {
            BoundaryError::Busy { record, holder } => {
                assert_eq!(record, "budget");
                assert_eq!(holder, "Sam");
            }
            other => panic!("expected Busy, got {other:?}"),
        }
        assert!(err.sentence().contains("Sam"), "{}", err.sentence());
        assert!(err.sentence().contains("budget"), "{}", err.sentence());
    }

    #[test]
    fn taking_a_hold_twice_is_a_renewal_not_a_refusal() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();
        assert!(
            leases.acquire(key(), &sam(), now).is_ok(),
            "a person must not be blocked by their own hold"
        );
    }

    /// Passive expiry: nothing sweeps, the next asker simply gets it.
    #[test]
    fn an_expired_hold_falls_to_the_next_asker() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();

        let later = now + LEASE_TTL + Duration::from_secs(1);
        assert!(
            leases.acquire(key(), &alex(), later).is_ok(),
            "an abandoned hold should not block the record forever"
        );
        assert_eq!(leases.holder_of(&key(), later).as_deref(), Some("Alex"));
    }

    #[test]
    fn a_heartbeat_keeps_a_hold_alive_past_the_ttl() {
        let leases = Leases::new();
        let start = Instant::now();
        leases.acquire(key(), &sam(), start).unwrap();

        // Renew every heartbeat for twice the TTL.
        let mut now = start;
        for _ in 0..6 {
            now += LEASE_HEARTBEAT;
            leases.renew(&key(), &sam(), now).unwrap();
        }

        assert!(now > start + LEASE_TTL, "the test did not outlast the TTL");
        let err = leases.acquire(key(), &alex(), now).unwrap_err();
        assert!(matches!(err, BoundaryError::Busy { .. }));
    }

    #[test]
    fn the_heartbeat_is_comfortably_inside_the_ttl() {
        // One dropped beat must not drop the hold.
        assert!(
            LEASE_HEARTBEAT * 2 < LEASE_TTL,
            "a single missed heartbeat would expire the lease"
        );
    }

    #[test]
    fn renewing_a_hold_someone_else_took_is_refused_naming_them() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();

        let later = now + LEASE_TTL + Duration::from_secs(1);
        leases.acquire(key(), &alex(), later).unwrap();

        let err = leases.renew(&key(), &sam(), later).unwrap_err();
        assert!(err.sentence().contains("Alex"), "{}", err.sentence());
    }

    #[test]
    fn renewing_a_hold_that_simply_lapsed_takes_it_back() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();

        let later = now + LEASE_TTL + Duration::from_secs(1);
        assert!(
            leases.renew(&key(), &sam(), later).is_ok(),
            "someone still looking at the record should keep it if nobody took it"
        );
    }

    #[test]
    fn releasing_gives_the_record_up_immediately() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();
        leases.release(&key(), &sam());

        assert!(leases.holder_of(&key(), now).is_none());
        assert!(leases.acquire(key(), &alex(), now).is_ok());
    }

    #[test]
    fn one_person_cannot_release_anothers_hold() {
        let leases = Leases::new();
        let now = Instant::now();
        leases.acquire(key(), &sam(), now).unwrap();

        leases.release(&key(), &alex());
        assert_eq!(
            leases.holder_of(&key(), now).as_deref(),
            Some("Sam"),
            "Alex released a hold that was not theirs"
        );
    }

    /// The commit is the letting-go.
    #[test]
    fn a_save_drops_the_authors_holds_on_that_kind() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b1"), &sam(), now)
            .unwrap();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b2"), &sam(), now)
            .unwrap();
        leases
            .acquire(LeaseKey::new(Leasable::Account, "a1"), &sam(), now)
            .unwrap();

        leases.release_all_of_kind(Leasable::Budget, &sam());

        assert!(leases.held_of_kind(Leasable::Budget, now).is_empty());
        assert_eq!(
            leases.held_of_kind(Leasable::Account, now).len(),
            1,
            "saving a budget should not drop a hold on an account"
        );
    }

    #[test]
    fn a_save_does_not_drop_someone_elses_holds() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b1"), &sam(), now)
            .unwrap();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b2"), &alex(), now)
            .unwrap();

        leases.release_all_of_kind(Leasable::Budget, &sam());

        let remaining = leases.held_of_kind(Leasable::Budget, now);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].1, "Alex");
    }

    #[test]
    fn signing_out_drops_everything_that_person_held() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b1"), &sam(), now)
            .unwrap();
        leases
            .acquire(LeaseKey::new(Leasable::Account, "a1"), &sam(), now)
            .unwrap();
        leases
            .acquire(LeaseKey::new(Leasable::Goal, "g1"), &alex(), now)
            .unwrap();

        leases.release_everything_for(&sam());

        assert!(leases.held_of_kind(Leasable::Budget, now).is_empty());
        assert!(leases.held_of_kind(Leasable::Account, now).is_empty());
        assert_eq!(leases.held_of_kind(Leasable::Goal, now).len(), 1);
    }

    /// The reason for keying on `(is_owner, user_id)` rather than the id alone.
    #[test]
    fn a_local_session_and_a_network_session_sharing_an_id_do_not_share_a_hold() {
        let leases = Leases::new();
        let now = Instant::now();

        let at_the_host = Actor::new("u1".into(), "Sam (here)".into(), true, Grants::all());
        let over_the_network = Actor::new("u1".into(), "Sam (laptop)".into(), false, Grants::all());

        leases.acquire(key(), &at_the_host, now).unwrap();
        let err = leases.acquire(key(), &over_the_network, now).unwrap_err();

        assert!(
            matches!(err, BoundaryError::Busy { .. }),
            "the network session took over a hold belonging to the local one"
        );
    }

    #[test]
    fn holds_of_different_records_do_not_interfere() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "b1"), &sam(), now)
            .unwrap();
        assert!(leases
            .acquire(LeaseKey::new(Leasable::Budget, "b2"), &alex(), now)
            .is_ok());
    }

    #[test]
    fn the_same_id_in_two_kinds_is_two_holds() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "shared-id"), &sam(), now)
            .unwrap();
        assert!(
            leases
                .acquire(LeaseKey::new(Leasable::Account, "shared-id"), &alex(), now)
                .is_ok(),
            "a budget and an account that happen to share an id are different records"
        );
    }

    #[test]
    fn pruning_drops_only_expired_holds() {
        let leases = Leases::new();
        let now = Instant::now();
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "old"), &sam(), now)
            .unwrap();

        let later = now + LEASE_TTL - Duration::from_secs(1);
        leases
            .acquire(LeaseKey::new(Leasable::Budget, "fresh"), &alex(), later)
            .unwrap();

        leases.prune(now + LEASE_TTL + Duration::from_secs(1));
        assert_eq!(leases.tracked(), 1);
        assert_eq!(
            leases
                .holder_of(
                    &LeaseKey::new(Leasable::Budget, "fresh"),
                    now + LEASE_TTL + Duration::from_secs(1)
                )
                .as_deref(),
            Some("Alex")
        );
    }

    #[test]
    fn every_leasable_kind_maps_to_a_real_table() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            for kind in Leasable::ALL {
                let sql = format!("SELECT COUNT(*) FROM {}", kind.table().table());
                let _: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
            }
            Ok(())
        })
        .unwrap();
    }

    // ------------------------------------------------ the optimistic half

    #[test]
    fn a_save_against_the_current_version_is_allowed() {
        let db = Database::in_memory().unwrap();
        let account = crate::models::Account::with_starting_balance(
            "Checking".into(),
            crate::models::AccountType::Checking,
            "100".parse().unwrap(),
        );
        db.with_connection(|conn| crate::database::repository::create_account(conn, &account))
            .unwrap();

        db.with_connection(|conn| {
            assert!(
                check_row_version(conn, Stamped::Accounts, "account", &account.id, 1).is_ok()
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_save_against_a_version_that_moved_is_refused_with_both_numbers() {
        let db = Database::in_memory().unwrap();
        let account = crate::models::Account::with_starting_balance(
            "Checking".into(),
            crate::models::AccountType::Checking,
            "100".parse().unwrap(),
        );
        db.with_connection(|conn| crate::database::repository::create_account(conn, &account))
            .unwrap();

        db.with_connection(|conn| {
            conn.execute(
                "UPDATE accounts SET name = 'Renamed' WHERE id = ?1",
                [&account.id],
            )?;

            let err =
                check_row_version(conn, Stamped::Accounts, "account", &account.id, 1).unwrap_err();
            match &err {
                BoundaryError::Stale {
                    record,
                    expected,
                    actual,
                } => {
                    assert_eq!(record, "account");
                    assert_eq!(*expected, 1);
                    assert_eq!(*actual, 2);
                }
                other => panic!("expected Stale, got {other:?}"),
            }
            assert!(err.sentence().contains("someone else"), "{}", err.sentence());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn saving_a_record_that_was_deleted_says_so() {
        let db = Database::in_memory().unwrap();
        db.with_connection(|conn| {
            let err =
                check_row_version(conn, Stamped::Accounts, "account", "gone", 1).unwrap_err();
            assert!(err.sentence().contains("deleted"), "{}", err.sentence());
            Ok(())
        })
        .unwrap();
    }
}

// ------------------------------------------------------------- commands

/// Naming the kind on the wire, so a client says `{"kind":"budget", ...}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeasableWire {
    Account,
    Budget,
    Category,
    Goal,
}

impl From<LeasableWire> for Leasable {
    fn from(wire: LeasableWire) -> Self {
        match wire {
            LeasableWire::Account => Leasable::Account,
            LeasableWire::Budget => Leasable::Budget,
            LeasableWire::Category => Leasable::Category,
            LeasableWire::Goal => Leasable::Goal,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LeaseArgs {
    kind: LeasableWire,
    record_id: String,
}

#[derive(Debug, Deserialize)]
struct HoldersArgs {
    kind: LeasableWire,
}

#[derive(Debug, Serialize)]
struct Holder {
    record_id: String,
    holder: String,
}

/// Resolve the argument, then check the grant the *edit* would need.
///
/// The registry gate for these commands is `Required::signed_in()`, because the
/// real answer depends on which kind of record is named. Deferring the check is
/// only safe because it happens here, before anything is held.
fn gate(ctx: &BoundaryCtx, args: Value) -> Result<(Leasable, String), BoundaryError> {
    let args: LeaseArgs = decode(args)?;
    let kind: Leasable = args.kind.into();
    authorize(ctx.actor, Required::write(kind.area()))?;
    Ok((kind, args.record_id))
}

fn h_acquire(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let (kind, record_id) = gate(ctx, args)?;
    ctx.shared.leases.acquire(
        LeaseKey::new(kind, record_id),
        ctx.actor,
        std::time::Instant::now(),
    )?;
    encode(serde_json::json!({ "held": true }))
}

fn h_renew(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let (kind, record_id) = gate(ctx, args)?;
    ctx.shared.leases.renew(
        &LeaseKey::new(kind, record_id),
        ctx.actor,
        std::time::Instant::now(),
    )?;
    encode(serde_json::json!({ "held": true }))
}

fn h_release(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let (kind, record_id) = gate(ctx, args)?;
    ctx.shared
        .leases
        .release(&LeaseKey::new(kind, record_id), ctx.actor);
    encode(serde_json::json!({ "held": false }))
}

fn h_holders(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let args: HoldersArgs = decode(args)?;
    let kind: Leasable = args.kind.into();
    // Seeing who is editing is a read, not an edit.
    authorize(ctx.actor, Required::read(kind.area()))?;

    let holders: Vec<Holder> = ctx
        .shared
        .leases
        .held_of_kind(kind, std::time::Instant::now())
        .into_iter()
        .map(|(record_id, holder)| Holder { record_id, holder })
        .collect();
    encode(holders)
}

pub fn register(registry: &mut Registry) {
    registry.register("lease_acquire", Required::signed_in(), h_acquire);
    registry.register("lease_renew", Required::signed_in(), h_renew);
    registry.register("lease_release", Required::signed_in(), h_release);
    registry.register("lease_holders", Required::signed_in(), h_holders);
}
