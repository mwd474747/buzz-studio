//! Local-owner identity admission and keyring-only persistence.
//!
//! This module is deliberately separate from the ordinary desktop identity
//! policy. The compiled flavor admits only its exact owner from the production
//! OS keyring and never makes an environment or plaintext fallback signable.

use super::*;

pub(super) fn build_http_client(builder: reqwest::ClientBuilder) -> reqwest::Client {
    if crate::local_owner_profile::profile_active() {
        builder
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("local-owner HTTP client must fail closed without redirect following")
    } else {
        builder.build().unwrap_or_else(|_| reqwest::Client::new())
    }
}

pub(super) fn signing_keys(state: &AppState) -> Result<Keys, String> {
    if state
        .relaunch_required
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("identity recovery was completed; relaunch Buzz before signing".to_string());
    }
    if state
        .identity_lost
        .load(std::sync::atomic::Ordering::Acquire)
        || state
            .keyring_locked
            .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("identity is in recovery mode; event signing is disabled until the identity is restored and Buzz is relaunched".to_string());
    }
    let keys = state
        .keys
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    crate::local_owner_profile::require_owner(&keys.public_key().to_hex())?;
    Ok(keys)
}

pub(super) fn configured_identity_from_env() -> Option<Keys> {
    if crate::local_owner_profile::profile_active() {
        if std::env::var_os("BUZZ_PRIVATE_KEY").is_some() {
            eprintln!(
                "buzz-desktop: ignoring BUZZ_PRIVATE_KEY in the compiled local-owner profile"
            );
        }
        None
    } else {
        identity_from_env()
    }
}

pub(super) fn env_identity_supersedes_persisted() -> bool {
    if crate::local_owner_profile::profile_active() {
        if std::env::var_os("BUZZ_PRIVATE_KEY").is_some() {
            eprintln!(
                "buzz-desktop: ignoring BUZZ_PRIVATE_KEY in the compiled local-owner profile"
            );
        }
        false
    } else {
        identity_from_env().is_some()
    }
}

pub(super) fn resolve_if_active(
    legacy_path: &std::path::Path,
    data_dir: &std::path::Path,
) -> Result<Option<ResolvedIdentity>, String> {
    if !crate::local_owner_profile::profile_active() {
        return Ok(None);
    }
    let profile = crate::local_owner_profile::profile()?;
    if !cfg!(feature = "system-keyring") {
        return resolve_without_keyring(profile, legacy_path).map(Some);
    }
    let store = crate::secret_store::SecretStore::shared(keyring_service());
    resolve_local_owner_with_store(store, profile, legacy_path, data_dir).map(Some)
}

fn recovery(keys: Option<Keys>, recovery: RecoveryState) -> ResolvedIdentity {
    ResolvedIdentity {
        // The recovery UI currently requires an in-memory Keys value. This
        // placeholder is never persisted and AppState::signing_keys rejects it.
        keys: keys.unwrap_or_else(Keys::generate),
        recovery,
        storage: IdentityStorage::Ephemeral,
    }
}

fn legacy_identity_exists(legacy_path: &std::path::Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(legacy_path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "inspect legacy plaintext identity path before keyring admission: {error}"
        )),
    }
}

fn resolve_without_keyring(
    _profile: &crate::local_owner_profile::LocalOwnerProfile,
    _legacy_path: &std::path::Path,
) -> Result<ResolvedIdentity, String> {
    // Defense in depth for downstream feature combinations. The Cargo feature
    // requires system-keyring, but plaintext still cannot become signable.
    Ok(recovery(None, RecoveryState::KeyringLocked))
}

pub(super) fn resolve_local_owner_with_store(
    store: &impl IdentityKeyStore,
    profile: &crate::local_owner_profile::LocalOwnerProfile,
    legacy_path: &std::path::Path,
    _data_dir: &std::path::Path,
) -> Result<ResolvedIdentity, String> {
    let stored = match store.load_all_readonly() {
        Ok(value) => value.unwrap_or_default(),
        Err(error) => {
            eprintln!(
                "buzz-desktop: production keyring blob cannot be read ({error}); entering locked recovery"
            );
            return Ok(recovery(None, RecoveryState::KeyringLocked));
        }
    };
    if stored.keys().any(|name| name != IDENTITY_KEY_NAME) {
        eprintln!(
            "buzz-desktop: production keyring contains legacy non-owner entries; entering recovery without migration"
        );
        return Ok(recovery(None, RecoveryState::Lost));
    }
    let Some(raw) = stored.get(IDENTITY_KEY_NAME) else {
        return Ok(recovery(None, RecoveryState::Lost));
    };
    let Ok(keys) = Keys::parse(raw.trim()) else {
        return Ok(recovery(None, RecoveryState::Lost));
    };
    if !crate::local_owner_profile::owner_matches(profile, &keys.public_key().to_hex())? {
        return Ok(recovery(Some(keys), RecoveryState::Lost));
    }
    if legacy_identity_exists(legacy_path)? {
        eprintln!(
            "buzz-desktop: admitted keyring owner remains in recovery while a legacy plaintext identity path exists"
        );
        return Ok(recovery(Some(keys), RecoveryState::Lost));
    }
    Ok(ResolvedIdentity {
        keys,
        recovery: RecoveryState::None,
        storage: IdentityStorage::SystemKeyring,
    })
}

/// Persist an admitted owner only after a verified production-keyring round
/// trip. Normal app code never reads, migrates, or removes plaintext key files.
pub(super) fn persist_local_owner_keyring_only(
    store: &impl IdentityKeyStore,
    keys: &Keys,
    legacy_path: &std::path::Path,
    _data_dir: &std::path::Path,
) -> Result<IdentityStorage, String> {
    if legacy_identity_exists(legacy_path)? {
        return Err(
            "legacy plaintext identity path must be retired outside normal app boot before owner import"
                .to_string(),
        );
    }
    if store
        .load_all_readonly()?
        .unwrap_or_default()
        .keys()
        .any(|name| name != IDENTITY_KEY_NAME)
    {
        return Err(
            "production keyring contains legacy non-owner entries; retire them before owner import"
                .to_string(),
        );
    }
    let nsec = keys
        .secret_key()
        .to_bech32()
        .map_err(|e| format!("encode nsec: {e}"))?;
    store.store(IDENTITY_KEY_NAME, &nsec)?;
    match store.verify_stored(IDENTITY_KEY_NAME, &nsec) {
        Ok(true) => {}
        Ok(false) => return Err("production keyring read-back verify failed".to_string()),
        Err(error) => {
            return Err(format!(
                "production keyring read-back verify failed: {error}"
            ))
        }
    }
    Ok(IdentityStorage::SystemKeyring)
}

pub(crate) fn persist_local_owner_import(
    store: &crate::secret_store::SecretStore,
    keys: &Keys,
    legacy_path: &std::path::Path,
    data_dir: &std::path::Path,
) -> Result<IdentityStorage, String> {
    persist_local_owner_keyring_only(store, keys, legacy_path, data_dir)
}
