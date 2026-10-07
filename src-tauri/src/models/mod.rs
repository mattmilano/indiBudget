pub mod account;
pub mod budget;
pub mod category;
pub mod goal;
pub mod recurring;
pub mod transaction;

pub use account::*;
pub use budget::*;
pub use category::*;
pub use goal::*;
pub use recurring::*;
pub use transaction::*;

/// The version a row starts at.
///
/// The `row_version` column defaults to 1 and only the AFTER UPDATE triggers
/// ever move it, so a record built in memory, or read from a backup written
/// before the column existed, is described truthfully as a row that has not
/// yet been changed.
pub fn first_row_version() -> i64 {
    1
}
