use std::io::Write;

use nostr::{Keys, ToBech32};

use crate::app_state::AppState;

/// Durable location of the active human identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum IdentityStorage {
    Ephemeral = 0,
    SystemKeyring = 1,
    LocalFile = 2,
    Environment = 3,
}

impl IdentityStorage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ephemeral => "ephemeral",
            Self::SystemKeyring => "system-keyring",
            Self::LocalFile => "local-file",
            Self::Environment => "environment",
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::SystemKeyring,
            2 => Self::LocalFile,
            3 => Self::Environment,
            _ => Self::Ephemeral,
        }
    }
}

impl AppState {
    pub(crate) fn identity_storage(&self) -> IdentityStorage {
        IdentityStorage::from_u8(
            self.identity_storage
                .load(std::sync::atomic::Ordering::Acquire),
        )
    }

    pub(crate) fn set_identity_storage(&self, storage: IdentityStorage) {
        self.identity_storage
            .store(storage as u8, std::sync::atomic::Ordering::Release);
    }
}

/// Recovery state produced by identity resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryState {
    None,
    Lost,
    KeyringLocked,
}

/// Identity and persistence metadata produced by startup resolution.
pub(crate) struct ResolvedIdentity {
    pub(crate) keys: Keys,
    pub(crate) recovery: RecoveryState,
    pub(crate) storage: IdentityStorage,
}

/// Atomically write the key to disk. Uses `atomic-write-file` which:
/// 1. Writes to a temp file in the same directory
/// 2. Calls fsync on the file
/// 3. Renames temp → target (atomic on POSIX, best-effort on Windows)
/// 4. Calls fsync on the parent directory
///
/// On Unix, the file is created with mode 0600 (owner read/write only).
/// On Windows, default ACLs apply — the app data directory is already
/// per-user, so the key is not world-readable in practice.
pub(crate) fn save_key_file(path: &std::path::Path, keys: &Keys) -> Result<(), String> {
    use atomic_write_file::AtomicWriteFile;

    let nsec = keys
        .secret_key()
        .to_bech32()
        .map_err(|e| format!("encode nsec: {e}"))?;

    let mut file = AtomicWriteFile::open(path)
        .map_err(|e| format!("open identity.key for atomic write: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("set identity.key permissions: {e}"))?;
    }

    file.write_all(nsec.as_bytes())
        .map_err(|e| format!("write identity.key: {e}"))?;
    file.commit()
        .map_err(|e| format!("commit identity.key: {e}"))
}
