//! `EZER_HOME` override tests in an isolated binary so `ezer_home()`'s process-wide `OnceLock` initializes from the overridden env var.

use std::path::PathBuf;

#[test]
#[serial_test::serial(EZER_HOME)]
fn ezer_home_override_path_helpers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ezer_home = tmp.path().to_path_buf();
    unsafe {
        std::env::set_var("EZER_HOME", &ezer_home);
    }

    assert_eq!(
        ezer_pager::util::pager_toml_path(),
        ezer_home.join("pager.toml")
    );
    assert_eq!(
        ezer_pager::util::display_ezer_home_prefix(),
        "$EZER_HOME"
    );
    assert_eq!(
        ezer_pager::util::display_user_ezer_path("config.toml"),
        "$EZER_HOME/config.toml"
    );

    let memory_path = ezer_home.join("memory/MEMORY.md");
    assert_eq!(
        ezer_pager::util::abbreviate_path(&memory_path.display().to_string()),
        "$EZER_HOME/memory/MEMORY.md"
    );

    // The copy toast abbreviates paths the same way, so a custom $EZER_HOME outside $HOME still shows the short form
    assert_eq!(
        ezer_pager::clipboard::display_copy_path(&ezer_home.join("last-copy.txt")),
        "$EZER_HOME/last-copy.txt"
    );

    assert!(ezer_pager::util::is_under_user_ezer_home(&memory_path));
    assert!(!ezer_pager::util::is_under_user_ezer_home(
        PathBuf::from("/tmp/other").as_path()
    ));
}

/// Isolated because `ezer_home()`'s `OnceLock` is already initialized by the time the shared lib-test binary reaches a case like this.
#[test]
#[serial_test::serial(EZER_HOME)]
fn disk_usage_run_creates_no_ezer_home() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ghost = tmp.path().join("ghost-home");
    unsafe {
        std::env::set_var("EZER_HOME", &ghost);
    }

    for json in [false, true] {
        ezer_pager::disk_usage_cmd::run(ezer_pager::disk_usage_cmd::DiskUsageArgs { json })
            .expect("a missing home is not an error");
        assert!(
            !ghost.exists(),
            "ezer du must not create the home it reports on (json={json})"
        );
    }
}
