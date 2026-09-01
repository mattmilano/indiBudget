//! Identities and grants.
//!
//! An app password is a gate — it answers "may anyone in?". This module
//! answers "who is asking?", which is what every later phase needs: an actor
//! to authorize, a name to put in a refusal, and an author to stamp on a row.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Access, Actor, Area, BoundaryError, Grants};

const MIN_PASSWORD_LENGTH: usize = 8;

/// The single sentence every sign-in failure gets.
///
/// Never "no such user" versus "wrong password" — the difference is how an
/// attacker learns which logins are real.
const SIGN_IN_FAILED: &str = "That login and password do not match.";

/// A person, as everything above the boundary sees them.
///
/// Deliberately carries no password hash. This struct is serialised to the
/// frontend and across the wire, and a hash that cannot travel cannot leak.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub is_owner: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub row_version: i64,
}

/// Argon2id parameters for verifying a sign-in.
///
/// These are lighter than the 64 MB used to derive the data-at-rest encryption
/// key, and deliberately so. Key derivation runs once when someone unlocks the
/// file and guards the database if the disk is stolen, which is worth the cost.
/// Password verification runs on *every* sign-in attempt, including hostile
/// ones, so an extremely expensive hash becomes a denial-of-service lever
/// against the host machine. These are the OWASP Argon2id baseline
/// (19 MiB, t=2, p=1), which resists offline cracking without handing anyone a
/// way to exhaust the host's memory by hammering the login.
fn hasher() -> Result<Argon2<'static>, BoundaryError> {
    let params = Params::new(19_456, 2, 1, None)
        .map_err(|e| BoundaryError::internal(format!("Could not configure password hashing: {e}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub fn hash_password(password: &str) -> Result<String, BoundaryError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| BoundaryError::internal(format!("Could not secure that password: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(stored_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    let Ok(argon2) = hasher() else {
        return false;
    };
    argon2
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn validate_password_strength(password: &str) -> Result<(), BoundaryError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(BoundaryError::invalid(format!(
            "Password must be at least {MIN_PASSWORD_LENGTH} characters."
        )));
    }
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_upper || !has_lower || !has_digit {
        return Err(BoundaryError::invalid(
            "Password must contain an uppercase letter, a lowercase letter, and a number.",
        ));
    }
    Ok(())
}

fn area_key(area: Area) -> &'static str {
    match area {
        Area::Money => "money",
        Area::Planning => "planning",
        Area::Structure => "structure",
        Area::Reports => "reports",
        Area::Admin => "admin",
    }
}

fn area_from_key(key: &str) -> Option<Area> {
    match key {
        "money" => Some(Area::Money),
        "planning" => Some(Area::Planning),
        "structure" => Some(Area::Structure),
        "reports" => Some(Area::Reports),
        "admin" => Some(Area::Admin),
        _ => None,
    }
}

fn access_key(access: Access) -> &'static str {
    match access {
        Access::None => "none",
        Access::Read => "read",
        Access::Write => "write",
    }
}

fn access_from_key(key: &str) -> Access {
    match key {
        "write" => Access::Write,
        "read" => Access::Read,
        _ => Access::None,
    }
}

fn map_db(e: rusqlite::Error) -> BoundaryError {
    BoundaryError::internal(format!("Could not reach the user list: {e}"))
}

fn row_to_user(row: &rusqlite::Row) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get("id")?,
        login: row.get("login")?,
        display_name: row.get("display_name")?,
        is_owner: row.get::<_, i64>("is_owner")? != 0,
        is_active: row.get::<_, i64>("is_active")? != 0,
        created_at: row
            .get::<_, String>("created_at")?
            .parse()
            .unwrap_or_else(|_| Utc::now()),
        updated_at: row
            .get::<_, String>("updated_at")?
            .parse()
            .unwrap_or_else(|_| Utc::now()),
        row_version: row.get("row_version")?,
    })
}

pub fn count_users(conn: &Connection) -> Result<i64, BoundaryError> {
    conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
        .map_err(map_db)
}

/// True when nobody has been created yet, so the app is still single-user.
pub fn is_single_user(conn: &Connection) -> Result<bool, BoundaryError> {
    Ok(count_users(conn)? == 0)
}

pub fn list_users(conn: &Connection) -> Result<Vec<User>, BoundaryError> {
    let mut stmt = conn
        .prepare("SELECT * FROM users ORDER BY is_owner DESC, display_name COLLATE NOCASE")
        .map_err(map_db)?;
    let users = stmt
        .query_map([], row_to_user)
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?;
    Ok(users)
}

pub fn get_user(conn: &Connection, user_id: &str) -> Result<Option<User>, BoundaryError> {
    conn.query_row("SELECT * FROM users WHERE id = ?1", [user_id], row_to_user)
        .optional()
        .map_err(map_db)
}

pub fn create_user(
    conn: &Connection,
    login: &str,
    display_name: &str,
    password: &str,
    is_owner: bool,
    grants: &Grants,
    created_by: Option<&str>,
) -> Result<User, BoundaryError> {
    let login = login.trim();
    if login.is_empty() {
        return Err(BoundaryError::invalid("A login name is required."));
    }
    if display_name.trim().is_empty() {
        return Err(BoundaryError::invalid("A display name is required."));
    }
    validate_password_strength(password)?;

    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE login = ?1 COLLATE NOCASE",
            [login],
            |row| row.get(0),
        )
        .map_err(map_db)?;
    if exists > 0 {
        return Err(BoundaryError::invalid(format!(
            "Someone is already using the login \"{login}\"."
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users
           (id, login, display_name, password_hash, is_owner, is_active,
            created_at, updated_at, row_version, created_by, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6, 1, ?7, ?7)",
        params![
            &id,
            login,
            display_name.trim(),
            hash_password(password)?,
            is_owner as i64,
            &now,
            created_by,
        ],
    )
    .map_err(map_db)?;

    set_grants(conn, &id, grants)?;

    get_user(conn, &id)?.ok_or_else(|| BoundaryError::internal("The new user could not be read back."))
}

pub fn set_grants(conn: &Connection, user_id: &str, grants: &Grants) -> Result<(), BoundaryError> {
    conn.execute("DELETE FROM user_grants WHERE user_id = ?1", [user_id])
        .map_err(map_db)?;
    for area in Area::ALL {
        let access = grants.access(area);
        if access == Access::None {
            continue;
        }
        conn.execute(
            "INSERT INTO user_grants (user_id, area, access) VALUES (?1, ?2, ?3)",
            params![user_id, area_key(area), access_key(access)],
        )
        .map_err(map_db)?;
    }
    Ok(())
}

/// Read a person's stored grants.
///
/// Note this returns only what the rows say. It does **not** apply the owner
/// rule — that lives in `Actor::new`, so there is exactly one place where an
/// owner's access is decided and it cannot be bypassed by a caller who reads
/// grants directly.
pub fn grants_for(conn: &Connection, user_id: &str) -> Result<Grants, BoundaryError> {
    let mut stmt = conn
        .prepare("SELECT area, access FROM user_grants WHERE user_id = ?1")
        .map_err(map_db)?;
    let rows = stmt
        .query_map([user_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?;

    let mut grants = Grants::none();
    for (area, access) in rows {
        if let Some(area) = area_from_key(&area) {
            grants = grants.with(area, access_from_key(&access));
        }
    }
    Ok(grants)
}

pub fn set_active(conn: &Connection, user_id: &str, is_active: bool) -> Result<(), BoundaryError> {
    let changed = conn
        .execute(
            "UPDATE users SET is_active = ?1, updated_at = ?2 WHERE id = ?3",
            params![is_active as i64, Utc::now().to_rfc3339(), user_id],
        )
        .map_err(map_db)?;
    if changed == 0 {
        return Err(BoundaryError::invalid("That person is no longer in the list."));
    }
    Ok(())
}

pub fn change_password(
    conn: &Connection,
    user_id: &str,
    new_password: &str,
) -> Result<(), BoundaryError> {
    validate_password_strength(new_password)?;
    let changed = conn
        .execute(
            "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
            params![hash_password(new_password)?, Utc::now().to_rfc3339(), user_id],
        )
        .map_err(map_db)?;
    if changed == 0 {
        return Err(BoundaryError::invalid("That person is no longer in the list."));
    }
    Ok(())
}

/// Verify a sign-in and build the actor for it.
///
/// Every failure returns the same sentence, and an unknown login still pays
/// the cost of a hash verification so that response time does not reveal which
/// logins exist.
pub fn authenticate(
    conn: &Connection,
    login: &str,
    password: &str,
) -> Result<Actor, BoundaryError> {
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT id, password_hash FROM users
             WHERE login = ?1 COLLATE NOCASE AND is_active = 1",
            [login.trim()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_db)?;

    let Some((user_id, stored_hash)) = found else {
        // Burn comparable time against a throwaway hash so that "no such
        // login" and "wrong password" cannot be told apart by a stopwatch.
        let decoy = hash_password("decoy-password-Aa1").unwrap_or_default();
        let _ = verify_password(&decoy, password);
        return Err(BoundaryError::invalid(SIGN_IN_FAILED));
    };

    if !verify_password(&stored_hash, password) {
        return Err(BoundaryError::invalid(SIGN_IN_FAILED));
    }

    let user = get_user(conn, &user_id)?
        .ok_or_else(|| BoundaryError::invalid(SIGN_IN_FAILED))?;
    let grants = grants_for(conn, &user_id)?;

    Ok(Actor::new(
        user.id,
        user.display_name,
        user.is_owner,
        grants,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::Required;
    use crate::database::Database;

    fn db() -> Database {
        Database::in_memory().expect("in-memory db")
    }

    fn member_grants() -> Grants {
        Grants::none()
            .with(Area::Money, Access::Write)
            .with(Area::Reports, Access::Read)
    }

    #[test]
    fn a_fresh_database_is_single_user() {
        let db = db();
        db.with_connection(|conn| {
            assert!(is_single_user(conn).unwrap());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_created_user_can_sign_in() {
        let db = db();
        db.with_connection(|conn| {
            create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
            let actor = authenticate(conn, "sam", "Password1").unwrap();
            assert_eq!(actor.display_name, "Sam");
            assert!(actor.is_owner);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn logins_are_case_insensitive() {
        let db = db();
        db.with_connection(|conn| {
            create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
            assert!(authenticate(conn, "SAM", "Password1").is_ok());
            assert!(authenticate(conn, "Sam", "Password1").is_ok());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_login_cannot_be_taken_twice_even_in_a_different_case() {
        let db = db();
        db.with_connection(|conn| {
            create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
            let err = create_user(conn, "SAM", "Samuel", "Password1", false, &Grants::none(), None)
                .unwrap_err();
            assert!(err.sentence().contains("already using"), "{}", err.sentence());
            Ok(())
        })
        .unwrap();
    }

    /// Both failure modes must be indistinguishable to a caller.
    #[test]
    fn every_sign_in_failure_gives_the_same_sentence() {
        let db = db();
        db.with_connection(|conn| {
            create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();

            let wrong_password = authenticate(conn, "sam", "Wrong1234").unwrap_err();
            let no_such_login = authenticate(conn, "nobody", "Wrong1234").unwrap_err();

            assert_eq!(wrong_password.sentence(), no_such_login.sentence());
            assert_eq!(wrong_password.sentence(), SIGN_IN_FAILED);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_deactivated_person_cannot_sign_in_and_is_not_told_why() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None)
                    .unwrap();
            assert!(authenticate(conn, "alex", "Password1").is_ok());

            set_active(conn, &user.id, false).unwrap();
            let err = authenticate(conn, "alex", "Password1").unwrap_err();
            assert_eq!(err.sentence(), SIGN_IN_FAILED);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn the_password_hash_never_appears_in_a_serialised_user() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "sam", "Sam", "Password1", true, &Grants::all(), None).unwrap();
            let encoded = serde_json::to_string(&user).unwrap();
            assert!(!encoded.contains("password"), "{encoded}");
            assert!(!encoded.contains("argon2"), "{encoded}");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn stored_hashes_are_argon2id_and_salted_per_user() {
        let first = hash_password("Password1").unwrap();
        let second = hash_password("Password1").unwrap();
        assert!(first.starts_with("$argon2id$"), "{first}");
        assert_ne!(first, second, "identical passwords must not share a hash");
        assert!(verify_password(&first, "Password1"));
        assert!(verify_password(&second, "Password1"));
        assert!(!verify_password(&first, "Password2"));
    }

    #[test]
    fn weak_passwords_are_refused_with_a_reason() {
        assert!(validate_password_strength("short1A").is_err());
        assert!(validate_password_strength("alllowercase1").is_err());
        assert!(validate_password_strength("NoDigitsHere").is_err());
        assert!(validate_password_strength("Password1").is_ok());
    }

    #[test]
    fn grants_round_trip_through_storage() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None)
                    .unwrap();
            let loaded = grants_for(conn, &user.id).unwrap();
            assert_eq!(loaded.access(Area::Money), Access::Write);
            assert_eq!(loaded.access(Area::Reports), Access::Read);
            assert_eq!(loaded.access(Area::Admin), Access::None);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn resetting_grants_replaces_rather_than_accumulates() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None)
                    .unwrap();

            set_grants(conn, &user.id, &Grants::none().with(Area::Reports, Access::Read))
                .unwrap();

            let loaded = grants_for(conn, &user.id).unwrap();
            assert_eq!(loaded.access(Area::Money), Access::None);
            assert_eq!(loaded.access(Area::Reports), Access::Read);
            Ok(())
        })
        .unwrap();
    }

    /// Trap #1 again, this time end-to-end through storage: an owner created
    /// with no grant rows at all must still sign in able to reach everything.
    #[test]
    fn an_owner_stored_with_no_grant_rows_signs_in_with_full_access() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "sam", "Sam", "Password1", true, &Grants::none(), None).unwrap();

            let stored = grants_for(conn, &user.id).unwrap();
            assert!(stored.is_empty(), "no grant rows should have been written");

            let actor = authenticate(conn, "sam", "Password1").unwrap();
            for area in Area::ALL {
                assert!(
                    actor.grants().allows(Required::write(area)),
                    "owner refused write on {}",
                    area.label()
                );
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_member_signs_in_with_exactly_their_stored_grants() {
        let db = db();
        db.with_connection(|conn| {
            create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None).unwrap();
            let actor = authenticate(conn, "alex", "Password1").unwrap();

            assert!(actor.grants().allows(Required::write(Area::Money)));
            assert!(actor.grants().allows(Required::read(Area::Reports)));
            assert!(!actor.grants().allows(Required::write(Area::Reports)));
            assert!(!actor.grants().allows(Required::read(Area::Admin)));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn changing_a_password_invalidates_the_old_one() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None)
                    .unwrap();
            change_password(conn, &user.id, "Password2").unwrap();

            assert!(authenticate(conn, "alex", "Password1").is_err());
            assert!(authenticate(conn, "alex", "Password2").is_ok());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn deleting_a_user_takes_their_grants_with_them() {
        let db = db();
        db.with_connection(|conn| {
            let user =
                create_user(conn, "alex", "Alex", "Password1", false, &member_grants(), None)
                    .unwrap();
            conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
            conn.execute("DELETE FROM users WHERE id = ?1", [&user.id])
                .unwrap();

            let orphaned: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM user_grants WHERE user_id = ?1",
                    [&user.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(orphaned, 0, "grants outlived their user");
            Ok(())
        })
        .unwrap();
    }
}
