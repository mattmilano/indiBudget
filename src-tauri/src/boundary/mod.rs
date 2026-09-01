//! The multi-user boundary.
//!
//! Every database operation that may be reached from another machine passes
//! through here as a serialisable command with a serialisable response.
//! Nothing above this boundary ever holds a connection, a cursor, or a
//! transaction handle — which is what lets multi-user be a transport swap
//! rather than a rewrite.
//!
//! A command that is not registered **does not exist remotely**. That is the
//! safe default and it is load-bearing: file dialogs, local backups,
//! encryption, and anything else that must only ever run on the host are
//! simply never registered, and a remote caller is refused with a sentence
//! rather than silently touching the wrong machine's files.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod leases;
pub mod maintenance;
pub mod news;
pub mod registry;
pub mod users;

/// Tables the boundary stamps authorship on.
///
/// An enum rather than a `&str` because a table name cannot be a bound
/// parameter — it has to be formatted into the SQL, so the set of possible
/// values must be closed at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stamped {
    Accounts,
    Transactions,
    Categories,
    Budgets,
    SavingsGoals,
    GoalContributions,
    RecurringTransactions,
    CategoryRules,
    Users,
}

impl Stamped {
    pub const fn table(self) -> &'static str {
        match self {
            Stamped::Accounts => "accounts",
            Stamped::Transactions => "transactions",
            Stamped::Categories => "categories",
            Stamped::Budgets => "budgets",
            Stamped::SavingsGoals => "savings_goals",
            Stamped::GoalContributions => "goal_contributions",
            Stamped::RecurringTransactions => "recurring_transactions",
            Stamped::CategoryRules => "category_rules",
            Stamped::Users => "users",
        }
    }
}

/// Record who wrote a row.
///
/// Deliberately explicit rather than a trigger. A trigger would need the
/// current actor on the connection as per-session state, and there are
/// legitimate raw connections — the test harness, backup and restore, any
/// future CLI or salvage path — that carry no such state and would fail on a
/// trigger referencing it. Calling this from the boundary's write wrappers
/// costs a line per command during the sweeps and cannot break a raw
/// connection.
///
/// `is_new` distinguishes an insert (stamp both columns) from an update
/// (stamp only `updated_by`, preserving who originally created the row).
///
/// This is itself an UPDATE, so it fires the `row_version` trigger and the
/// version advances again. That is deliberate and harmless: `row_version` is
/// an opaque change-detector, not an edit counter. Optimistic concurrency only
/// ever asks "is this row still the one I read?", and a caller reads the row
/// after both the write and its stamp have landed, so it holds the settled
/// value either way.
///
/// The rule that keeps this safe is that a stamp never travels alone — it
/// always accompanies a data write. Stamping a row that was not otherwise
/// changed would invalidate a version someone is legitimately holding.
pub fn stamp_write(
    conn: &rusqlite::Connection,
    table: Stamped,
    id: &str,
    actor: &Actor,
    is_new: bool,
) -> Result<(), BoundaryError> {
    let sql = if is_new {
        format!(
            "UPDATE {} SET created_by = ?1, updated_by = ?1 WHERE id = ?2",
            table.table()
        )
    } else {
        format!(
            "UPDATE {} SET updated_by = ?1 WHERE id = ?2",
            table.table()
        )
    };

    conn.execute(&sql, rusqlite::params![&actor.user_id, id])
        .map_err(|e| BoundaryError::internal(format!("Could not record who made that change: {e}")))?;
    Ok(())
}

/// The areas indiBudget divides permission by.
///
/// These map to how a household actually divides trust rather than to the
/// module layout: someone may be allowed to log spending without being
/// allowed to reshape the budget, and a teenager may track their own goals
/// without seeing the mortgage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// Accounts, transactions, transfers — the money itself.
    Money,
    /// Budgets and savings goals — the plan for the money.
    Planning,
    /// Categories, payees, categorisation rules — the shape of the data.
    Structure,
    /// Reporting and analytics.
    Reports,
    /// Users, settings, backup and restore.
    Admin,
}

impl Area {
    pub const ALL: [Area; 5] = [
        Area::Money,
        Area::Planning,
        Area::Structure,
        Area::Reports,
        Area::Admin,
    ];

    /// Human-readable name, for refusal sentences.
    pub fn label(&self) -> &'static str {
        match self {
            Area::Money => "Money",
            Area::Planning => "Planning",
            Area::Structure => "Structure",
            Area::Reports => "Reports",
            Area::Admin => "Admin",
        }
    }
}

/// What a person may do in an area. Ordered: `None` < `Read` < `Write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    None,
    Read,
    Write,
}

impl Access {
    pub fn label(&self) -> &'static str {
        match self {
            Access::None => "no access",
            Access::Read => "read",
            Access::Write => "write",
        }
    }
}

/// A person's access, area by area. An area absent from the map is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants(BTreeMap<Area, Access>);

impl Grants {
    pub fn none() -> Self {
        Grants(BTreeMap::new())
    }

    /// Write access to every area. This is what an owner or administrator
    /// resolves to — see `Actor::new`.
    pub fn all() -> Self {
        let mut map = BTreeMap::new();
        for area in Area::ALL {
            map.insert(area, Access::Write);
        }
        Grants(map)
    }

    pub fn with(mut self, area: Area, access: Access) -> Self {
        self.0.insert(area, access);
        self
    }

    pub fn access(&self, area: Area) -> Access {
        self.0.get(&area).copied().unwrap_or(Access::None)
    }

    pub fn allows(&self, required: Required) -> bool {
        self.access(required.area) >= required.access
    }

    /// True when this actor may reach nothing at all.
    pub fn is_empty(&self) -> bool {
        Area::ALL.iter().all(|a| self.access(*a) == Access::None)
    }
}

/// Who is making a request.
///
/// Identity is attached by whoever authenticated the connection and is never
/// read from the message body, so no client can name itself administrator by
/// editing a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub user_id: String,
    pub display_name: String,
    pub is_owner: bool,
    grants: Grants,
}

impl Actor {
    /// An owner's grants are *always* the full set, regardless of what grant
    /// rows exist for them.
    ///
    /// This is deliberate and load-bearing. An administrator created through
    /// the hosting screen has no per-area grant rows at all; resolving their
    /// access from those rows would leave them able to manage users but unable
    /// to open an account or run a report. Compiling the owner flag to the full
    /// grant set here means that cannot happen, wherever the actor is built.
    pub fn new(user_id: String, display_name: String, is_owner: bool, grants: Grants) -> Self {
        let grants = if is_owner { Grants::all() } else { grants };
        Self {
            user_id,
            display_name,
            is_owner,
            grants,
        }
    }

    /// The person sitting at the hosting machine in single-user use.
    pub fn local_owner() -> Self {
        Actor::new(
            "local".to_string(),
            "This computer".to_string(),
            true,
            Grants::all(),
        )
    }

    pub fn grants(&self) -> &Grants {
        &self.grants
    }
}

/// State the hosting process keeps in memory, shared by every session.
///
/// Held here rather than in the transport so that the local session and a
/// network session reach exactly the same holds — a lease taken at the hosting
/// machine must block a laptop, and vice versa.
#[derive(Debug, Default)]
pub struct SharedState {
    pub leases: leases::Leases,
    pub news: news::News,
    pub maintenance: maintenance::Maintenance,
}

impl SharedState {
    pub fn new() -> Self {
        SharedState {
            leases: leases::Leases::new(),
            news: news::News::new(),
            maintenance: maintenance::Maintenance::new(),
        }
    }
}

/// What a command touches. Stated at registration — a command cannot enter the
/// registry without declaring this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Required {
    pub area: Area,
    pub access: Access,
}

impl Required {
    pub const fn read(area: Area) -> Self {
        Required {
            area,
            access: Access::Read,
        }
    }

    pub const fn write(area: Area) -> Self {
        Required {
            area,
            access: Access::Write,
        }
    }

    /// Passable by any signed-in actor.
    ///
    /// Only for commands whose real authorization depends on their arguments
    /// and therefore cannot be stated once at registration — the edit-hold
    /// commands, where the area follows from which kind of record is being
    /// held. Such a handler MUST call `authorize` itself; this constructor is
    /// not a way to skip the check, only to defer it to where the answer is
    /// knowable.
    pub const fn signed_in() -> Self {
        Required {
            area: Area::Money,
            access: Access::None,
        }
    }
}

/// A refusal, with a sentence a person can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoundaryError {
    /// Not in the registry — therefore not reachable from another machine.
    UnknownCommand { command: String },
    /// The actor lacks the grant this command requires.
    Denied {
        actor: String,
        area: Area,
        needed: Access,
        held: Access,
    },
    /// Optimistic-concurrency conflict: the row moved under the caller.
    Stale {
        record: String,
        expected: i64,
        actual: i64,
    },
    /// Someone else holds the edit lease on this record.
    Busy { record: String, holder: String },
    /// The host has closed the file for maintenance.
    Maintenance { closed_by: String },
    /// Malformed arguments.
    Invalid { message: String },
    /// Anything else, already rendered into a sentence.
    Internal { message: String },
}

impl BoundaryError {
    /// The sentence shown to a person. Refusals name who and what, never just
    /// "denied" — a refusal that does not say who is holding something up
    /// generates a support conversation instead of resolving one.
    pub fn sentence(&self) -> String {
        match self {
            BoundaryError::UnknownCommand { command } => format!(
                "\"{command}\" can only be run on the computer hosting this budget."
            ),
            BoundaryError::Denied {
                actor,
                area,
                needed,
                held,
            } => format!(
                "{actor} needs {} access to {} to do that, but currently has {}.",
                needed.label(),
                area.label(),
                held.label()
            ),
            BoundaryError::Stale {
                record,
                expected,
                actual,
            } => format!(
                "This {record} was changed by someone else while you were editing it \
                 (you started from version {expected}, it is now version {actual}). \
                 Reopen it to see their changes."
            ),
            BoundaryError::Busy { record, holder } => {
                format!("{holder} is editing this {record} right now.")
            }
            BoundaryError::Maintenance { closed_by } => format!(
                "{closed_by} has closed this budget for maintenance. \
                 You can still look at it, but changes are paused."
            ),
            BoundaryError::Invalid { message } => message.clone(),
            BoundaryError::Internal { message } => message.clone(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        BoundaryError::Invalid {
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        BoundaryError::Internal {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sentence())
    }
}

impl std::error::Error for BoundaryError {}

/// A request crossing the boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

impl Request {
    pub fn new(command: impl Into<String>, args: Value) -> Self {
        Request {
            command: command.into(),
            args,
        }
    }
}

/// A response crossing the boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { value: Value },
    Err { error: BoundaryError, sentence: String },
}

impl Response {
    pub fn ok(value: Value) -> Self {
        Response::Ok { value }
    }

    pub fn err(error: BoundaryError) -> Self {
        let sentence = error.sentence();
        Response::Err { error, sentence }
    }

    pub fn from_result(result: Result<Value, BoundaryError>) -> Self {
        match result {
            Ok(value) => Response::ok(value),
            Err(error) => Response::err(error),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Response::Ok { .. })
    }
}

/// Check an actor against what a command requires.
pub fn authorize(actor: &Actor, required: Required) -> Result<(), BoundaryError> {
    if actor.grants().allows(required) {
        Ok(())
    } else {
        Err(BoundaryError::Denied {
            actor: actor.display_name.clone(),
            area: required.area,
            needed: required.access,
            held: actor.grants().access(required.area),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_is_ordered() {
        assert!(Access::Write > Access::Read);
        assert!(Access::Read > Access::None);
    }

    #[test]
    fn absent_area_is_no_access() {
        let grants = Grants::none().with(Area::Money, Access::Write);
        assert_eq!(grants.access(Area::Money), Access::Write);
        assert_eq!(grants.access(Area::Admin), Access::None);
    }

    #[test]
    fn read_grant_does_not_allow_write() {
        let grants = Grants::none().with(Area::Money, Access::Read);
        assert!(grants.allows(Required::read(Area::Money)));
        assert!(!grants.allows(Required::write(Area::Money)));
    }

    /// Trap #1 from the indiAccounting handoff: an administrator created
    /// through the hosting screen has no grant rows. If the owner flag did not
    /// compile to the full grant set, they could manage users but not open an
    /// account.
    #[test]
    fn owner_with_no_grant_rows_still_reaches_everything() {
        let owner = Actor::new("u1".into(), "Sam".into(), true, Grants::none());
        for area in Area::ALL {
            assert!(
                owner.grants().allows(Required::write(area)),
                "owner was refused write on {}",
                area.label()
            );
        }
    }

    #[test]
    fn non_owner_keeps_exactly_the_grants_given() {
        let member = Actor::new(
            "u2".into(),
            "Alex".into(),
            false,
            Grants::none().with(Area::Money, Access::Write),
        );
        assert!(member.grants().allows(Required::write(Area::Money)));
        assert!(!member.grants().allows(Required::read(Area::Admin)));
    }

    #[test]
    fn denial_names_the_person_the_area_and_both_levels() {
        let member = Actor::new(
            "u2".into(),
            "Alex".into(),
            false,
            Grants::none().with(Area::Money, Access::Read),
        );
        let err = authorize(&member, Required::write(Area::Money)).unwrap_err();
        let sentence = err.sentence();
        assert!(sentence.contains("Alex"), "sentence: {sentence}");
        assert!(sentence.contains("Money"), "sentence: {sentence}");
        assert!(sentence.contains("write"), "sentence: {sentence}");
        assert!(sentence.contains("read"), "sentence: {sentence}");
    }

    #[test]
    fn every_refusal_has_a_non_empty_sentence() {
        let errors = vec![
            BoundaryError::UnknownCommand {
                command: "export_backup".into(),
            },
            BoundaryError::Denied {
                actor: "Alex".into(),
                area: Area::Admin,
                needed: Access::Write,
                held: Access::None,
            },
            BoundaryError::Stale {
                record: "budget".into(),
                expected: 3,
                actual: 5,
            },
            BoundaryError::Busy {
                record: "budget".into(),
                holder: "Sam".into(),
            },
            BoundaryError::Maintenance {
                closed_by: "Sam".into(),
            },
            BoundaryError::invalid("Amount must be a number."),
            BoundaryError::internal("Database is not open."),
        ];
        for error in errors {
            assert!(!error.sentence().trim().is_empty(), "empty: {error:?}");
        }
    }

    #[test]
    fn busy_and_stale_sentences_name_the_other_party_or_the_versions() {
        let busy = BoundaryError::Busy {
            record: "budget".into(),
            holder: "Sam".into(),
        };
        assert!(busy.sentence().contains("Sam"));

        let stale = BoundaryError::Stale {
            record: "budget".into(),
            expected: 3,
            actual: 5,
        };
        let sentence = stale.sentence();
        assert!(sentence.contains('3') && sentence.contains('5'), "{sentence}");
    }

    #[test]
    fn envelopes_round_trip_through_json() {
        let request = Request::new("get_accounts", serde_json::json!({ "active_only": true }));
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: Request = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.command, "get_accounts");
        assert_eq!(decoded.args["active_only"], serde_json::json!(true));

        let response = Response::ok(serde_json::json!([{ "id": "a1" }]));
        let encoded = serde_json::to_string(&response).unwrap();
        let decoded: Response = serde_json::from_str(&encoded).unwrap();
        assert!(decoded.is_ok());
    }

    #[test]
    fn error_responses_carry_the_sentence_over_the_wire() {
        let response = Response::err(BoundaryError::Busy {
            record: "budget".into(),
            holder: "Sam".into(),
        });
        let encoded = serde_json::to_string(&response).unwrap();
        let decoded: Response = serde_json::from_str(&encoded).unwrap();
        match decoded {
            Response::Err { sentence, .. } => assert!(sentence.contains("Sam")),
            Response::Ok { .. } => panic!("expected an error response"),
        }
    }

    #[test]
    fn actor_identity_is_not_readable_from_request_args() {
        // Identity travels with the connection, never the message. A request
        // is only ever a command name and arguments.
        let request = Request::new("delete_account", serde_json::json!({ "is_owner": true }));
        let encoded = serde_json::to_value(&request).unwrap();
        let fields: Vec<String> = encoded.as_object().unwrap().keys().cloned().collect();
        assert_eq!(fields, vec!["args".to_string(), "command".to_string()]);

        // An `is_owner` sitting in the args is just data — it is not consulted
        // when building the Actor, which comes from the authenticated
        // connection.
        let actor = Actor::new("u".into(), "Alex".into(), false, Grants::none());
        assert!(!actor.is_owner);
    }
}
