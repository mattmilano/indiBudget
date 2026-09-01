//! Admin: the people who may reach this budget, and the machines they use.
//!
//! Two credentials answer two questions, and the levers here are deliberately
//! separate for that reason: deactivating a person does not un-pair their
//! machine, and revoking a machine does not change anyone's password.

use serde::Deserialize;
use serde_json::Value;

use super::{db_err, ok};
use crate::boundary::news::Notice;
use crate::boundary::registry::{decode, BoundaryCtx, Registry};
use crate::boundary::users;
use crate::boundary::{Area, BoundaryError, Grants, Required};
use crate::net::pairing;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewUser {
    login: String,
    display_name: String,
    password: String,
    #[serde(default)]
    is_owner: bool,
    #[serde(default)]
    grants: Grants,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserGrants {
    user_id: String,
    grants: Grants,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserActive {
    user_id: String,
    is_active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserPassword {
    user_id: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceId {
    device_id: String,
}

fn announce(ctx: &BoundaryCtx, id: &str) {
    ctx.shared.news.publish(Notice::RecordChanged {
        area: Area::Admin,
        record_kind: "person".into(),
        record_id: id.to_string(),
    });
}

fn h_list_users(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| Ok(users::list_users(c))).map_err(db_err)??)
}

fn h_create_user(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: NewUser = decode(args)?;
    let created_by = ctx.actor.user_id.clone();
    let user = ctx
        .db
        .with_connection(|c| {
            Ok(users::create_user(
                c,
                &a.login,
                &a.display_name,
                &a.password,
                a.is_owner,
                &a.grants,
                Some(&created_by),
            ))
        })
        .map_err(db_err)??;
    announce(ctx, &user.id);
    ok(user)
}

fn h_set_grants(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserGrants = decode(args)?;

    // Removing your own last way back in is the kind of mistake that needs a
    // second person to undo, so it is refused rather than merely warned about.
    if a.user_id == ctx.actor.user_id && !ctx.actor.is_owner {
        return Err(BoundaryError::invalid(
            "You cannot change your own access. Ask another administrator to do it.",
        ));
    }

    ctx.db
        .with_connection(|c| Ok(users::set_grants(c, &a.user_id, &a.grants)))
        .map_err(db_err)??;
    announce(ctx, &a.user_id);
    ok(serde_json::json!({ "updated": true }))
}

fn h_set_active(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserActive = decode(args)?;

    // Deactivating yourself locks you out of the budget you are administering,
    // and if you were the only administrator it locks everyone out.
    if a.user_id == ctx.actor.user_id && !a.is_active {
        return Err(BoundaryError::invalid(
            "You cannot deactivate your own account.",
        ));
    }

    ctx.db
        .with_connection(|c| Ok(users::set_active(c, &a.user_id, a.is_active)))
        .map_err(db_err)??;
    announce(ctx, &a.user_id);
    ok(serde_json::json!({ "updated": true }))
}

fn h_change_password(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserPassword = decode(args)?;
    ctx.db
        .with_connection(|c| Ok(users::change_password(c, &a.user_id, &a.new_password)))
        .map_err(db_err)??;
    // Deliberately no news: nothing about a password belongs in a log every
    // signed-in person can read.
    ok(serde_json::json!({ "updated": true }))
}

fn h_list_devices(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    ok(ctx.db.with_connection(|c| Ok(pairing::list_devices(c))).map_err(db_err)??)
}

fn h_revoke_device(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: DeviceId = decode(args)?;
    ctx.db
        .with_connection(|c| Ok(pairing::revoke_device(c, &a.device_id)))
        .map_err(db_err)??;
    ok(serde_json::json!({ "revoked": true }))
}

pub fn register(r: &mut Registry) {
    let w = Required::write(Area::Admin);
    let rd = Required::read(Area::Admin);

    r.register("list_users", rd, h_list_users);
    r.register("create_user", w, h_create_user);
    r.register("set_user_grants", w, h_set_grants);
    r.register("set_user_active", w, h_set_active);
    r.register("change_user_password", w, h_change_password);

    r.register("list_devices", rd, h_list_devices);
    r.register("revoke_device", w, h_revoke_device);
}
