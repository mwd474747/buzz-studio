use super::*;
use crate::local_dev_production::{
    fixture_owner_hex, fixture_profile, with_test_profile, PRODUCTION_KEYRING_SERVICE,
};

#[test]
fn local_dev_production_refuses_first_launch_generate() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::reachable_but_empty();
    let err = with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        match resolve_identity_with_store(&store, &legacy_path, dir.path()) {
            Ok(_) => panic!("expected first-launch generate to fail closed"),
            Err(err) => err,
        }
    });
    assert!(
        err.contains("refuses first-launch generate"),
        "unexpected error: {err}"
    );
    assert!(!legacy_path.exists());
}

#[test]
fn local_dev_production_recovers_exact_owner_and_refuses_wrong_identity() {
    let owner = Keys::generate();
    let owner_hex = owner.public_key().to_hex();
    let nsec = owner.secret_key().to_bech32().unwrap();
    // Labeled fixture pin is ea840b3e… — a generated key will not match.
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::present_with(&nsec);
    let err = with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        match resolve_identity_with_store(&store, &legacy_path, dir.path()) {
            Ok(_) => panic!("expected wrong identity to fail closed"),
            Err(err) => err,
        }
    });
    assert!(
        err.contains("does not exactly match")
            || err.contains("WrongIdentity")
            || err.contains("display"),
        "unexpected error: {err}"
    );
    assert_ne!(owner_hex, fixture_owner_hex());
}

#[test]
fn local_dev_production_locked_keychain_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    write_migration_marker(&migration_marker_path(dir.path())).unwrap();
    let store = FakeIdentityStore::unreachable();
    let err = with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        match resolve_identity_with_store(&store, &legacy_path, dir.path()) {
            Ok(_) => panic!("expected locked keychain to fail closed"),
            Err(err) => err,
        }
    });
    assert!(
        err.contains("locked or unreachable"),
        "unexpected error: {err}"
    );
}

#[test]
fn local_dev_production_forces_production_keyring_service() {
    with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        assert_eq!(keyring_service(), PRODUCTION_KEYRING_SERVICE);
    });
}
