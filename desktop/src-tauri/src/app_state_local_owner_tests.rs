#[test]
fn local_owner_build_ignores_runtime_private_key_injection() {
    let injected = Keys::generate();
    let nsec = injected.secret_key().to_bech32().unwrap();
    let state = with_env_key(Some(&nsec), || {
        crate::local_owner_profile::with_test_profile_active(build_app_state)
    });

    assert_eq!(state.identity_storage(), IdentityStorage::Ephemeral);
    assert_ne!(
        state.keys.lock().unwrap().public_key(),
        injected.public_key(),
        "compiled local-owner profile must never activate an env-injected key"
    );
}

#[test]
fn signing_keys_requires_exact_owner_when_profile_is_active() {
    let state = build_app_state();
    let result = crate::local_owner_profile::with_test_profile_active(|| state.signing_keys());
    assert!(
        result.is_err(),
        "a non-owner key must never sign in this flavor"
    );
}

// ── Local-owner profile: admit before mutation ──────────────────────────

#[test]
fn local_owner_exact_keyring_blocks_on_wrong_file_without_mutation() {
    let owner = Keys::generate();
    let wrong = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &wrong).unwrap();
    let owner_nsec = owner.secret_key().to_bech32().unwrap();
    let store = FakeIdentityStore::present_with(&owner_nsec);

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_key_eq(&owner, &resolved.keys);
    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert_eq!(
        store.slot.borrow().get(IDENTITY_KEY_NAME),
        Some(&owner_nsec)
    );
    assert!(store.deleted.borrow().is_empty());
    assert!(legacy_path.exists(), "wrong file must be left untouched");
    assert_key_eq(&wrong, &load_key_file(&legacy_path).unwrap());
}

#[test]
fn local_owner_exact_keyring_blocks_on_any_plaintext_identity_path() {
    let owner = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &owner).unwrap();
    let owner_nsec = owner.secret_key().to_bech32().unwrap();
    let store = FakeIdentityStore::present_with(&owner_nsec);

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_key_eq(&owner, &resolved.keys);
    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert!(legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
    assert_eq!(
        store.slot.borrow().get(IDENTITY_KEY_NAME),
        Some(&owner_nsec)
    );
}

#[test]
fn local_owner_wrong_keyring_enters_recovery_without_mutation() {
    let owner = Keys::generate();
    let wrong = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let wrong_nsec = wrong.secret_key().to_bech32().unwrap();
    let store = FakeIdentityStore::present_with(&wrong_nsec);

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert_eq!(
        store.slot.borrow().get(IDENTITY_KEY_NAME),
        Some(&wrong_nsec)
    );
    assert!(store.deleted.borrow().is_empty());
    assert!(!legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_rejects_legacy_keyring_entries_without_migration() {
    let owner = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let owner_nsec = owner.secret_key().to_bech32().unwrap();
    let store = FakeIdentityStore::present_with(&owner_nsec);
    store
        .slot
        .borrow_mut()
        .insert("agent:legacy".to_string(), "legacy-secret".to_string());

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert_eq!(
        store.slot.borrow().get(IDENTITY_KEY_NAME),
        Some(&owner_nsec)
    );
    assert_eq!(
        store.slot.borrow().get("agent:legacy").map(String::as_str),
        Some("legacy-secret")
    );
    assert!(store.deleted.borrow().is_empty());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_wrong_keyring_does_not_adopt_exact_owner_file() {
    let owner = Keys::generate();
    let wrong = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &owner).unwrap();
    let wrong_nsec = wrong.secret_key().to_bech32().unwrap();
    let store = FakeIdentityStore::present_with(&wrong_nsec);

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert_eq!(
        store.slot.borrow().get(IDENTITY_KEY_NAME),
        Some(&wrong_nsec),
        "boot must not replace a present keyring identity"
    );
    assert!(store.deleted.borrow().is_empty());
    assert!(legacy_path.exists());
    assert_key_eq(&owner, &load_key_file(&legacy_path).unwrap());
}

#[test]
fn local_owner_empty_keyring_never_reads_or_migrates_plaintext_identity() {
    let owner = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &owner).unwrap();
    let store = FakeIdentityStore::reachable_but_empty();

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert!(store.slot.borrow().is_empty());
    assert!(legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_import_refuses_existing_plaintext_before_keyring_mutation() {
    let owner = Keys::generate();
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &owner).unwrap();
    let store = FakeIdentityStore::reachable_but_empty();

    let result = persist_local_owner_keyring_only(&store, &owner, &legacy_path, dir.path());

    assert!(result.is_err());
    assert!(store.slot.borrow().is_empty());
    assert!(legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_import_has_no_plaintext_fallback_on_keyring_failure() {
    let owner = Keys::generate();
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::store_failing();

    let result = persist_local_owner_keyring_only(&store, &owner, &legacy_path, dir.path());

    assert!(result.is_err());
    assert!(!legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_import_requires_uncached_keyring_readback() {
    let owner = Keys::generate();
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::with_verify_failing();

    let result = persist_local_owner_keyring_only(&store, &owner, &legacy_path, dir.path());

    assert!(result.is_err());
    assert!(!legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_import_persists_only_the_owner_keyring_entry() {
    let owner = Keys::generate();
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::reachable_but_empty();

    let storage =
        persist_local_owner_keyring_only(&store, &owner, &legacy_path, dir.path()).unwrap();

    assert_eq!(storage, IdentityStorage::SystemKeyring);
    assert_eq!(store.slot.borrow().len(), 1);
    let persisted = store
        .slot
        .borrow()
        .get(IDENTITY_KEY_NAME)
        .cloned()
        .unwrap();
    assert_key_eq(&owner, &Keys::parse(&persisted).unwrap());
    assert!(!migration_marker_path(dir.path()).exists());
}

#[test]
fn local_owner_wrong_file_is_not_migrated_or_replaced() {
    let owner = Keys::generate();
    let wrong = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &wrong).unwrap();
    let store = FakeIdentityStore::reachable_but_empty();

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::Lost);
    assert!(store.slot.borrow().is_empty());
    assert!(store.deleted.borrow().is_empty());
    assert!(legacy_path.exists());
    assert_key_eq(&wrong, &load_key_file(&legacy_path).unwrap());
}

#[test]
fn local_owner_locked_keyring_opens_recovery_without_persistence() {
    let owner = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    let store = FakeIdentityStore::unreachable();

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::KeyringLocked);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert!(store.slot.borrow().is_empty());
    assert!(store.deleted.borrow().is_empty());
    assert!(!legacy_path.exists());
}

#[test]
fn local_owner_locked_keyring_does_not_sign_from_plaintext_fallback() {
    let owner = Keys::generate();
    let profile = crate::local_owner_profile::test_profile_for_owner(&owner.public_key().to_hex());
    let dir = tempfile::tempdir().unwrap();
    let legacy_path = dir.path().join("identity.key");
    save_key_file(&legacy_path, &owner).unwrap();
    let store = FakeIdentityStore::unreachable();

    let resolved =
        resolve_local_owner_with_store(&store, &profile, &legacy_path, dir.path()).unwrap();

    assert_eq!(resolved.recovery, RecoveryState::KeyringLocked);
    assert_eq!(resolved.storage, IdentityStorage::Ephemeral);
    assert!(legacy_path.exists());
    assert!(!migration_marker_path(dir.path()).exists());
    assert!(store.slot.borrow().is_empty());
}
