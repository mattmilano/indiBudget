//! The registered commands, area by area.
//!
//! Registration is the declaration point: a command cannot appear here without
//! stating the area and access it needs. Anything absent from these modules
//! **does not exist remotely** — file dialogs, imports and backups that name a
//! path on disk, the encryption session, OS notifications, and the settings
//! store are all deliberately never registered. See `HOST_ONLY` below for the
//! list and the reason for each.

use serde::Serialize;
use serde_json::Value;

use super::leases::Leasable;
use super::news::Notice;
use super::registry::{encode, BoundaryCtx, Registry};
use super::{stamp_write, Area, BoundaryError, Stamped};

pub mod admin;
pub mod money;
pub mod planning;
pub mod reports;
pub mod structure;

/// Commands that stay on the hosting machine, and why.
///
/// Kept as data so the invariants test can assert none of them ever drifts
/// into the registry. Every entry either touches the host's own disk, its OS
/// session, or a secret that must not travel.
pub const HOST_ONLY: &[(&str, &str)] = &[
    ("init_app", "opens the database file on this machine"),
    ("get_database_path", "a path on the host's disk"),
    ("get_default_backup_path", "a path on the host's disk"),
    ("export_backup", "writes a file to the host's disk"),
    ("import_backup", "reads a file from the host's disk and replaces everything"),
    ("get_backup_info", "reads a file from the host's disk"),
    ("detect_import_columns", "reads a file from the host's disk"),
    ("preview_import", "reads a file from the host's disk"),
    ("import_transactions", "reads a file from the host's disk"),
    ("get_encryption_status", "the master-password session belongs to the host"),
    ("enable_encryption", "the master-password session belongs to the host"),
    ("disable_encryption", "the master-password session belongs to the host"),
    ("unlock_encryption", "the master-password session belongs to the host"),
    ("lock_encryption", "the master-password session belongs to the host"),
    ("change_encryption_password", "the master-password session belongs to the host"),
    ("send_bill_notification", "raises a notification on the host's desktop"),
    ("check_and_send_notifications", "raises notifications on the host's desktop"),
    // The settings store holds the host's own TLS private key alongside
    // ordinary preferences. A remote caller able to read an arbitrary key
    // could ask for that one, so the whole store stays local rather than
    // relying on a denylist of key names staying complete.
    ("get_setting", "the settings store holds this machine's private key"),
    ("set_setting", "the settings store holds this machine's private key"),
    ("delete_setting", "the settings store holds this machine's private key"),
];

/// Turn a repository error into a sentence.
pub fn db_err<E: std::fmt::Display>(e: E) -> BoundaryError {
    BoundaryError::internal(e.to_string())
}

/// What a write just did, so the boundary can finish the job around it.
pub struct Written<'a> {
    pub table: Stamped,
    pub area: Area,
    pub record_kind: &'static str,
    pub id: &'a str,
    pub is_new: bool,
    /// Set when this kind of record takes an edit hold, so a successful save
    /// can let go of it.
    pub leasable: Option<Leasable>,
}

/// The shape every swept write ends with.
///
/// Three things happen here and the order matters: the row is stamped with its
/// author, the author's holds on that kind are dropped because **the commit is
/// the letting-go**, and only then is any of it announced. Nothing is published
/// before the write has actually landed — a refused write changed nothing and
/// so announces nothing.
pub fn after_write(ctx: &BoundaryCtx, written: Written<'_>) -> Result<(), BoundaryError> {
    ctx.db
        .with_connection(|conn| {
            Ok(stamp_write(
                conn,
                written.table,
                written.id,
                ctx.actor,
                written.is_new,
            ))
        })
        .map_err(db_err)??;

    let mut freed = Vec::new();
    if let Some(kind) = written.leasable {
        freed = ctx.shared.leases.release_all_of_kind(kind, ctx.actor);
    }

    ctx.shared.news.publish(Notice::RecordChanged {
        area: written.area,
        record_kind: written.record_kind.to_string(),
        record_id: written.id.to_string(),
    });

    for key in freed {
        ctx.shared.news.publish(Notice::RecordFreed {
            area: key.kind.area(),
            record_kind: key.kind.label().to_string(),
            record_id: key.record_id,
        });
    }
    Ok(())
}

/// The same, for a row that no longer exists to stamp.
pub fn after_delete(
    ctx: &BoundaryCtx,
    area: Area,
    record_kind: &str,
    id: &str,
    leasable: Option<Leasable>,
) {
    let mut freed = Vec::new();
    if let Some(kind) = leasable {
        freed = ctx.shared.leases.release_all_of_kind(kind, ctx.actor);
    }

    ctx.shared.news.publish(Notice::RecordChanged {
        area,
        record_kind: record_kind.to_string(),
        record_id: id.to_string(),
    });

    for key in freed {
        ctx.shared.news.publish(Notice::RecordFreed {
            area: key.kind.area(),
            record_kind: key.kind.label().to_string(),
            record_id: key.record_id,
        });
    }
}

/// Encode a value that needed no announcement.
pub fn ok<T: Serialize>(value: T) -> Result<Value, BoundaryError> {
    encode(value)
}

/// Build the registry the host actually serves.
pub fn build_registry() -> Registry {
    let mut registry = Registry::new();
    money::register(&mut registry);
    planning::register(&mut registry);
    structure::register(&mut registry);
    reports::register(&mut registry);
    admin::register(&mut registry);
    super::leases::register(&mut registry);
    super::news::register(&mut registry);
    super::maintenance::register(&mut registry);
    registry
}
