//! Startup refuses to open the budget somewhere SQLite cannot keep it safely,
//! and opens it once pointed at a local folder.
//!
//! Kept in its own file so it runs in its own process: it sets
//! `INDIBUDGET_DATA_DIR`, and environment variables are process-wide.

use indibudget_lib::commands::{AppState, InitError};

#[test]
fn a_budget_in_a_synced_folder_is_refused_and_a_local_folder_works() {
    let scratch = std::env::temp_dir().join(format!("indibudget-unsafe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);

    // A folder Syncthing keeps in step with other computers.
    let synced = scratch.join("Sync");
    std::fs::create_dir_all(&synced).unwrap();
    std::fs::write(synced.join(".stfolder"), "").unwrap();
    std::env::set_var("INDIBUDGET_DATA_DIR", synced.join("indibudget"));

    let state = AppState::new();
    match state.init_database() {
        Err(InitError::Refused { message }) => {
            assert!(message.contains("Syncthing"), "{message}");
            assert!(message.contains("INDIBUDGET_DATA_DIR"), "{message}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(state.db.lock().unwrap().is_none(), "nothing may be left open");
    assert!(
        !synced.join("indibudget").join("indibudget.db").exists(),
        "the refusal must come before the file is created, or the sync service already has it"
    );

    // Pointed at a local folder, it opens.
    let local = scratch.join("local");
    std::env::set_var("INDIBUDGET_DATA_DIR", &local);
    state.init_database().expect("a local folder opens");
    assert!(local.join("indibudget.db").exists());

    drop(state);
    let _ = std::fs::remove_dir_all(&scratch);
}
