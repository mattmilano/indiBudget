//! The closed sign.
//!
//! An administrator closes the budget for a few minutes to take a backup or
//! restore one, and everyone else stops being able to change things until it
//! reopens.
//!
//! Three decisions worth stating:
//!
//! - **It lives in memory.** A crash must not leave the doors closed with no
//!   way in but editing the database by hand. Restarting the host clears it.
//! - **Any administrator may reopen it, not only whoever closed it.** A sign
//!   left up by someone who has gone out for the afternoon is a social problem,
//!   and the fix must not require finding them.
//! - **Reads keep working.** A closed sign is not a blackout. Someone can still
//!   look at the budget while it is closed; they just cannot change it. A
//!   refusal wider than the reason for it reads as a bug.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;

use super::news::Notice;
use super::registry::{encode, BoundaryCtx, Registry};
use super::{Access, Actor, Area, BoundaryError, Required};

/// Who closed the budget, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedSign {
    pub closed_by: String,
    pub closed_by_id: String,
    pub closed_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct Maintenance {
    sign: Mutex<Option<ClosedSign>>,
}

impl Maintenance {
    pub fn new() -> Self {
        Maintenance {
            sign: Mutex::new(None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<ClosedSign>> {
        self.sign
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn status(&self) -> Option<ClosedSign> {
        self.lock().clone()
    }

    pub fn is_closed(&self) -> bool {
        self.lock().is_some()
    }

    /// Put the sign up. Returns whether this actually changed anything, so the
    /// caller knows whether there is news to announce — closing an already
    /// closed budget is not an event, and the original closer stays named.
    pub fn close(&self, actor: &Actor) -> bool {
        let mut sign = self.lock();
        if sign.is_some() {
            return false;
        }
        *sign = Some(ClosedSign {
            closed_by: actor.display_name.clone(),
            closed_by_id: actor.user_id.clone(),
            closed_at: Utc::now(),
        });
        true
    }

    /// Take the sign down. Any administrator may do this; see the module
    /// comment for why it is deliberately not restricted to the closer.
    pub fn reopen(&self) -> bool {
        self.lock().take().is_some()
    }

    /// The gate, applied in `dispatch` before a handler runs.
    ///
    /// Scoped to writes. A command that only reads is never held up by a closed
    /// sign, because stopping people looking was never the point.
    pub fn gate(&self, required: Required) -> Result<(), BoundaryError> {
        if required.access < Access::Write {
            return Ok(());
        }
        match self.status() {
            Some(sign) => Err(BoundaryError::Maintenance {
                closed_by: sign.closed_by,
            }),
            None => Ok(()),
        }
    }
}

// -------------------------------------------------------------- commands

fn h_close(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    if ctx.shared.maintenance.close(ctx.actor) {
        ctx.shared.news.publish(Notice::MaintenanceOn {
            closed_by: ctx.actor.display_name.clone(),
        });
    }
    encode(ctx.shared.maintenance.status())
}

fn h_reopen(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    if ctx.shared.maintenance.reopen() {
        ctx.shared.news.publish(Notice::MaintenanceOff);
    }
    encode(serde_json::json!({ "closed": false }))
}

fn h_status(ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
    encode(ctx.shared.maintenance.status())
}

pub fn register(registry: &mut Registry) {
    // Both transitions are exempt from the gate they operate.
    //
    // Reopening is a write on Admin, so without the exemption a closed budget
    // could only be reopened by restarting the host — the gate would be
    // blocking the one command that removes it. Closing is exempt for the same
    // shape of reason, so that a second administrator pressing the button on an
    // already-closed budget gets the harmless no-op rather than a confusing
    // refusal citing the first one.
    registry.register_during_maintenance(
        "maintenance_close",
        Required::write(Area::Admin),
        h_close,
    );
    registry.register_during_maintenance(
        "maintenance_reopen",
        Required::write(Area::Admin),
        h_reopen,
    );

    // Anyone signed in may ask why their saves are being refused.
    registry.register("maintenance_status", Required::signed_in(), h_status);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::Grants;

    fn admin(name: &str) -> Actor {
        Actor::new(format!("id-{name}"), name.into(), true, Grants::all())
    }

    #[test]
    fn a_new_budget_is_open() {
        let maintenance = Maintenance::new();
        assert!(!maintenance.is_closed());
        assert!(maintenance.status().is_none());
    }

    #[test]
    fn closing_records_who_did_it() {
        let maintenance = Maintenance::new();
        assert!(maintenance.close(&admin("Sam")));

        let sign = maintenance.status().expect("a sign should be up");
        assert_eq!(sign.closed_by, "Sam");
        assert_eq!(sign.closed_by_id, "id-Sam");
    }

    #[test]
    fn closing_an_already_closed_budget_changes_nothing() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        assert!(
            !maintenance.close(&admin("Alex")),
            "a second close should report that nothing changed"
        );
        assert_eq!(
            maintenance.status().unwrap().closed_by,
            "Sam",
            "the original closer should stay named"
        );
    }

    /// A sign left up by someone who went out must not need them to come back.
    #[test]
    fn any_administrator_may_reopen_not_only_the_closer() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        // `reopen` takes no actor at all — there is deliberately nothing here
        // that could compare it against the closer.
        assert!(maintenance.reopen());
        assert!(!maintenance.is_closed());
    }

    #[test]
    fn reopening_an_open_budget_changes_nothing() {
        let maintenance = Maintenance::new();
        assert!(!maintenance.reopen());
    }

    // ----------------------------------------------------------- the gate

    #[test]
    fn a_closed_budget_refuses_writes_naming_who_closed_it() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        let err = maintenance.gate(Required::write(Area::Money)).unwrap_err();
        match &err {
            BoundaryError::Maintenance { closed_by } => assert_eq!(closed_by, "Sam"),
            other => panic!("expected Maintenance, got {other:?}"),
        }
        assert!(err.sentence().contains("Sam"), "{}", err.sentence());
    }

    /// Trap #6: scope every block to the narrowest act. A closed sign is not a
    /// blackout.
    #[test]
    fn reads_keep_working_while_closed() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        for area in Area::ALL {
            assert!(
                maintenance.gate(Required::read(area)).is_ok(),
                "reading {} was blocked, which is wider than the reason for closing",
                area.label()
            );
        }
    }

    #[test]
    fn signed_in_commands_are_not_held_up_either() {
        // Edit holds and the news beat sit below Write, so a closed budget does
        // not stop someone letting go of a record or hearing that it reopened.
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));
        assert!(maintenance.gate(Required::signed_in()).is_ok());
    }

    #[test]
    fn every_area_is_blocked_for_writing_while_closed() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        for area in Area::ALL {
            assert!(
                maintenance.gate(Required::write(area)).is_err(),
                "writing {} was allowed while closed",
                area.label()
            );
        }
    }

    #[test]
    fn reopening_lets_writes_through_again() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));
        assert!(maintenance.gate(Required::write(Area::Money)).is_err());

        maintenance.reopen();
        assert!(maintenance.gate(Required::write(Area::Money)).is_ok());
    }

    #[test]
    fn the_sign_round_trips_through_json() {
        let maintenance = Maintenance::new();
        maintenance.close(&admin("Sam"));

        let sign = maintenance.status().unwrap();
        let encoded = serde_json::to_string(&sign).unwrap();
        let decoded: ClosedSign = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, sign);
    }
}
