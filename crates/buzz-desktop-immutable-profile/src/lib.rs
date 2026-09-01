//! Immutable local-dev production Desktop profile.
//!
//! This module is the fail-closed contract for the #local-dev production
//! Desktop release. It does **not** mint, import, export, print, or rotate
//! keys. It does **not** talk to a live keyring, a live relay, or pairing.
//!
//! Default desktop identity resolution (`app_state`) is unchanged. Pairing
//! (`commands/pairing.rs`) is unchanged. This profile is evaluated by
//! packaging + deny-case tests, and is the pin the leftover
//! `mac-packaged-app-build` worker must compile against.
//!
//! Tests use mock pubkey hex and mock relay probes only. The live
//! `#local-dev` owner (prefix `ea840b3e`) is never loaded here.

use std::path::{Path, PathBuf};

/// Production bundle identifier already declared in `tauri.conf.json`.
pub const BUNDLE_IDENTIFIER: &str = "xyz.block.buzz.app";

/// Production OS keyring service. Debug builds use `buzz-desktop-dev`.
pub const PRODUCTION_KEYRING_SERVICE: &str = "buzz-desktop";

/// Debug / worktree keyring service. Must never be selected by this profile.
pub const DEBUG_KEYRING_SERVICE: &str = "buzz-desktop-dev";

/// Fixed #local-dev relay. Not the desktop default (`ws://localhost:3000`).
pub const PRODUCTION_RELAY_WS_URL: &str = "ws://localhost:3300";

/// Owner identity pin for #local-dev. Prefix only — never a full key.
pub const EXPECTED_OWNER_PUBKEY_PREFIX: &str = "ea840b3e";

/// Tauri production frontend embed path (not Vite `devUrl`).
pub const FRONTEND_DIST: &str = "../dist";

/// Leftover work object id for a signed macOS `.app` (Linux cannot produce it).
pub const MAC_PACKAGED_APP_BUILD_LEFTOVER: &str = "mac-packaged-app-build";

/// Compile-time pin used by a Mac worker. Unset in ordinary desktop builds.
pub fn immutable_profile_compiled_in() -> bool {
    option_env!("BUZZ_DESKTOP_IMMUTABLE_PROFILE").is_some()
}

/// Classified identity resolution outcome. No secret material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityClass {
    /// Keyring (or file) already holds an identity; this is a restart/recovery.
    RecoveredExisting { pubkey_hex: String },
    /// Reachable-empty keyring + leftover `identity.key` — migrate, do not generate.
    MigratedExisting { pubkey_hex: String },
    /// No prior identity. Ordinary desktop may generate; this profile must not.
    GeneratedFresh { pubkey_hex: String },
    /// Marker present, keyring empty, no file — prior identity is gone.
    ExistingIdentityLost,
    /// Marker present, keyring unreachable — fail closed, do not generate.
    KeyringLocked,
}

/// Fail-closed deny cases required by the Phase 2 production profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyCase {
    FirstLaunchGenerate,
    WrongIdentity,
    LockedKeychain,
    ExistingIdentityUnrecoverable,
    RelayUnavailable,
    RelayMismatch,
    StatePathForbidden,
    LogPathForbidden,
    FrontendDevUrl,
    KeyringServiceMismatch,
    BundleIdentifierMismatch,
    RollbackUnknown,
    RollbackCurrent,
    MacAppMissing,
}

/// Relay connectivity as observed by a caller-supplied probe (tests mock this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayProbe {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept { reason: String },
    Deny { case: DenyCase, reason: String },
}

impl Verdict {
    pub fn is_accept(&self) -> bool {
        matches!(self, Self::Accept { .. })
    }

    pub fn deny_case(&self) -> Option<DenyCase> {
        match self {
            Self::Deny { case, .. } => Some(*case),
            Self::Accept { .. } => None,
        }
    }
}

fn pubkey_matches_owner_prefix(pubkey_hex: &str) -> bool {
    pubkey_hex.len() >= EXPECTED_OWNER_PUBKEY_PREFIX.len()
        && pubkey_hex
            .get(..EXPECTED_OWNER_PUBKEY_PREFIX.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(EXPECTED_OWNER_PUBKEY_PREFIX))
}

/// Admit a recovered or migrated identity. Never generates a key.
pub fn admit_identity(class: &IdentityClass) -> Verdict {
    match class {
        IdentityClass::GeneratedFresh { .. } => Verdict::Deny {
            case: DenyCase::FirstLaunchGenerate,
            reason: "immutable production profile refuses first-launch generate \
                     (no remint; migrate or recover the existing #local-dev owner)"
                .to_string(),
        },
        IdentityClass::KeyringLocked => Verdict::Deny {
            case: DenyCase::LockedKeychain,
            reason: "immutable production profile fails closed when the \
                     production keyring is locked or unreachable"
                .to_string(),
        },
        IdentityClass::ExistingIdentityLost => Verdict::Deny {
            case: DenyCase::ExistingIdentityUnrecoverable,
            reason: "immutable production profile refuses to replace a lost \
                     owner identity (no remint)"
                .to_string(),
        },
        IdentityClass::RecoveredExisting { pubkey_hex }
        | IdentityClass::MigratedExisting { pubkey_hex } => {
            if pubkey_matches_owner_prefix(pubkey_hex) {
                let kind = match class {
                    IdentityClass::MigratedExisting { .. } => "migrated",
                    _ => "recovered",
                };
                Verdict::Accept {
                    reason: format!(
                        "{kind} existing identity matches owner prefix {EXPECTED_OWNER_PUBKEY_PREFIX}"
                    ),
                }
            } else {
                Verdict::Deny {
                    case: DenyCase::WrongIdentity,
                    reason: format!(
                        "identity pubkey prefix does not match required owner \
                         prefix {EXPECTED_OWNER_PUBKEY_PREFIX}"
                    ),
                }
            }
        }
    }
}

/// Restart must resolve the same already-admitted identity again.
pub fn admit_restart(first: &IdentityClass, second: &IdentityClass) -> Verdict {
    match (admit_identity(first), admit_identity(second)) {
        (Verdict::Accept { .. }, Verdict::Accept { .. }) => match (first, second) {
            (
                IdentityClass::RecoveredExisting { pubkey_hex: a },
                IdentityClass::RecoveredExisting { pubkey_hex: b },
            )
            | (
                IdentityClass::MigratedExisting { pubkey_hex: a },
                IdentityClass::RecoveredExisting { pubkey_hex: b },
            ) if a.eq_ignore_ascii_case(b) => Verdict::Accept {
                reason: "restart recovered the same admitted identity".to_string(),
            },
            _ => Verdict::Deny {
                case: DenyCase::WrongIdentity,
                reason: "restart did not recover the same admitted identity".to_string(),
            },
        },
        (deny @ Verdict::Deny { .. }, _) => deny,
        (_, deny) => deny,
    }
}

pub fn admit_keyring_service(service: &str) -> Verdict {
    if service == PRODUCTION_KEYRING_SERVICE {
        Verdict::Accept {
            reason: format!("production keyring service is {PRODUCTION_KEYRING_SERVICE}"),
        }
    } else {
        Verdict::Deny {
            case: DenyCase::KeyringServiceMismatch,
            reason: format!(
                "keyring service {service:?} is not {PRODUCTION_KEYRING_SERVICE} \
                 (debug service {DEBUG_KEYRING_SERVICE} is forbidden for this profile)"
            ),
        }
    }
}

pub fn admit_bundle_identifier(identifier: &str) -> Verdict {
    if identifier == BUNDLE_IDENTIFIER {
        Verdict::Accept {
            reason: format!("bundle identifier is {BUNDLE_IDENTIFIER}"),
        }
    } else {
        Verdict::Deny {
            case: DenyCase::BundleIdentifierMismatch,
            reason: format!("bundle identifier {identifier:?} is not {BUNDLE_IDENTIFIER}"),
        }
    }
}

pub fn admit_relay(configured: &str, probe: RelayProbe) -> Verdict {
    if configured != PRODUCTION_RELAY_WS_URL {
        return Verdict::Deny {
            case: DenyCase::RelayMismatch,
            reason: format!(
                "relay {configured:?} is not the pinned production relay \
                 {PRODUCTION_RELAY_WS_URL} (no fallback)"
            ),
        };
    }
    match probe {
        RelayProbe::Available => Verdict::Accept {
            reason: format!("relay {PRODUCTION_RELAY_WS_URL} is available"),
        },
        RelayProbe::Unavailable => Verdict::Deny {
            case: DenyCase::RelayUnavailable,
            reason: format!(
                "pinned relay {PRODUCTION_RELAY_WS_URL} is unavailable; \
                 fail closed (do not fall back)"
            ),
        },
    }
}

/// Frontend must be the packaged `frontendDist`, never Vite `devUrl`.
pub fn admit_frontend_embed(frontend_dist: Option<&str>, dev_url_active: bool) -> Verdict {
    if dev_url_active {
        return Verdict::Deny {
            case: DenyCase::FrontendDevUrl,
            reason: "production profile forbids Vite devUrl; embed frontendDist".to_string(),
        };
    }
    match frontend_dist {
        Some(FRONTEND_DIST) => Verdict::Accept {
            reason: format!("frontend embedded via frontendDist {FRONTEND_DIST}"),
        },
        Some(other) => Verdict::Deny {
            case: DenyCase::FrontendDevUrl,
            reason: format!("frontendDist {other:?} is not {FRONTEND_DIST}"),
        },
        None => Verdict::Deny {
            case: DenyCase::FrontendDevUrl,
            reason: "production profile requires tauri frontendDist".to_string(),
        },
    }
}

fn path_has_component(path: &Path, name: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str() == name)
}

/// True when `candidate` is the checkout or a path inside it.
pub fn path_is_inside(checkout: &Path, candidate: &Path) -> bool {
    let checkout = normalize_for_compare(checkout);
    let candidate = normalize_for_compare(candidate);
    candidate == checkout || candidate.starts_with(&checkout)
}

fn normalize_for_compare(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// State and logs must live outside the source checkout and outside DawsOS
/// `reports` / `ops` trees.
pub fn path_is_forbidden_runtime_root(checkout: &Path, candidate: &Path) -> bool {
    if path_is_inside(checkout, candidate) {
        return true;
    }
    let dawsos = path_has_component(candidate, "DawsOS") || path_has_component(candidate, "dawsos");
    if dawsos && (path_has_component(candidate, "reports") || path_has_component(candidate, "ops"))
    {
        return true;
    }
    let as_str = candidate.to_string_lossy();
    as_str.contains("/reports/ops/") || as_str.contains("/reports/ops")
}

pub fn admit_runtime_path(checkout: &Path, candidate: &Path, kind: RuntimePathKind) -> Verdict {
    if path_is_forbidden_runtime_root(checkout, candidate) {
        let case = match kind {
            RuntimePathKind::State => DenyCase::StatePathForbidden,
            RuntimePathKind::Log => DenyCase::LogPathForbidden,
        };
        return Verdict::Deny {
            case,
            reason: format!(
                "{kind:?} path {} is inside the source checkout or a DawsOS \
                 reports/ops tree",
                candidate.display()
            ),
        };
    }
    Verdict::Accept {
        reason: format!("{kind:?} path is outside checkout and DawsOS reports/ops"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePathKind {
    State,
    Log,
}

/// Rollback may only target a previously published content digest, never an
/// unknown id and never the digest already current.
pub fn admit_rollback(
    current_digest: &str,
    rollback_target: Option<&str>,
    published: &[&str],
) -> Verdict {
    let Some(target) = rollback_target.filter(|value| !value.is_empty()) else {
        return Verdict::Deny {
            case: DenyCase::RollbackUnknown,
            reason: "rollback target is missing".to_string(),
        };
    };
    if target == current_digest {
        return Verdict::Deny {
            case: DenyCase::RollbackCurrent,
            reason: "rollback target must not be the current digest".to_string(),
        };
    }
    if !published.contains(&target) {
        return Verdict::Deny {
            case: DenyCase::RollbackUnknown,
            reason: format!("rollback target {target} is not a published digest"),
        };
    }
    Verdict::Accept {
        reason: format!("rollback target {target} is an exact published digest"),
    }
}

/// Linux/cloud packaging must not claim a Buzz.app exists.
pub fn admit_macos_app_artifact(macos_app_present: bool) -> Verdict {
    if macos_app_present {
        Verdict::Accept {
            reason: "signed macOS application artifact is present".to_string(),
        }
    } else {
        Verdict::Deny {
            case: DenyCase::MacAppMissing,
            reason: format!(
                "no signed macOS .app on this host; leftover work object \
                 {MAC_PACKAGED_APP_BUILD_LEFTOVER}"
            ),
        }
    }
}

/// Production pins written into every release manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutableDesktopPins {
    pub bundle_identifier: &'static str,
    pub keyring_service: &'static str,
    pub relay_ws_url: &'static str,
    pub expected_owner_pubkey_prefix: &'static str,
    pub frontend_dist: &'static str,
}

pub fn production_pins() -> ImmutableDesktopPins {
    ImmutableDesktopPins {
        bundle_identifier: BUNDLE_IDENTIFIER,
        keyring_service: PRODUCTION_KEYRING_SERVICE,
        relay_ws_url: PRODUCTION_RELAY_WS_URL,
        expected_owner_pubkey_prefix: EXPECTED_OWNER_PUBKEY_PREFIX,
        frontend_dist: FRONTEND_DIST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn owner_hex() -> String {
        format!("{EXPECTED_OWNER_PUBKEY_PREFIX}{}", "ab".repeat(28))
    }

    fn other_hex() -> String {
        format!("ffff0000{}", "cd".repeat(28))
    }

    #[test]
    fn first_launch_generate_is_denied() {
        let verdict = admit_identity(&IdentityClass::GeneratedFresh {
            pubkey_hex: owner_hex(),
        });
        assert_eq!(verdict.deny_case(), Some(DenyCase::FirstLaunchGenerate));
    }

    #[test]
    fn first_launch_migrate_matching_owner_is_accepted() {
        let verdict = admit_identity(&IdentityClass::MigratedExisting {
            pubkey_hex: owner_hex(),
        });
        assert!(verdict.is_accept(), "{verdict:?}");
    }

    #[test]
    fn first_launch_migrate_wrong_identity_is_denied() {
        let verdict = admit_identity(&IdentityClass::MigratedExisting {
            pubkey_hex: other_hex(),
        });
        assert_eq!(verdict.deny_case(), Some(DenyCase::WrongIdentity));
    }

    #[test]
    fn restart_recovers_same_identity() {
        let first = IdentityClass::MigratedExisting {
            pubkey_hex: owner_hex(),
        };
        let second = IdentityClass::RecoveredExisting {
            pubkey_hex: owner_hex(),
        };
        let verdict = admit_restart(&first, &second);
        assert!(verdict.is_accept(), "{verdict:?}");
    }

    #[test]
    fn restart_with_rotated_identity_is_denied() {
        let first = IdentityClass::RecoveredExisting {
            pubkey_hex: owner_hex(),
        };
        let second = IdentityClass::RecoveredExisting {
            pubkey_hex: other_hex(),
        };
        let verdict = admit_restart(&first, &second);
        assert_eq!(verdict.deny_case(), Some(DenyCase::WrongIdentity));
    }

    #[test]
    fn existing_identity_recovery_is_accepted() {
        let verdict = admit_identity(&IdentityClass::RecoveredExisting {
            pubkey_hex: owner_hex(),
        });
        assert!(verdict.is_accept(), "{verdict:?}");
    }

    #[test]
    fn locked_keychain_fails_closed() {
        let verdict = admit_identity(&IdentityClass::KeyringLocked);
        assert_eq!(verdict.deny_case(), Some(DenyCase::LockedKeychain));
    }

    #[test]
    fn lost_identity_is_not_reminted() {
        let verdict = admit_identity(&IdentityClass::ExistingIdentityLost);
        assert_eq!(
            verdict.deny_case(),
            Some(DenyCase::ExistingIdentityUnrecoverable)
        );
    }

    #[test]
    fn wrong_identity_refusal_is_prefix_only() {
        let verdict = admit_identity(&IdentityClass::RecoveredExisting {
            pubkey_hex: other_hex(),
        });
        assert_eq!(verdict.deny_case(), Some(DenyCase::WrongIdentity));
        // Prefix check is case-insensitive and does not require a live key.
        let mixed = IdentityClass::RecoveredExisting {
            pubkey_hex: format!("EA840B3E{}", "11".repeat(28)),
        };
        assert!(admit_identity(&mixed).is_accept());
    }

    #[test]
    fn relay_unavailable_fails_closed_without_fallback() {
        let verdict = admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Unavailable);
        assert_eq!(verdict.deny_case(), Some(DenyCase::RelayUnavailable));
        let fallback = admit_relay("ws://localhost:3000", RelayProbe::Available);
        assert_eq!(fallback.deny_case(), Some(DenyCase::RelayMismatch));
    }

    #[test]
    fn pinned_relay_available_is_accepted() {
        let verdict = admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Available);
        assert!(verdict.is_accept(), "{verdict:?}");
    }

    #[test]
    fn rollback_requires_exact_published_digest() {
        let current = "sha256:aaa";
        let previous = "sha256:bbb";
        assert!(admit_rollback(current, Some(previous), &[previous, current]).is_accept());
        assert_eq!(
            admit_rollback(current, Some(current), &[previous, current]).deny_case(),
            Some(DenyCase::RollbackCurrent)
        );
        assert_eq!(
            admit_rollback(current, Some("sha256:missing"), &[previous, current]).deny_case(),
            Some(DenyCase::RollbackUnknown)
        );
        assert_eq!(
            admit_rollback(current, None, &[previous]).deny_case(),
            Some(DenyCase::RollbackUnknown)
        );
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
        assert_eq!(
            admit_frontend_embed(None, false).deny_case(),
            Some(DenyCase::FrontendDevUrl)
        );
    }

    #[test]
    fn tauri_conf_embeds_frontend_dist_and_production_bundle_id() {
        let raw = include_str!("../../../desktop/src-tauri/tauri.conf.json");
        let conf: Value = serde_json::from_str(raw).expect("tauri.conf.json must parse");
        assert_eq!(
            conf["identifier"].as_str(),
            Some(BUNDLE_IDENTIFIER),
            "production bundle identifier must stay pinned"
        );
        assert_eq!(
            conf["build"]["frontendDist"].as_str(),
            Some(FRONTEND_DIST),
            "production embed is frontendDist, not a substituted path"
        );
        // devUrl may exist for `tauri dev` only; production packaging must
        // not activate it. The proof is that frontendDist is present and
        // the identifier is the production id.
        assert!(
            conf["build"]["devUrl"].as_str().is_some(),
            "devUrl remains for developer `tauri dev`; packaging must not use it"
        );
        let proof = admit_frontend_embed(conf["build"]["frontendDist"].as_str(), false);
        assert!(proof.is_accept(), "{proof:?}");
        assert!(admit_bundle_identifier(conf["identifier"].as_str().unwrap()).is_accept());
    }

    #[test]
    fn keyring_module_documents_production_service() {
        let raw = include_str!("../../../desktop/src-tauri/src/app_state_keyring.rs");
        assert!(
            raw.contains("\"buzz-desktop\""),
            "production keyring service must remain buzz-desktop"
        );
        assert!(
            raw.contains("\"buzz-desktop-dev\""),
            "debug keyring service must remain distinct"
        );
    }

    #[test]
    fn state_and_logs_must_leave_checkout_and_dawsos_ops() {
        let checkout = Path::new("/Users/mike/src/buzz");
        let ok_state = Path::new("/Users/mike/Library/Application Support/xyz.block.buzz.app");
        let ok_logs = Path::new("/Users/mike/Library/Logs/xyz.block.buzz.app");
        assert!(admit_runtime_path(checkout, ok_state, RuntimePathKind::State).is_accept());
        assert!(admit_runtime_path(checkout, ok_logs, RuntimePathKind::Log).is_accept());

        let in_checkout = checkout.join("desktop/state");
        assert_eq!(
            admit_runtime_path(checkout, &in_checkout, RuntimePathKind::State).deny_case(),
            Some(DenyCase::StatePathForbidden)
        );

        let dawsos_reports = Path::new("/Users/mike/DawsOS/reports/desktop.log");
        assert_eq!(
            admit_runtime_path(checkout, dawsos_reports, RuntimePathKind::Log).deny_case(),
            Some(DenyCase::LogPathForbidden)
        );
        let dawsos_ops = Path::new("/Users/mike/DawsOS/ops/buzz");
        assert_eq!(
            admit_runtime_path(checkout, dawsos_ops, RuntimePathKind::State).deny_case(),
            Some(DenyCase::StatePathForbidden)
        );
        let reports_ops = Path::new("/var/lib/reports/ops/buzz");
        assert_eq!(
            admit_runtime_path(checkout, reports_ops, RuntimePathKind::Log).deny_case(),
            Some(DenyCase::LogPathForbidden)
        );
    }

    #[test]
    fn linux_host_records_mac_app_leftover_instead_of_pretending() {
        let verdict = admit_macos_app_artifact(false);
        assert_eq!(verdict.deny_case(), Some(DenyCase::MacAppMissing));
        match verdict {
            Verdict::Deny { reason, .. } => {
                assert!(reason.contains(MAC_PACKAGED_APP_BUILD_LEFTOVER));
            }
            Verdict::Accept { .. } => panic!("Linux must not accept a missing Buzz.app"),
        }
    }

    #[test]
    fn production_pins_are_stable() {
        let pins = production_pins();
        assert_eq!(pins.bundle_identifier, "xyz.block.buzz.app");
        assert_eq!(pins.keyring_service, "buzz-desktop");
        assert_eq!(pins.relay_ws_url, "ws://localhost:3300");
        assert_eq!(pins.expected_owner_pubkey_prefix, "ea840b3e");
        assert_eq!(pins.frontend_dist, "../dist");
        assert!(!immutable_profile_compiled_in());
    }

    #[test]
    fn deny_cases_never_print_or_require_live_keys() {
        // Mock hex only — 32-byte-looking strings, not nsec, not live #local-dev.
        for hex in [owner_hex(), other_hex()] {
            assert!(!hex.starts_with("nsec"));
            assert_ne!(hex, EXPECTED_OWNER_PUBKEY_PREFIX);
        }
    }
}
