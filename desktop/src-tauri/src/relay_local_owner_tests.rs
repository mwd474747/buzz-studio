use super::build_nip98_auth_header;
use crate::app_state::build_app_state;
use nostr::Keys;
use reqwest::Method;

#[test]
fn nip98_auth_is_disabled_while_identity_is_lost() {
    let state = build_app_state();
    state
        .identity_lost
        .store(true, std::sync::atomic::Ordering::Release);

    let result = build_nip98_auth_header(
        &Method::GET,
        "http://localhost:3300/api/channels",
        b"",
        &state,
    );

    assert!(result.is_err());
}

#[test]
fn nip98_auth_is_disabled_while_keyring_is_locked() {
    let state = build_app_state();
    state
        .keyring_locked
        .store(true, std::sync::atomic::Ordering::Release);

    let result = build_nip98_auth_header(
        &Method::GET,
        "http://localhost:3300/api/channels",
        b"",
        &state,
    );

    assert!(result.is_err());
}

#[test]
fn explicit_signers_are_rejected_before_relay_io_in_local_owner_recovery() {
    let explicit = Keys::generate();
    for locked in [false, true] {
        let state = build_app_state();
        state
            .identity_lost
            .store(!locked, std::sync::atomic::Ordering::Release);
        state
            .keyring_locked
            .store(locked, std::sync::atomic::Ordering::Release);
        let result = crate::local_owner_profile::with_test_profile_active(|| {
            crate::local_owner_profile::require_explicit_signer(&state, &explicit)
        });
        assert!(result.is_err());
    }
}

#[test]
fn local_owner_profile_rejects_a_non_owner_explicit_signer() {
    let state = build_app_state();
    let explicit = Keys::generate();
    let result = crate::local_owner_profile::with_test_profile_active(|| {
        crate::local_owner_profile::require_explicit_signer(&state, &explicit)
    });
    assert!(result.is_err());
}
