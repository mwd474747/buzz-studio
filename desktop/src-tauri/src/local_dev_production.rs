//! Local-dev production profile for the existing Desktop release lane.
//!
//! Pins and fail-closed admission live here so the Tauri boot path
//! (`app_state`, `relay`, `keyring`) depends on them. This is not a sidecar
//! crate. It does not mint, import, export, print, or rotate keys. It never
//! prints `nsec`.
//!
//! The in-tree profile (`.release/local-dev-production.json`) carries the
//! ratified `#local-dev` owner public-key pin. Display prefix `ea840b3e` is
//! for display only. Env vars are not a Finder-launched pin. Approved macOS
//! Team ID / identity pins stay empty until Mike/Codex fill them; empty
//! signing pins fail closed. This module does not invent a Team ID.

use sha2::{Digest, Sha256};

/// Same file `scripts/desktop_release.py` owns under `.release/`.
const PROFILE_JSON: &str = include_str!("../../../.release/local-dev-production.json");

pub const BUNDLE_IDENTIFIER: &str = "xyz.block.buzz.app";
pub const PRODUCTION_KEYRING_SERVICE: &str = "buzz-desktop";
pub const DEBUG_KEYRING_SERVICE: &str = "buzz-desktop-dev";
pub const PRODUCTION_RELAY_WS_URL: &str = "ws://localhost:3300";
pub const OWNER_DISPLAY_PREFIX: &str = "ea840b3e";
pub const BUZZ_TRANSPORT: &str = "optional-to-transport";

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
    pub owner_pin_required: bool,
    pub approved_team_id: Option<String>,
    pub approved_codesign_identity: Option<String>,
    pub macos_signing_pin_required: bool,
    pub frontend_dist: String,
    pub buzz_transport: String,
    pub desktop_requires_relay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyCase {
    FirstLaunchGenerate,
    WrongIdentity,
    LockedKeychain,
    ExistingIdentityUnrecoverable,
    OwnerPinMissing,
    RelayMismatch,
    KeyringServiceMismatch,
    BundleIdentifierMismatch,
    #[cfg(test)]
    RelayUnavailable,
    #[cfg(test)]
    StatePathForbidden,
    #[cfg(test)]
    LogPathForbidden,
    #[cfg(test)]
    FrontendDevUrl,
    #[cfg(test)]
    RollbackUnauthenticated,
    #[cfg(test)]
    MacAppUnproven,
    #[cfg(test)]
    TransportDesktopOptional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityClass {
    RecoveredExisting {
        pubkey_hex: String,
    },
    #[allow(dead_code)]
    MigratedExisting {
        pubkey_hex: String,
    },
    GeneratedFresh {
        pubkey_hex: String,
    },
    ExistingIdentityLost,
    KeyringLocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept { reason: String },
    Deny { case: DenyCase, reason: String },
}

impl Verdict {
    #[cfg(test)]
    pub fn is_accept(&self) -> bool {
        matches!(self, Self::Accept { .. })
    }

    #[cfg(test)]
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

/// Compiled-in profile only. Env vars are not a Finder-launched pin.
pub fn active_profile() -> Result<LocalDevProductionProfile, String> {
    #[cfg(test)]
    if let Some(profile) = TEST_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Ok(profile);
    }
    load_in_tree_profile()
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
    let get_bool = |key: &str| -> Result<bool, String> {
        value
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| format!("local-dev production profile missing boolean {key}"))
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
        owner_pin_required: get_bool("owner_pin_required")?,
        approved_team_id: optional_pin("approved_team_id")?,
        approved_codesign_identity: optional_pin("approved_codesign_identity")?,
        macos_signing_pin_required: get_bool("macos_signing_pin_required")?,
        frontend_dist: get_str("frontend_dist")?,
        buzz_transport: get_str("buzz_transport")?,
        desktop_requires_relay: get_bool("desktop_requires_relay")?,
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
    let digest = Sha256::digest(&bytes);
    Ok(format!("sha256:{}", hex::encode(digest)))
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
    if profile.owner_pin_required && pubkey.is_none() && digest.is_none() {
        return Err(
            "local-dev production profile pin is required and is absent \
             (do not invent a key; do not treat an empty JSON pin as pinned)"
                .to_string(),
        );
    }
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

/// Configuration admission for the pinned relay. This is not a health probe.
pub fn admit_relay_configuration(configured: &str) -> Verdict {
    if configured == PRODUCTION_RELAY_WS_URL {
        Verdict::Accept {
            reason: format!(
                "relay configuration admits pinned {PRODUCTION_RELAY_WS_URL} \
                 (not a health check)"
            ),
        }
    } else {
        Verdict::Deny {
            case: DenyCase::RelayMismatch,
            reason: format!(
                "relay {configured:?} is not the pinned production relay \
                 {PRODUCTION_RELAY_WS_URL} (no fallback to :3000)"
            ),
        }
    }
}

/// Boot-time admission after identity resolution. Fail closed; never print nsec.
/// Relay check is configuration admission, not a health probe.
pub fn admit_boot_identity(pubkey_hex: &str) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let profile = active_profile()?;
    if !profile.desktop_requires_relay || profile.buzz_transport != BUZZ_TRANSPORT {
        return Err(
            "local-dev production profile must keep Desktop optional to \
             buzz_transport and require the relay"
                .to_string(),
        );
    }
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
    admit_relay_configuration(&profile.relay_ws_url).into_result()
}

/// Workspace apply must not accept an arbitrary relay or nsec.
pub fn admit_workspace_apply(
    relay_url: &str,
    imported_pubkey_hex: Option<&str>,
) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let profile = active_profile()?;
    admit_owner_pin(&profile).into_result()?;
    admit_relay_configuration(relay_url).into_result()?;
    if let Some(hex) = imported_pubkey_hex {
        pubkey_matches_pin(&profile, hex).into_result()?;
    }
    Ok(())
}

/// Import must match the owner pin before any persist.
pub fn admit_imported_identity(pubkey_hex: &str) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let profile = active_profile()?;
    admit_owner_pin(&profile).into_result()?;
    pubkey_matches_pin(&profile, pubkey_hex).into_result()
}

pub fn deny_first_launch_generate() -> Result<(), String> {
    if !refuses_first_launch_generate() {
        return Ok(());
    }
    admit_identity(
        &active_profile()?,
        &IdentityClass::GeneratedFresh {
            pubkey_hex: String::new(),
        },
    )
    .into_result()
}

pub fn deny_locked_or_lost(locked: bool, lost: bool) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let profile = active_profile()?;
    if locked {
        return admit_identity(&profile, &IdentityClass::KeyringLocked).into_result();
    }
    if lost {
        return admit_identity(&profile, &IdentityClass::ExistingIdentityLost).into_result();
    }
    Ok(())
}

/// Production reset must not erase the owner and then refuse remint.
pub fn deny_identity_erasing_reset() -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    Err(
        "local-dev production profile refuses reset that would erase the \
         owner identity (no remint)"
            .to_string(),
    )
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

/// Labeled test fixture — not the live `#local-dev` owner and not minted.
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
        owner_pin_required: true,
        approved_team_id: None,
        approved_codesign_identity: None,
        macos_signing_pin_required: true,
        frontend_dist: "../dist".to_string(),
        buzz_transport: BUZZ_TRANSPORT.to_string(),
        desktop_requires_relay: true,
    }
}

#[cfg(test)]
#[path = "local_dev_production_contract.rs"]
mod contract;
#[cfg(test)]
pub use contract::*;

#[cfg(test)]
#[path = "local_dev_production_tests.rs"]
mod tests;
