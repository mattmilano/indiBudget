//! A budget kept somewhere SQLite cannot keep it safely still opens, with a
//! warning that can be dismissed — and stays dismissed for that place.
//!
//! Kept in its own file so it runs in its own process: it sets
//! `INDIBUDGET_DATA_DIR`, and environment variables are process-wide.

use indibudget_lib::commands::AppState;

fn open() -> AppState {
    let state = AppState::new();
    state.init_database().expect("a risky place still opens");
    state
}

#[test]
fn a_budget_in_a_synced_folder_opens_with_a_warning_that_can_be_dismissed() {
    let scratch = std::env::temp_dir().join(format!("indibudget-unsafe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);

    // A folder Syncthing keeps in step with other computers.
    let synced = scratch.join("Sync");
    std::fs::create_dir_all(&synced).unwrap();
    std::fs::write(synced.join(".stfolder"), "").unwrap();
    std::env::set_var("INDIBUDGET_DATA_DIR", synced.join("indibudget"));

    let state = open();
    assert!(state.db.lock().unwrap().is_some(), "the budget must still open");
    let warning = state.location_warning().expect("a warning about the folder");
    assert!(warning.contains("Syncthing"), "{warning}");
    assert!(warning.contains("INDIBUDGET_DATA_DIR"), "{warning}");

    state.dismiss_location_warning().unwrap();
    assert_eq!(state.location_warning(), None, "dismissed");

    // Still dismissed after a restart.
    drop(state);
    let state = open();
    assert_eq!(state.location_warning(), None, "came back after a restart");
    drop(state);

    // A local folder never warns.
    std::env::set_var("INDIBUDGET_DATA_DIR", scratch.join("local"));
    let state = open();
    assert_eq!(state.location_warning(), None);
    drop(state);

    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn dismissing_with_nothing_to_warn_about_is_harmless() {
    let state = AppState::new();
    assert_eq!(state.location_warning(), None);
    state.dismiss_location_warning().unwrap();
}
