//! Local-dev production profile for the existing Desktop release lane.
//!
//! Pins and fail-closed admission live here so the Tauri boot path
//! (`app_state`, `relay`, `keyring`) depends on them. This is not a sidecar
//! crate. It does not mint, import, export, print, or rotate keys. It never
//! prints `nsec`.
//!
//! The in-tree profile (`.release/local-dev-production.json`) does **not**
//! invent the canonical owner public key. Display prefix `ea840b3e` is for
//! display only. Boot and live admission require a complete 64-hex pin or a
//! SHA-256 digest supplied by config/env.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Same file `scripts/desktop_release.py` owns under `.release/`.
const PROFILE_JSON: &str = include_str!("../../../.release/local-dev-production.json");

pub const BUNDLE_IDENTIFIER: &str = "xyz.block.buzz.app";
pub const PRODUCTION_KEYRING_SERVICE: &str = "buzz-desktop";
pub const DEBUG_KEYRING_SERVICE: &str = "buzz-desktop-dev";
pub const PRODUCTION_RELAY_WS_URL: &str = "ws://localhost:3300";
pub const OWNER_DISPLAY_PREFIX: &str = "ea840b3e";
pub const FRONTEND_DIST: &str = "../dist";
pub const MAC_PACKAGED_APP_BUILD_LEFTOVER: &str = "mac-packaged-app-build";

const PUBKEY_HEX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDevProductionProfile {
    pub profile: String,
    pub bundle_identifier: String,
    pub keyring_service: String,
    pub relay_ws_url: String,
    pub owner_display_prefix: String,
    pub owner_pubkey: Option<String>,
    pub owner_pubkey_sha256: Option<String>,
    pub frontend_dist: String,
    pub buzz_transport: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyCase {
    FirstLaunchGenerate,
    WrongIdentity,
    LockedKeychain,
    ExistingIdentityUnrecoverable,
    OwnerPinMissing,
    RelayUnavailable,
    RelayMismatch,
    StatePathForbidden,
    LogPathForbidden,
    FrontendDevUrl,
    KeyringServiceMismatch,
    BundleIdentifierMismatch,
    RollbackUnauthenticated,
    MacAppUnproven,
    TransportDesktopOptional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityClass {
    RecoveredExisting { pubkey_hex: String },
    MigratedExisting { pubkey_hex: String },
    GeneratedFresh { pubkey_hex: String },
    ExistingIdentityLost,
    KeyringLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayProbe {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportObservation {
    /// Transport is healthy. Desktop may be absent.
    Ready,
    /// Transport failed for a reason other than Desktop absence.
    Failed { reason: &'static str },
    /// Illegal: transport marked failed only because Desktop is absent.
    FailedBecauseDesktopAbsent,
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

    pub fn into_result(self) -> Result<(), String> {
        match self {
            Self::Accept { .. } => Ok(()),
            Self::Deny { reason, .. } => Err(reason),
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_OVERRIDE: std::cell::RefCell<Option<LocalDevProductionProfile>> =
        const { std::cell::RefCell::new(None) };
}

/// Compile-time or runtime activation of the local-dev production profile.
pub fn profile_active() -> bool {
    #[cfg(test)]
    if TEST_OVERRIDE.with(|slot| slot.borrow().is_some()) {
        return true;
    }
    option_env!("BUZZ_DESKTOP_LOCAL_DEV_PRODUCTION").is_some()
        || std::env::var("BUZZ_DESKTOP_LOCAL_DEV_PRODUCTION")
            .ok()
            .is_some_and(|value| !value.is_empty())
}

/// First-launch generate is forbidden while the production profile is active.
pub fn refuses_first_launch_generate() -> bool {
    profile_active()
}

pub fn load_in_tree_profile() -> Result<LocalDevProductionProfile, String> {
    parse_profile_json(PROFILE_JSON)
}

pub fn active_profile() -> Result<LocalDevProductionProfile, String> {
    #[cfg(test)]
    if let Some(profile) = TEST_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Ok(profile);
    }
    let mut profile = load_in_tree_profile()?;
    if let Some(pubkey) = env_nonempty("BUZZ_DESKTOP_OWNER_PUBKEY") {
        profile.owner_pubkey = Some(normalize_owner_pubkey(&pubkey)?);
    }
    if let Some(digest) = env_nonempty("BUZZ_DESKTOP_OWNER_PUBKEY_SHA256") {
        profile.owner_pubkey_sha256 = Some(normalize_digest(&digest)?);
    }
    Ok(profile)
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_profile_json(raw: &str) -> Result<LocalDevProductionProfile, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("local-dev production profile: {e}"))?;
    let get_str = |key: &str| -> Result<String, String> {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("local-dev production profile missing {key}"))
    };
    let optional_pin = |key: &str| -> Result<Option<String>, String> {
        match value.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!(
                "local-dev production profile {key} must be a string or null"
            )),
        }
    };
    Ok(LocalDevProductionProfile {
        profile: get_str("profile")?,
        bundle_identifier: get_str("bundle_identifier")?,
        keyring_service: get_str("keyring_service")?,
        relay_ws_url: get_str("relay_ws_url")?,
        owner_display_prefix: get_str("owner_display_prefix")?,
        owner_pubkey: optional_pin("owner_pubkey")?,
        owner_pubkey_sha256: optional_pin("owner_pubkey_sha256")?,
        frontend_dist: get_str("frontend_dist")?,
        buzz_transport: get_str("buzz_transport")?,
    })
}

fn normalize_owner_pubkey(raw: &str) -> Result<String, String> {
    let hex = raw.trim().to_ascii_lowercase();
    if hex.len() != PUBKEY_HEX_LEN || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "owner pubkey pin must be the complete 64-char lowercase hex public key".to_string(),
        );
    }
    Ok(hex)
}

fn normalize_digest(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_ascii_lowercase();
    let hex = value.strip_prefix("sha256:").unwrap_or(&value);
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("owner pubkey digest must be sha256:<64 hex>".to_string());
    }
    Ok(format!("sha256:{hex}"))
}

/// SHA-256 of the 32-byte x-only public key. Used when the full key is pinned
/// by digest rather than by embedding the hex in-tree.
pub fn owner_pubkey_digest(pubkey_hex: &str) -> Result<String, String> {
    let hex = normalize_owner_pubkey(pubkey_hex)?;
    let bytes = hex::decode(&hex).map_err(|e| format!("owner pubkey hex: {e}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

pub fn display_prefix(pubkey_hex: &str) -> String {
    pubkey_hex
        .get(..OWNER_DISPLAY_PREFIX.len())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn resolved_owner_pin(
    profile: &LocalDevProductionProfile,
) -> Result<(Option<String>, Option<String>), String> {
    let pubkey = match profile.owner_pubkey.as_deref() {
        None => None,
        Some(raw) => Some(normalize_owner_pubkey(raw)?),
    };
    let digest = match profile.owner_pubkey_sha256.as_deref() {
        None => None,
        Some(raw) => Some(normalize_digest(raw)?),
    };
    if pubkey.is_none() && digest.is_none() {
        return Err("local-dev production profile has no owner public-key pin \
             (set owner_pubkey or owner_pubkey_sha256; do not invent a key)"
            .to_string());
    }
    if let Some(ref hex) = pubkey {
        if !hex.starts_with(OWNER_DISPLAY_PREFIX) {
            return Err(format!(
                "owner pubkey pin display prefix {} does not match {}",
                display_prefix(hex),
                OWNER_DISPLAY_PREFIX
            ));
        }
        if let Some(ref expected) = digest {
            let actual = owner_pubkey_digest(hex)?;
            if &actual != expected {
                return Err("owner pubkey pin does not match owner_pubkey_sha256".to_string());
            }
        }
    }
    Ok((pubkey, digest))
}

pub fn admit_owner_pin(profile: &LocalDevProductionProfile) -> Verdict {
    match resolved_owner_pin(profile) {
        Ok(_) => Verdict::Accept {
            reason: "owner public-key pin is present (exact hex or digest)".to_string(),
        },
        Err(reason) => Verdict::Deny {
            case: DenyCase::OwnerPinMissing,
            reason,
        },
    }
}

fn pubkey_matches_pin(profile: &LocalDevProductionProfile, pubkey_hex: &str) -> Verdict {
    let hex = match normalize_owner_pubkey(pubkey_hex) {
        Ok(hex) => hex,
        Err(reason) => {
            return Verdict::Deny {
                case: DenyCase::WrongIdentity,
                reason,
            };
        }
    };
    let (pinned, digest) = match resolved_owner_pin(profile) {
        Ok(pins) => pins,
        Err(reason) => {
            return Verdict::Deny {
                case: DenyCase::OwnerPinMissing,
                reason,
            };
        }
    };
    if let Some(expected) = pinned {
        if hex != expected {
            return Verdict::Deny {
                case: DenyCase::WrongIdentity,
                reason: format!(
                    "identity {} does not exactly match the pinned owner public key \
                     (display {})",
                    display_prefix(&hex),
                    OWNER_DISPLAY_PREFIX
                ),
            };
        }
        return Verdict::Accept {
            reason: format!(
                "identity matches pinned owner public key (display {OWNER_DISPLAY_PREFIX})"
            ),
        };
    }
    match digest {
        Some(expected) => match owner_pubkey_digest(&hex) {
            Ok(actual) if actual == expected => Verdict::Accept {
                reason: "identity matches pinned owner public-key digest".to_string(),
            },
            Ok(_) => Verdict::Deny {
                case: DenyCase::WrongIdentity,
                reason: "identity public-key digest does not match pin".to_string(),
            },
            Err(reason) => Verdict::Deny {
                case: DenyCase::WrongIdentity,
                reason,
            },
        },
        None => Verdict::Deny {
            case: DenyCase::OwnerPinMissing,
            reason: "owner public-key pin is absent".to_string(),
        },
    }
}

/// Admit a recovered or migrated identity. Never generates a key.
pub fn admit_identity(profile: &LocalDevProductionProfile, class: &IdentityClass) -> Verdict {
    match class {
        IdentityClass::GeneratedFresh { .. } => Verdict::Deny {
            case: DenyCase::FirstLaunchGenerate,
            reason: "local-dev production profile refuses first-launch generate \
                     (no remint; recover the existing owner identity)"
                .to_string(),
        },
        IdentityClass::KeyringLocked => Verdict::Deny {
            case: DenyCase::LockedKeychain,
            reason: "local-dev production profile fails closed when the \
                     production keyring is locked or unreachable"
                .to_string(),
        },
        IdentityClass::ExistingIdentityLost => Verdict::Deny {
            case: DenyCase::ExistingIdentityUnrecoverable,
            reason: "local-dev production profile refuses to replace a lost \
                     owner identity (no remint)"
                .to_string(),
        },
        IdentityClass::RecoveredExisting { pubkey_hex }
        | IdentityClass::MigratedExisting { pubkey_hex } => pubkey_matches_pin(profile, pubkey_hex),
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
                 {PRODUCTION_RELAY_WS_URL} (no fallback to :3000)"
            ),
        };
    }
    match probe {
        RelayProbe::Available => Verdict::Accept {
            reason: format!("relay {PRODUCTION_RELAY_WS_URL} is available"),
        },
        RelayProbe::Unavailable => Verdict::Deny {
            case: DenyCase::RelayUnavailable,
            reason: format!("pinned relay {PRODUCTION_RELAY_WS_URL} is unavailable; fail closed"),
        },
    }
}

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

/// Desktop requires the relay. Transport must not be marked failed solely
/// because Desktop is absent.
pub fn admit_transport(observation: TransportObservation) -> Verdict {
    match observation {
        TransportObservation::Ready => Verdict::Accept {
            reason: "buzz_transport is optional; Desktop absence is not a transport failure"
                .to_string(),
        },
        TransportObservation::Failed { reason } => Verdict::Deny {
            case: DenyCase::TransportDesktopOptional,
            reason: reason.to_string(),
        },
        TransportObservation::FailedBecauseDesktopAbsent => Verdict::Deny {
            case: DenyCase::TransportDesktopOptional,
            reason: "buzz_transport must not be marked failed solely because Desktop is absent"
                .to_string(),
        },
    }
}

pub fn admit_macos_app_artifact(signed_app_sha256: Option<&str>) -> Verdict {
    match signed_app_sha256 {
        Some(digest) => match normalize_digest(digest) {
            Ok(_) => Verdict::Accept {
                reason: "signed macOS .app hash is present".to_string(),
            },
            Err(reason) => Verdict::Deny {
                case: DenyCase::MacAppUnproven,
                reason,
            },
        },
        None => Verdict::Deny {
            case: DenyCase::MacAppUnproven,
            reason: format!(
                "signed macOS .app hash is required to admit a live package; \
                 leftover {MAC_PACKAGED_APP_BUILD_LEFTOVER} is unsatisfied \
                 (a boolean true is not proof)"
            ),
        },
    }
}

/// Rollback must authenticate the target tree digest, not a mutable pointer.
pub fn admit_rollback_target(
    claimed_target: &str,
    recomputed_target_digest: &str,
    current_digest: &str,
) -> Verdict {
    if claimed_target == current_digest {
        return Verdict::Deny {
            case: DenyCase::RollbackUnauthenticated,
            reason: "rollback target must not equal the current digest".to_string(),
        };
    }
    if claimed_target != recomputed_target_digest {
        return Verdict::Deny {
            case: DenyCase::RollbackUnauthenticated,
            reason: "rollback target digest does not match the recomputed tree \
                     (mutable pointer files are not authority)"
                .to_string(),
        };
    }
    Verdict::Accept {
        reason: "rollback target tree digest recomputed and authenticated".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePathKind {
    State,
    Log,
}

fn normalize_for_compare(path: &Path) -> PathBuf {
    path.components().collect()
}

pub fn path_is_inside(checkout: &Path, candidate: &Path) -> bool {
    let checkout = normalize_for_compare(checkout);
    let candidate = normalize_for_compare(candidate);
    candidate == checkout || candidate.starts_with(&checkout)
}

fn path_has_component(path: &Path, name: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str() == name)
}

fn path_is_forbidden_runtime(checkout: &Path, candidate: &Path) -> bool {
    if path_is_inside(checkout, candidate) {
        return true;
    }
    let text = candidate.to_string_lossy().to_ascii_lowercase();
    if text.contains("/reports/ops") || text.ends_with("/reports/ops") {
        return true;
    }
    let has_dawsos =
        path_has_component(candidate, "DawsOS") || path_has_component(candidate, "dawsos");
    let has_reports = path_has_component(candidate, "reports");
    let has_ops = path_has_component(candidate, "ops");
    has_dawsos && (has_reports || has_ops)
}

pub fn admit_runtime_path(checkout: &Path, candidate: &Path, kind: RuntimePathKind) -> Verdict {
    if path_is_forbidden_runtime(checkout, candidate) {
        let case = match kind {
            RuntimePathKind::State => DenyCase::StatePathForbidden,
            RuntimePathKind::Log => DenyCase::LogPathForbidden,
        };
        return Verdict::Deny {
            case,
            reason: format!(
                "{kind:?} path {} is inside the source checkout or a DawsOS reports/ops tree",
                candidate.display()
            ),
        };
    }
    Verdict::Accept {
        reason: format!("{kind:?} path is outside checkout and DawsOS reports/ops"),
    }
}

/// Boot-time admission after identity resolution. Fail closed; never print nsec.
pub fn admit_boot_identity(pubkey_hex: &str) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let profile = active_profile()?;
    admit_owner_pin(&profile).into_result()?;
    admit_identity(
        &profile,
        &IdentityClass::RecoveredExisting {
            pubkey_hex: pubkey_hex.to_string(),
        },
    )
    .into_result()?;
    admit_keyring_service(PRODUCTION_KEYRING_SERVICE).into_result()?;
    admit_bundle_identifier(&profile.bundle_identifier).into_result()?;
    admit_relay(&profile.relay_ws_url, RelayProbe::Available).into_result()
}

pub fn deny_first_launch_generate() -> Result<(), String> {
    if !refuses_first_launch_generate() {
        return Ok(());
    }
    Err(
        "local-dev production profile refuses first-launch generate \
         (no remint; recover the existing owner identity)"
            .to_string(),
    )
}

pub fn deny_locked_or_lost(locked: bool, lost: bool) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    if locked {
        return Err(
            "local-dev production profile fails closed when the production \
             keyring is locked or unreachable"
                .to_string(),
        );
    }
    if lost {
        return Err(
            "local-dev production profile refuses to replace a lost owner \
             identity (no remint)"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn with_test_profile<T>(profile: LocalDevProductionProfile, f: impl FnOnce() -> T) -> T {
    TEST_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = Some(profile);
    });
    let result = f();
    TEST_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = None;
    });
    result
}

#[cfg(test)]
pub(crate) fn fixture_owner_hex() -> String {
    format!("{OWNER_DISPLAY_PREFIX}{}", "ab".repeat(28))
}

#[cfg(test)]
pub(crate) fn fixture_profile(
    owner: Option<String>,
    digest: Option<String>,
) -> LocalDevProductionProfile {
    LocalDevProductionProfile {
        profile: "local-dev-production".to_string(),
        bundle_identifier: BUNDLE_IDENTIFIER.to_string(),
        keyring_service: PRODUCTION_KEYRING_SERVICE.to_string(),
        relay_ws_url: PRODUCTION_RELAY_WS_URL.to_string(),
        owner_display_prefix: OWNER_DISPLAY_PREFIX.to_string(),
        owner_pubkey: owner,
        owner_pubkey_sha256: digest,
        frontend_dist: FRONTEND_DIST.to_string(),
        buzz_transport: "optional".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn other_hex() -> String {
        format!("ffff0000{}", "cd".repeat(28))
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
        assert_eq!(profile.buzz_transport, "optional");
        assert!(profile.owner_pubkey.is_none());
        assert!(profile.owner_pubkey_sha256.is_none());
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
        // Prefix alone is not a boundary.
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
    fn relay_is_3300_and_does_not_fall_back_to_3000() {
        assert!(admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Available).is_accept());
        assert_eq!(
            admit_relay(PRODUCTION_RELAY_WS_URL, RelayProbe::Unavailable).deny_case(),
            Some(DenyCase::RelayUnavailable)
        );
        assert_eq!(
            admit_relay("ws://localhost:3000", RelayProbe::Available).deny_case(),
            Some(DenyCase::RelayMismatch)
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
    fn signed_app_boolean_is_not_proof() {
        assert_eq!(
            admit_macos_app_artifact(None).deny_case(),
            Some(DenyCase::MacAppUnproven)
        );
        assert!(admit_macos_app_artifact(Some(&format!("sha256:{}", "ab".repeat(32)))).is_accept());
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
}
