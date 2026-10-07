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
    /// Make them an administrator, or stop them being one.
    #[serde(default)]
    is_owner: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserId {
    user_id: String,
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

/// The account this request is about is the caller's own.
///
/// Nobody changes their own role, access or active state, or deletes
/// themselves: a slip there can lock someone out of the budget they are
/// managing, and it needs a second person to undo. Another administrator can.
/// The person at the hosting computer acts as `local`, which no account has, so
/// this never stands in their way.
fn refuse_own_account(ctx: &BoundaryCtx, user_id: &str, what: &str) -> Result<(), BoundaryError> {
    if user_id == ctx.actor.user_id {
        return Err(BoundaryError::invalid(format!(
            "You can't {what} your own account. Another administrator can."
        )));
    }
    Ok(())
}

/// After someone's access changed: let go of anything they were holding, and
/// tell every window to re-read, because what each person may see may have
/// shifted. The notice names nobody.
fn people_changed(ctx: &BoundaryCtx, user_id: &str) {
    for key in ctx.shared.leases.release_user(user_id) {
        ctx.shared.news.publish(Notice::RecordFreed {
            area: key.kind.area(),
            record_kind: key.kind.label().to_string(),
            record_id: key.record_id,
        });
    }
    ctx.shared.news.publish(Notice::PeopleChanged);
}

/// A person as the People screen shows them, with their access.
#[derive(Debug, serde::Serialize)]
struct Person {
    #[serde(flatten)]
    user: users::User,
    grants: Grants,
}

fn h_list_users(ctx: &BoundaryCtx, _a: Value) -> Result<Value, BoundaryError> {
    let people = ctx
        .db
        .with_connection(|c| {
            Ok((|| {
                let mut out = Vec::new();
                for user in users::list_users(c)? {
                    let grants = users::grants_for(c, &user.id)?;
                    out.push(Person { user, grants });
                }
                Ok::<_, BoundaryError>(out)
            })())
        })
        .map_err(db_err)??;
    ok(people)
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
    ctx.shared.news.publish(Notice::PeopleChanged);
    ok(user)
}

fn h_set_grants(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserGrants = decode(args)?;
    refuse_own_account(ctx, &a.user_id, "change the access of")?;

    ctx.db
        .with_connection(|c| {
            Ok((|| {
                if let Some(is_owner) = a.is_owner {
                    users::set_owner(c, &a.user_id, is_owner)?;
                }
                users::set_grants(c, &a.user_id, &a.grants)
            })())
        })
        .map_err(db_err)??;
    people_changed(ctx, &a.user_id);
    ok(serde_json::json!({ "updated": true }))
}

fn h_set_active(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserActive = decode(args)?;
    refuse_own_account(ctx, &a.user_id, if a.is_active { "reactivate" } else { "deactivate" })?;

    ctx.db
        .with_connection(|c| Ok(users::set_active(c, &a.user_id, a.is_active)))
        .map_err(db_err)??;
    people_changed(ctx, &a.user_id);
    ok(serde_json::json!({ "updated": true }))
}

fn h_delete_user(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserId = decode(args)?;
    refuse_own_account(ctx, &a.user_id, "delete")?;

    ctx.db
        .with_connection(|c| Ok(users::delete_user(c, &a.user_id)))
        .map_err(db_err)??;
    people_changed(ctx, &a.user_id);
    ok(serde_json::json!({ "deleted": true }))
}

/// An administrator setting someone else's password, for when they forgot it.
fn h_change_password(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: UserPassword = decode(args)?;
    ctx.db
        .with_connection(|c| Ok(users::change_password(c, &a.user_id, &a.new_password)))
        .map_err(db_err)??;
    // Deliberately no news: nothing about a password belongs in a log every
    // signed-in person can read.
    ok(serde_json::json!({ "updated": true }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnPassword {
    current_password: String,
    new_password: String,
}

/// Anyone may change their own password, proving the current one first.
///
/// Registered as `signed_in()` because it touches nothing but the caller's own
/// account, which every signed-in person may manage.
fn h_change_own_password(ctx: &BoundaryCtx, args: Value) -> Result<Value, BoundaryError> {
    let a: OwnPassword = decode(args)?;
    if ctx.actor.user_id == "local" {
        return Err(BoundaryError::invalid(
            "The person at the hosting computer signs in through the computer itself, \
             so there is no password here to change.",
        ));
    }
    let me = ctx.actor.user_id.clone();
    ctx.db
        .with_connection(|c| {
            Ok(users::change_own_password(c, &me, &a.current_password, &a.new_password))
        })
        .map_err(db_err)??;
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
    r.register("delete_user", w, h_delete_user);
    r.register("change_own_password", Required::signed_in(), h_change_own_password);

    r.register("list_devices", rd, h_list_devices);
    r.register("revoke_device", w, h_revoke_device);
}
