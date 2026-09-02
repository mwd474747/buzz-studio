// Shared schema, included from the same source the runtime command parses with,
// so the build-time validation below and the runtime parse cannot drift.
include!("src/commands/reconnect_hook_config.rs");

use base64::Engine as _;

const LOCAL_OWNER_PROFILE_PATH: &str = "../../.release/local-owner-profile.json";
const LOCAL_OWNER_RATIFICATION_PATH: &str = "../../.release/local-owner-ratification.json";
const LOCAL_OWNER_SOURCE_RECEIPT_PATH: &str = "generated/local-owner-source-receipt.json";
const RATIFIED_OWNER_PUBKEY: &str =
    "ea840b3e14aceac2b09619de28aedda628e79fcb120dea462ed3ccc512875971";
const RATIFIED_OWNER_DIGEST: &str =
    "sha256:af3cd8c1007e504b9d0385c0090395f2a4fecef56e34fd91e66301093583637e";
const RATIFICATION_RECEIPT_DIGEST: &str =
    "sha256:9ccb24a04428fec6d9638d729bbddf0784c4af0de72c55ef0f3f1c22e9e42517";

#[derive(serde::Deserialize)]
struct LocalOwnerBuildProfile {
    schema_version: u8,
    profile: String,
    bundle_identifier: String,
    keyring_service: String,
    relay_ws_url: String,
    owner_pubkey: String,
    owner_pubkey_sha256: String,
    owner_pin_required: bool,
    macos_signing: LocalOwnerMacosSigning,
}

#[derive(serde::Deserialize)]
struct LocalOwnerMacosSigning {
    required: bool,
    team_id: Option<String>,
    identity: Option<String>,
}

#[derive(serde::Deserialize)]
struct LocalOwnerBuildRatification {
    schema_version: u8,
    authority: String,
    ratified_on: String,
    channel: String,
    owner_pubkey: String,
    owner_pubkey_sha256: String,
    authority_receipt_sha256: String,
}

#[derive(serde::Deserialize)]
struct LocalOwnerSourceReceipt {
    schema_version: u8,
    profile: String,
    source_commit: String,
    source_tree: String,
    profile_sha256: String,
    builder_class: String,
    artifact_stage: String,
    source_dirty: bool,
}

fn validate_local_owner_profile() -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let raw = std::fs::read(LOCAL_OWNER_PROFILE_PATH)
        .map_err(|error| format!("read {LOCAL_OWNER_PROFILE_PATH}: {error}"))?;
    let profile: LocalOwnerBuildProfile = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse {LOCAL_OWNER_PROFILE_PATH}: {error}"))?;
    let ratification_raw = std::fs::read(LOCAL_OWNER_RATIFICATION_PATH)
        .map_err(|error| format!("read {LOCAL_OWNER_RATIFICATION_PATH}: {error}"))?;
    let ratification: LocalOwnerBuildRatification = serde_json::from_slice(&ratification_raw)
        .map_err(|error| format!("parse {LOCAL_OWNER_RATIFICATION_PATH}: {error}"))?;
    if ratification.schema_version != 1
        || ratification.authority != "mike"
        || ratification.ratified_on != "2026-09-01"
        || ratification.channel != "#local-dev"
        || ratification.owner_pubkey != RATIFIED_OWNER_PUBKEY
        || ratification.owner_pubkey_sha256 != RATIFIED_OWNER_DIGEST
        || ratification.authority_receipt_sha256 != RATIFICATION_RECEIPT_DIGEST
    {
        return Err(
            "local-owner ratification does not match Mike's exact authority receipt".into(),
        );
    }

    if profile.schema_version != 1 || profile.profile != "local-owner" {
        return Err("local-owner profile schema/name mismatch".to_string());
    }
    if profile.bundle_identifier != "xyz.block.buzz.app" {
        return Err("local-owner bundle identifier must be xyz.block.buzz.app".to_string());
    }
    if profile.keyring_service != "buzz-desktop" {
        return Err("local-owner keyring service must be buzz-desktop".to_string());
    }
    if profile.relay_ws_url != "ws://localhost:3300" {
        return Err("local-owner relay must be ws://localhost:3300".to_string());
    }
    if !profile.owner_pin_required {
        return Err("local-owner public-key pin must be required".to_string());
    }
    if profile.owner_pubkey.len() != 64
        || profile.owner_pubkey != profile.owner_pubkey.to_ascii_lowercase()
        || !profile
            .owner_pubkey
            .chars()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err("local-owner public key must be 64 lowercase hex characters".to_string());
    }
    let owner_bytes = (0..profile.owner_pubkey.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&profile.owner_pubkey[index..index + 2], 16)
                .map_err(|error| format!("decode local-owner public key: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let owner_digest = format!("sha256:{}", hex::encode(Sha256::digest(owner_bytes)));
    if profile.owner_pubkey_sha256 != owner_digest {
        return Err("local-owner public key does not match its raw-byte digest".to_string());
    }
    if profile.owner_pubkey != ratification.owner_pubkey
        || profile.owner_pubkey_sha256 != ratification.owner_pubkey_sha256
    {
        return Err("local-owner profile does not match the ratified #local-dev owner".to_string());
    }
    if !profile.macos_signing.required {
        return Err("local-owner macOS signing must remain required".to_string());
    }
    match (
        profile.macos_signing.team_id.as_deref(),
        profile.macos_signing.identity.as_deref(),
    ) {
        (None, None) => {}
        (Some(team_id), Some(identity)) => {
            if team_id.len() != 10
                || !team_id
                    .chars()
                    .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit())
            {
                return Err(
                    "local-owner Team ID must be exactly 10 uppercase alphanumeric characters"
                        .to_string(),
                );
            }
            if identity.is_empty() || identity.trim() != identity {
                return Err(
                    "local-owner signing identity must be nonempty without surrounding whitespace"
                        .to_string(),
                );
            }
        }
        _ => {
            return Err(
                "local-owner Team ID and signing identity must be filled together".to_string(),
            );
        }
    }

    Ok(format!("sha256:{}", hex::encode(Sha256::digest(raw))))
}

fn configure_local_owner_profile() {
    println!("cargo:rerun-if-changed={LOCAL_OWNER_PROFILE_PATH}");
    println!("cargo:rerun-if-changed={LOCAL_OWNER_RATIFICATION_PATH}");
    println!("cargo:rerun-if-changed={LOCAL_OWNER_SOURCE_RECEIPT_PATH}");
    println!("cargo:rerun-if-env-changed=BUZZ_DESKTOP_SOURCE_COMMIT");
    println!("cargo:rerun-if-env-changed=BUZZ_DESKTOP_SOURCE_TREE");

    if std::env::var_os("CARGO_FEATURE_LOCAL_OWNER_PROFILE").is_none() {
        return;
    }

    let profile_digest = match validate_local_owner_profile() {
        Ok(value) => value,
        Err(error) => panic!("invalid local-owner profile: {error}"),
    };
    let valid_source_oid = |value: &str| {
        value.len() == 40
            && value
                .chars()
                .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
    };
    let source_commit = std::env::var("BUZZ_DESKTOP_SOURCE_COMMIT")
        .ok()
        .filter(|value| valid_source_oid(value));
    let expected_source_tree = std::env::var("BUZZ_DESKTOP_SOURCE_TREE")
        .ok()
        .filter(|value| valid_source_oid(value));

    if std::env::var("PROFILE").as_deref() == Ok("release")
        && (source_commit.is_none() || expected_source_tree.is_none())
    {
        panic!("local-owner release builds require exact lowercase 40-hex source commit and tree");
    }

    let source_tree = if std::env::var("PROFILE").as_deref() == Ok("release") {
        let receipt_raw = match std::fs::read(LOCAL_OWNER_SOURCE_RECEIPT_PATH) {
            Ok(value) => value,
            Err(error) => panic!(
                "local-owner release builds require {LOCAL_OWNER_SOURCE_RECEIPT_PATH}: {error}"
            ),
        };
        let receipt: LocalOwnerSourceReceipt = match serde_json::from_slice(&receipt_raw) {
            Ok(value) => value,
            Err(error) => panic!("parse local-owner source receipt: {error}"),
        };
        let expected_commit = match source_commit.as_deref() {
            Some(value) => value,
            None => panic!("local-owner source commit disappeared during build validation"),
        };
        let expected_tree = match expected_source_tree.as_deref() {
            Some(value) => value,
            None => panic!("local-owner source tree disappeared during build validation"),
        };
        if receipt.schema_version != 1
            || receipt.profile != "local-owner"
            || receipt.source_commit != expected_commit
            || receipt.source_tree != expected_tree
            || receipt.profile_sha256 != profile_digest
            || receipt.builder_class != "buzz-local-owner-tauri-wrapper"
            || receipt.artifact_stage != "unsigned-before-apple-signing"
            || receipt.source_dirty
            || !valid_source_oid(&receipt.source_commit)
            || !valid_source_oid(&receipt.source_tree)
        {
            panic!("local-owner source receipt does not match the release build inputs");
        }
        Some(receipt.source_tree)
    } else {
        None
    };

    println!("cargo:rustc-env=BUZZ_DESKTOP_LOCAL_OWNER_PROFILE_SHA256={profile_digest}");
    if let Some(commit) = source_commit {
        println!("cargo:rustc-env=BUZZ_DESKTOP_SOURCE_COMMIT={commit}");
    }
    if let Some(tree) = source_tree {
        println!("cargo:rustc-env=BUZZ_DESKTOP_SOURCE_TREE={tree}");
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=BUZZ_RELAY_URL");
    println!("cargo:rerun-if-env-changed=BUZZ_RELAY_HTTP");
    println!("cargo:rerun-if-env-changed=BUZZ_UPDATER_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=BUZZ_UPDATER_ENDPOINT");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_BUZZ_AGENT_PROVIDER");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_BUZZ_AGENT_MODEL");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_AGENT_ENV");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_RELAY_RECONNECT_CMD");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_OBSERVER_ARCHIVE_DEFAULT");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_AGENT_METRIC_ARCHIVE_DEFAULT");
    println!("cargo:rerun-if-env-changed=BUZZ_BUILD_AUTO_CONNECT_DEFAULT_RELAY");
    println!("cargo:rustc-check-cfg=cfg(buzz_updater_enabled)");

    configure_local_owner_profile();

    if let Ok(relay_url) = std::env::var("BUZZ_RELAY_URL") {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_RELAY_URL={relay_url}");
    }

    if let Ok(relay_http) = std::env::var("BUZZ_RELAY_HTTP") {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_RELAY_HTTP={relay_http}");
    }

    if let Ok(provider) = std::env::var("BUZZ_BUILD_BUZZ_AGENT_PROVIDER") {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_BUZZ_AGENT_PROVIDER={provider}");
    }

    if let Ok(model) = std::env::var("BUZZ_BUILD_BUZZ_AGENT_MODEL") {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_BUZZ_AGENT_MODEL={model}");
    }

    // Generic KEY=VALUE pairs to inject into every spawned agent process.
    // Newline-delimited; each line must be non-empty and contain exactly one
    // `=` separator with a non-empty key.  OSS builds leave this unset.
    // The validated value is base64-encoded before emitting so the single-line
    // Cargo build-script output carries all pairs (Cargo output is line-oriented;
    // a raw multiline value would be silently truncated to the first line).
    if let Ok(raw) = std::env::var("BUZZ_BUILD_AGENT_ENV") {
        for (line_no, line) in raw.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let eq = line.find('=').unwrap_or_else(|| {
                panic!(
                    "BUZZ_BUILD_AGENT_ENV line {}: missing '=' separator in {:?}",
                    line_no + 1,
                    line
                )
            });
            let key = &line[..eq];
            if key.is_empty() {
                panic!(
                    "BUZZ_BUILD_AGENT_ENV line {}: key must not be empty in {:?}",
                    line_no + 1,
                    line
                );
            }
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_AGENT_ENV={encoded}");
    }

    if let Ok(val) = std::env::var("BUZZ_BUILD_RELAY_RECONNECT_CMD") {
        let parsed: serde_json::Value = serde_json::from_str(&val)
            .unwrap_or_else(|e| panic!("BUZZ_BUILD_RELAY_RECONNECT_CMD is not valid JSON: {e}"));
        serde_json::from_value::<ReconnectHookConfig>(parsed).unwrap_or_else(|e| {
            panic!("BUZZ_BUILD_RELAY_RECONNECT_CMD doesn't match ReconnectHookConfig: {e}")
        });
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_RELAY_RECONNECT_CMD={val}");
    }

    // Presence-only flag: when set (any non-empty value), observer-feed archive
    // defaults to ON for the current identity on first run.  OSS builds leave
    // this unset → default OFF.  No JSON validation needed — the command only
    // checks `.is_some()`.
    if std::env::var("BUZZ_BUILD_OBSERVER_ARCHIVE_DEFAULT").is_ok() {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_OBSERVER_ARCHIVE_DEFAULT=1");
    }

    // Presence-only flag: when set (any non-empty value), agent-turn-metric
    // archive defaults to ON for the current identity on first run.  OSS builds
    // leave this unset → default OFF.
    if std::env::var("BUZZ_BUILD_AGENT_METRIC_ARCHIVE_DEFAULT").is_ok() {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_AGENT_METRIC_ARCHIVE_DEFAULT=1");
    }

    // Presence-only release capability: internal desktop builds opt into
    // auto-connecting their configured default relay on first run. OSS builds
    // leave this unset and retain explicit community selection.
    if std::env::var("BUZZ_BUILD_AUTO_CONNECT_DEFAULT_RELAY").is_ok() {
        println!("cargo:rustc-env=BUZZ_DESKTOP_BUILD_AUTO_CONNECT_DEFAULT_RELAY=1");
    }

    let updater_public_key = std::env::var("BUZZ_UPDATER_PUBLIC_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let updater_endpoint = std::env::var("BUZZ_UPDATER_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if updater_public_key.is_some() && updater_endpoint.is_some() {
        println!("cargo:rustc-cfg=buzz_updater_enabled");
    }

    // Cargo test executables get no embedded Windows manifest (tauri_build
    // attaches one to bin targets only), so the loader binds comctl32 v5, which
    // lacks TaskDialogIndirect (statically imported via tauri-plugin-dialog/rfd)
    // and debug test exes die at load with STATUS_ENTRYPOINT_NOT_FOUND. Declaring
    // the Common Controls v6 dependency makes link.exe emit a side-by-side
    // <exe>.manifest that the loader honors for manifest-less executables;
    // binaries with an embedded manifest (the real app) ignore it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    tauri_build::try_build(
        tauri_build::Attributes::new().plugin(
            "websocket",
            tauri_build::InlinedPlugin::new()
                .commands(&["connect", "send", "disconnect", "disconnect_all"])
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        ),
    )
    .expect("failed to build Tauri application");
}
