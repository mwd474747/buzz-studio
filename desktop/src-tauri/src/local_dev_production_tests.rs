use super::*;

fn other_hex() -> String {
    format!("ffff0000{}", "cd".repeat(28))
}

fn digest_only_app(digest: &str) -> MacosAppEvidence {
    MacosAppEvidence {
        artifact_digest: digest.to_string(),
        codesign_identity: None,
        team_id: None,
        notarization: None,
        stapled: false,
    }
}

fn structured_app(digest: &str) -> MacosAppEvidence {
    MacosAppEvidence {
        artifact_digest: digest.to_string(),
        codesign_identity: Some("Developer ID Application: Example (TEAMID1)".into()),
        team_id: Some("TEAMID1".into()),
        notarization: Some("notarization-ticket:example".into()),
        stapled: true,
    }
}

#[test]
fn in_tree_profile_does_not_invent_an_owner_key() {
    let profile = load_in_tree_profile().expect("in-tree profile must parse");
    assert_eq!(profile.profile, "local-dev-production");
    assert_eq!(profile.bundle_identifier, BUNDLE_IDENTIFIER);
    assert_eq!(profile.keyring_service, PRODUCTION_KEYRING_SERVICE);
    assert_eq!(profile.relay_ws_url, PRODUCTION_RELAY_WS_URL);
    assert_eq!(profile.owner_display_prefix, OWNER_DISPLAY_PREFIX);
    assert_eq!(profile.frontend_dist, FRONTEND_DIST);
    assert_eq!(profile.buzz_transport, BUZZ_TRANSPORT);
    assert!(profile.owner_pin_required);
    assert!(profile.desktop_requires_relay);
    assert!(profile.owner_pubkey.is_none());
    assert!(profile.owner_pubkey_sha256.is_none());
    assert_eq!(
        admit_owner_pin(&profile).deny_case(),
        Some(DenyCase::OwnerPinMissing)
    );
}

#[test]
fn compiled_in_pins_are_not_filled_from_env() {
    let prior = std::env::var("BUZZ_DESKTOP_OWNER_PUBKEY").ok();
    std::env::set_var("BUZZ_DESKTOP_OWNER_PUBKEY", fixture_owner_hex());
    let profile = active_profile().expect("compiled-in profile");
    match prior {
        Some(value) => std::env::set_var("BUZZ_DESKTOP_OWNER_PUBKEY", value),
        None => std::env::remove_var("BUZZ_DESKTOP_OWNER_PUBKEY"),
    }
    assert!(profile.owner_pubkey.is_none());
    assert_eq!(
        admit_owner_pin(&profile).deny_case(),
        Some(DenyCase::OwnerPinMissing)
    );
}

#[test]
fn missing_owner_pin_fails_closed() {
    let profile = fixture_profile(None, None);
    let verdict = admit_identity(
        &profile,
        &IdentityClass::RecoveredExisting {
            pubkey_hex: fixture_owner_hex(),
        },
    );
    assert_eq!(verdict.deny_case(), Some(DenyCase::OwnerPinMissing));
}

#[test]
fn first_launch_generate_is_denied() {
    let profile = fixture_profile(Some(fixture_owner_hex()), None);
    let verdict = admit_identity(
        &profile,
        &IdentityClass::GeneratedFresh {
            pubkey_hex: fixture_owner_hex(),
        },
    );
    assert_eq!(verdict.deny_case(), Some(DenyCase::FirstLaunchGenerate));
}

#[test]
fn recovered_owner_requires_exact_64_hex_match() {
    let profile = fixture_profile(Some(fixture_owner_hex()), None);
    assert!(admit_identity(
        &profile,
        &IdentityClass::RecoveredExisting {
            pubkey_hex: fixture_owner_hex(),
        },
    )
    .is_accept());
    let prefix_only = format!("{OWNER_DISPLAY_PREFIX}{}", "00".repeat(28));
    assert_eq!(
        admit_identity(
            &profile,
            &IdentityClass::RecoveredExisting {
                pubkey_hex: prefix_only,
            },
        )
        .deny_case(),
        Some(DenyCase::WrongIdentity)
    );
    assert_eq!(
        admit_identity(
            &profile,
            &IdentityClass::MigratedExisting {
                pubkey_hex: other_hex(),
            },
        )
        .deny_case(),
        Some(DenyCase::WrongIdentity)
    );
}

#[test]
fn digest_pin_matches_complete_key_not_prefix() {
    let owner = fixture_owner_hex();
    let digest = owner_pubkey_digest(&owner).unwrap();
    let profile = fixture_profile(None, Some(digest));
    assert!(admit_identity(
        &profile,
        &IdentityClass::RecoveredExisting { pubkey_hex: owner },
    )
    .is_accept());
    let prefix_collision = format!("{OWNER_DISPLAY_PREFIX}{}", "11".repeat(28));
    assert_eq!(
        admit_identity(
            &profile,
            &IdentityClass::RecoveredExisting {
                pubkey_hex: prefix_collision,
            },
        )
        .deny_case(),
        Some(DenyCase::WrongIdentity)
    );
}

#[test]
fn locked_and_lost_fail_closed() {
    let profile = fixture_profile(Some(fixture_owner_hex()), None);
    assert_eq!(
        admit_identity(&profile, &IdentityClass::KeyringLocked).deny_case(),
        Some(DenyCase::LockedKeychain)
    );
    assert_eq!(
        admit_identity(&profile, &IdentityClass::ExistingIdentityLost).deny_case(),
        Some(DenyCase::ExistingIdentityUnrecoverable)
    );
}

#[test]
fn relay_configuration_is_not_health() {
    assert!(admit_relay_configuration(PRODUCTION_RELAY_WS_URL).is_accept());
    assert_eq!(
        admit_relay_configuration("ws://localhost:3000").deny_case(),
        Some(DenyCase::RelayMismatch)
    );
    assert!(admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Available).is_accept());
    assert_eq!(
        admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Unavailable).deny_case(),
        Some(DenyCase::RelayUnavailable)
    );
}

#[test]
fn workspace_and_import_use_the_same_pin() {
    let owner = fixture_owner_hex();
    with_test_profile(fixture_profile(Some(owner.clone()), None), || {
        assert!(admit_workspace_apply(PRODUCTION_RELAY_WS_URL, None).is_ok());
        assert!(admit_workspace_apply(PRODUCTION_RELAY_WS_URL, Some(&owner)).is_ok());
        assert!(admit_workspace_apply("ws://localhost:3000", None).is_err());
        assert!(admit_workspace_apply(PRODUCTION_RELAY_WS_URL, Some(&other_hex())).is_err());
        assert!(admit_imported_identity(&owner).is_ok());
        assert!(admit_imported_identity(&other_hex()).is_err());
    });
}

#[test]
fn production_reset_is_refused() {
    assert!(deny_identity_erasing_reset().is_ok());
    with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        assert!(deny_identity_erasing_reset().is_err());
    });
}

#[test]
fn production_keyring_and_bundle_pins() {
    assert!(admit_keyring_service(PRODUCTION_KEYRING_SERVICE).is_accept());
    assert_eq!(
        admit_keyring_service(DEBUG_KEYRING_SERVICE).deny_case(),
        Some(DenyCase::KeyringServiceMismatch)
    );
    assert!(admit_bundle_identifier(BUNDLE_IDENTIFIER).is_accept());
    assert_eq!(
        admit_bundle_identifier("xyz.block.buzz.app.dev").deny_case(),
        Some(DenyCase::BundleIdentifierMismatch)
    );
}

#[test]
fn frontend_dist_not_dev_url() {
    assert!(admit_frontend_embed(Some(FRONTEND_DIST), false).is_accept());
    assert_eq!(
        admit_frontend_embed(Some(FRONTEND_DIST), true).deny_case(),
        Some(DenyCase::FrontendDevUrl)
    );
}

#[test]
fn tauri_conf_embeds_frontend_dist_and_production_bundle_id() {
    let raw = include_str!("../tauri.conf.json");
    let conf: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json");
    assert_eq!(conf["identifier"].as_str(), Some(BUNDLE_IDENTIFIER));
    assert_eq!(conf["build"]["frontendDist"].as_str(), Some(FRONTEND_DIST));
    assert!(admit_frontend_embed(conf["build"]["frontendDist"].as_str(), false).is_accept());
}

#[test]
fn transport_is_not_failed_because_desktop_is_absent() {
    assert!(admit_transport(TransportObservation::Ready).is_accept());
    assert_eq!(
        admit_transport(TransportObservation::FailedBecauseDesktopAbsent).deny_case(),
        Some(DenyCase::TransportDesktopOptional)
    );
}

#[test]
fn digest_is_not_signed_or_notarized_proof() {
    let digest = format!("sha256:{}", "ab".repeat(32));
    assert_eq!(
        admit_macos_app_artifact(None).deny_case(),
        Some(DenyCase::MacAppUnproven)
    );
    assert_eq!(
        admit_macos_app_artifact(Some(&digest_only_app(&digest))).deny_case(),
        Some(DenyCase::MacAppUnproven)
    );
    assert!(admit_macos_app_artifact(Some(&structured_app(&digest))).is_accept());
}

#[test]
fn rollback_authenticates_recomputed_tree() {
    let current = "sha256:aaa";
    let target = "sha256:bbb";
    assert!(admit_rollback_target(target, target, current).is_accept());
    assert_eq!(
        admit_rollback_target(target, "sha256:forged", current).deny_case(),
        Some(DenyCase::RollbackUnauthenticated)
    );
    assert_eq!(
        admit_rollback_target(current, current, current).deny_case(),
        Some(DenyCase::RollbackUnauthenticated)
    );
}

#[test]
fn state_and_logs_must_leave_checkout_and_dawsos_ops() {
    let checkout = Path::new("/Users/mike/src/buzz");
    let ok_state = Path::new("/Users/mike/Library/Application Support/xyz.block.buzz.app");
    let ok_logs = Path::new("/Users/mike/Library/Logs/xyz.block.buzz.app");
    assert!(admit_runtime_path(checkout, ok_state, RuntimePathKind::State).is_accept());
    assert!(admit_runtime_path(checkout, ok_logs, RuntimePathKind::Log).is_accept());
    assert_eq!(
        admit_runtime_path(
            checkout,
            &checkout.join("desktop/state"),
            RuntimePathKind::State
        )
        .deny_case(),
        Some(DenyCase::StatePathForbidden)
    );
    assert_eq!(
        admit_runtime_path(
            checkout,
            Path::new("/Users/mike/DawsOS/reports/desktop.log"),
            RuntimePathKind::Log
        )
        .deny_case(),
        Some(DenyCase::LogPathForbidden)
    );
}

#[test]
fn display_prefix_is_not_a_boundary() {
    let owner = fixture_owner_hex();
    assert_eq!(display_prefix(&owner), OWNER_DISPLAY_PREFIX);
    assert_ne!(owner, OWNER_DISPLAY_PREFIX);
    assert!(!owner.starts_with("nsec"));
}

#[test]
fn deny_first_launch_generate_respects_inactive_profile() {
    assert!(deny_first_launch_generate().is_ok());
    with_test_profile(fixture_profile(Some(fixture_owner_hex()), None), || {
        assert!(deny_first_launch_generate().is_err());
        assert!(profile_active());
    });
    assert!(!profile_active());
}
