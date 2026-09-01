//! The command registry.
//!
//! A name-keyed table rather than an enum: `Command::GetAccounts` style
//! exhaustive matching works at forty operations and stops working well before
//! two hundred. Registration is the declaration point — a command cannot enter
//! this table without stating the area and access level it needs, which buys
//! back the compile-time property an exhaustive match would have given.
//!
//! **A command absent from this registry does not exist remotely.** File
//! dialogs, local-path backups, encryption unlock, and the database path are
//! deliberately never registered; a remote caller receives a sentence telling
//! them to do it at the host.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

use super::{authorize, Actor, BoundaryError, Request, Required, Response, SharedState};
use crate::database::Database;

/// What a handler is given: the open database and who is asking.
///
/// The handler borrows the database for the duration of the call and returns a
/// serialisable value. It never hands a connection back out.
pub struct BoundaryCtx<'a> {
    pub db: &'a Database,
    pub actor: &'a Actor,
    pub shared: &'a SharedState,
}

impl<'a> BoundaryCtx<'a> {
    pub fn new(db: &'a Database, actor: &'a Actor, shared: &'a SharedState) -> Self {
        BoundaryCtx { db, actor, shared }
    }
}

pub type Handler = fn(&BoundaryCtx, Value) -> Result<Value, BoundaryError>;

/// One registered command.
pub struct Registration {
    pub name: &'static str,
    pub required: Required,
    pub handler: Handler,
    /// Whether this command still runs while the budget is closed for
    /// maintenance. True only for the handful that operate the closed sign
    /// itself — see `register_during_maintenance`.
    pub allowed_during_maintenance: bool,
}

#[derive(Default)]
pub struct Registry {
    commands: BTreeMap<&'static str, Registration>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            commands: BTreeMap::new(),
        }
    }

    /// Register a command. Panics on a duplicate name — that is a programming
    /// error that should fail at startup, not silently shadow a handler.
    pub fn register(&mut self, name: &'static str, required: Required, handler: Handler) {
        self.insert(Registration {
            name,
            required,
            handler,
            allowed_during_maintenance: false,
        });
    }

    /// Register a command that keeps working while the budget is closed.
    ///
    /// A separate method rather than a flag on `register`, so that the
    /// exemption is visible at the declaration point and cannot be granted by
    /// absent-mindedly passing `true`.
    ///
    /// This exists because reopening is itself a write on Admin: gated like any
    /// other write, the one command that removes the closed sign would be
    /// blocked by it, and a closed budget could only be reopened by restarting
    /// the host.
    pub fn register_during_maintenance(
        &mut self,
        name: &'static str,
        required: Required,
        handler: Handler,
    ) {
        self.insert(Registration {
            name,
            required,
            handler,
            allowed_during_maintenance: true,
        });
    }

    fn insert(&mut self, registration: Registration) {
        let name = registration.name;
        if self.commands.insert(name, registration).is_some() {
            panic!("command \"{name}\" registered twice");
        }
    }

    pub fn get(&self, name: &str) -> Option<&Registration> {
        self.commands.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &&'static str> {
        self.commands.keys()
    }

    pub fn all(&self) -> impl Iterator<Item = &Registration> {
        self.commands.values()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Decode a handler's arguments, turning a serde failure into a sentence.
pub fn decode<A: DeserializeOwned>(args: Value) -> Result<A, BoundaryError> {
    serde_json::from_value(args)
        .map_err(|e| BoundaryError::invalid(format!("Those details were not in the expected form: {e}")))
}

/// Encode a handler's result.
pub fn encode<R: Serialize>(value: R) -> Result<Value, BoundaryError> {
    serde_json::to_value(value)
        .map_err(|e| BoundaryError::internal(format!("Could not prepare the response: {e}")))
}

/// Run a request against the registry.
///
/// Order matters: an unregistered command is refused before authorization, so
/// a remote caller learns "that runs at the host" rather than "you lack a
/// grant" for something that was never reachable in the first place.
pub fn dispatch(registry: &Registry, ctx: &BoundaryCtx, request: Request) -> Response {
    let Some(registration) = registry.get(&request.command) else {
        return Response::err(BoundaryError::UnknownCommand {
            command: request.command,
        });
    };

    if let Err(error) = authorize(ctx.actor, registration.required) {
        return Response::err(error);
    }

    // The closed sign, applied after authorization and before the handler.
    // Scoped to writes, and skipped for the commands that operate the sign
    // itself.
    if !registration.allowed_during_maintenance {
        if let Err(error) = ctx.shared.maintenance.gate(registration.required) {
            return Response::err(error);
        }
    }

    Response::from_result((registration.handler)(ctx, request.args))
}

/// Dispatch from a JSON string, for the transport to call in phase 3.
pub fn dispatch_json(registry: &Registry, ctx: &BoundaryCtx, raw: &str) -> String {
    let response = match serde_json::from_str::<Request>(raw) {
        Ok(request) => dispatch(registry, ctx, request),
        Err(e) => Response::err(BoundaryError::invalid(format!(
            "That request could not be read: {e}"
        ))),
    };
    serde_json::to_string(&response).unwrap_or_else(|e| {
        format!(
            r#"{{"status":"err","error":{{"kind":"internal","message":"encode failed"}},"sentence":"Could not prepare the response: {e}"}}"#
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::{Access, Area, Grants};
    use crate::database::Database;

    fn probe(_ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
        Ok(serde_json::json!({ "echoed": args }))
    }

    fn boom(_ctx: &BoundaryCtx, _args: Value) -> Result<Value, BoundaryError> {
        Err(BoundaryError::invalid("Nope."))
    }

    fn test_registry() -> Registry {
        let mut registry = Registry::new();
        registry.register("read_money", Required::read(Area::Money), probe);
        registry.register("write_money", Required::write(Area::Money), probe);
        registry.register("admin_thing", Required::write(Area::Admin), probe);
        registry.register("always_fails", Required::read(Area::Reports), boom);
        registry
    }

    #[test]
    fn unregistered_commands_do_not_exist() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let response = dispatch(&registry, &ctx, Request::new("export_backup", Value::Null));
        match response {
            Response::Err { error, sentence } => {
                assert!(matches!(error, BoundaryError::UnknownCommand { .. }));
                assert!(sentence.contains("hosting"), "sentence: {sentence}");
            }
            Response::Ok { .. } => panic!("an unregistered command was dispatched"),
        }
    }

    #[test]
    fn unknown_command_is_refused_before_authorization() {
        // A no-grants actor asking for something unregistered should be told
        // it runs at the host, not that they lack a grant for it.
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::new("u".into(), "Alex".into(), false, Grants::none());
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let response = dispatch(&registry, &ctx, Request::new("no_such_command", Value::Null));
        match response {
            Response::Err { error, .. } => {
                assert!(matches!(error, BoundaryError::UnknownCommand { .. }));
            }
            Response::Ok { .. } => panic!("expected refusal"),
        }
    }

    #[test]
    fn a_reader_cannot_reach_a_write() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::new(
            "u".into(),
            "Alex".into(),
            false,
            Grants::none().with(Area::Money, Access::Read),
        );
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        assert!(dispatch(&registry, &ctx, Request::new("read_money", Value::Null)).is_ok());

        let response = dispatch(&registry, &ctx, Request::new("write_money", Value::Null));
        match response {
            Response::Err { error, .. } => assert!(matches!(error, BoundaryError::Denied { .. })),
            Response::Ok { .. } => panic!("a read grant reached a write command"),
        }
    }

    /// The registry-invariants test, written before the sweeps so that every
    /// command added later is covered by it for free.
    #[test]
    fn registry_invariants_hold_for_every_registered_command() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();

        // Names are unique — guaranteed by `register` panicking, asserted here
        // so the property is stated where it is relied upon.
        let mut seen = std::collections::BTreeSet::new();
        for name in registry.names() {
            assert!(seen.insert(*name), "duplicate command name: {name}");
        }

        // A no-grants actor is refused at every door, with the grant sentence.
        let nobody = Actor::new("u0".into(), "Nobody".into(), false, Grants::none());
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &nobody, &shared);
        for name in registry.names() {
            let response = dispatch(&registry, &ctx, Request::new(*name, Value::Null));
            match response {
                Response::Err { error, sentence } => {
                    assert!(
                        matches!(error, BoundaryError::Denied { .. }),
                        "{name} refused a no-grants actor with {error:?}, expected Denied"
                    );
                    assert!(
                        sentence.contains("Nobody"),
                        "{name} refusal did not name the person: {sentence}"
                    );
                }
                Response::Ok { .. } => panic!("{name} admitted an actor with no grants"),
            }
        }

        // Null arguments never panic. An owner reaches every handler, so this
        // exercises the handler bodies rather than stopping at authorization.
        let owner = Actor::local_owner();
        let ctx = BoundaryCtx::new(&db, &owner, &shared);
        for name in registry.names() {
            let _ = dispatch(&registry, &ctx, Request::new(*name, Value::Null));
        }
    }

    #[test]
    fn handler_errors_become_error_responses_with_sentences() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let response = dispatch(&registry, &ctx, Request::new("always_fails", Value::Null));
        match response {
            Response::Err { sentence, .. } => assert_eq!(sentence, "Nope."),
            Response::Ok { .. } => panic!("expected the handler's error"),
        }
    }

    #[test]
    fn dispatch_json_round_trips_a_whole_call() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let raw = r#"{"command":"read_money","args":{"hello":"world"}}"#;
        let out = dispatch_json(&registry, &ctx, raw);
        let response: Response = serde_json::from_str(&out).unwrap();
        match response {
            Response::Ok { value } => assert_eq!(value["echoed"]["hello"], "world"),
            Response::Err { sentence, .. } => panic!("unexpected refusal: {sentence}"),
        }
    }

    #[test]
    fn malformed_json_is_refused_with_a_sentence_not_a_panic() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let out = dispatch_json(&registry, &ctx, "{not json at all");
        let response: Response = serde_json::from_str(&out).unwrap();
        assert!(!response.is_ok());
    }

    #[test]
    fn a_closed_budget_gates_writes_but_not_reads_at_dispatch() {
        let registry = test_registry();
        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        shared.maintenance.close(&actor);
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        let read = dispatch(&registry, &ctx, Request::new("read_money", Value::Null));
        assert!(read.is_ok(), "a read was gated by the closed sign");

        let write = dispatch(&registry, &ctx, Request::new("write_money", Value::Null));
        match write {
            Response::Err { error, .. } => {
                assert!(matches!(error, BoundaryError::Maintenance { .. }))
            }
            Response::Ok { .. } => panic!("a write landed while closed"),
        }
    }

    /// Without the exemption, the gate would block the very command that
    /// removes it, and a closed budget could only be reopened by restarting.
    #[test]
    fn an_exempt_command_still_runs_while_closed() {
        let mut registry = Registry::new();
        registry.register("ordinary_write", Required::write(Area::Admin), probe);
        registry.register_during_maintenance("undo_the_sign", Required::write(Area::Admin), probe);

        let db = Database::in_memory().unwrap();
        let actor = Actor::local_owner();
        let shared = SharedState::new();
        shared.maintenance.close(&actor);
        let ctx = BoundaryCtx::new(&db, &actor, &shared);

        assert!(
            !dispatch(&registry, &ctx, Request::new("ordinary_write", Value::Null)).is_ok(),
            "an ordinary admin write should be gated"
        );
        assert!(
            dispatch(&registry, &ctx, Request::new("undo_the_sign", Value::Null)).is_ok(),
            "the exempt command was gated, which would lock the budget closed"
        );
    }

    #[test]
    fn the_exemption_is_off_unless_asked_for() {
        let mut registry = Registry::new();
        registry.register("plain", Required::write(Area::Money), probe);
        assert!(!registry.get("plain").unwrap().allowed_during_maintenance);

        registry.register_during_maintenance("special", Required::write(Area::Money), probe);
        assert!(registry.get("special").unwrap().allowed_during_maintenance);
    }

    #[test]
    #[should_panic(expected = "registered twice")]
    fn duplicate_registration_panics_at_startup() {
        let mut registry = Registry::new();
        registry.register("dup", Required::read(Area::Money), probe);
        registry.register("dup", Required::write(Area::Money), probe);
    }
}
