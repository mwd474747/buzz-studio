//! Compile-time profile for Mike's local Buzz cockpit.
//!
//! The profile contains public configuration only. It pins the existing owner
//! public key, production keyring service, bundle identifier, and localhost
//! relay. It never reads, stores, prints, exports, or rotates a private key.

#[cfg(feature = "mesh-llm")]
compile_error!("local-owner-profile cannot be combined with mesh-llm");

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PROFILE_JSON: &str = include_str!("../../../.release/local-owner-profile.json");
const RATIFICATION_JSON: &str = include_str!("../../../.release/local-owner-ratification.json");

pub(crate) const BUNDLE_IDENTIFIER: &str = "xyz.block.buzz.app";
pub(crate) const KEYRING_SERVICE: &str = "buzz-desktop";
pub(crate) const RELAY_WS_URL: &str = "ws://localhost:3300";
const RATIFIED_OWNER_PUBKEY: &str =
    "ea840b3e14aceac2b09619de28aedda628e79fcb120dea462ed3ccc512875971";
const RATIFIED_OWNER_DIGEST: &str =
    "sha256:af3cd8c1007e504b9d0385c0090395f2a4fecef56e34fd91e66301093583637e";
const RATIFICATION_RECEIPT_DIGEST: &str =
    "sha256:9ccb24a04428fec6d9638d729bbddf0784c4af0de72c55ef0f3f1c22e9e42517";

#[derive(Debug, Deserialize)]
struct LocalOwnerRatification {
    schema_version: u8,
    authority: String,
    ratified_on: String,
    channel: String,
    owner_pubkey: String,
    owner_pubkey_sha256: String,
    authority_receipt_sha256: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct LocalOwnerProfile {
    schema_version: u8,
    profile: String,
    pub(crate) bundle_identifier: String,
    pub(crate) keyring_service: String,
    pub(crate) relay_ws_url: String,
    pub(crate) owner_pubkey: String,
    pub(crate) owner_pubkey_sha256: String,
    owner_pin_required: bool,
    pub(crate) macos_signing: MacosSigningPins,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct MacosSigningPins {
    required: bool,
    team_id: Option<String>,
    identity: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct LocalOwnerProfileSummary {
    pub profile: String,
    pub profile_sha256: String,
    pub source_commit: Option<String>,
    pub source_tree: Option<String>,
    pub bundle_identifier: String,
    pub keyring_service: String,
    pub relay_ws_url: String,
    pub owner_pubkey: String,
    pub owner_pubkey_sha256: String,
    pub macos_signing_required: bool,
    pub macos_signing_configured: bool,
}

static PROFILE: OnceLock<Result<LocalOwnerProfile, String>> = OnceLock::new();

/// Keep the local-owner renderer on the application document it was packaged
/// with. Links may still be rendered as content, but the webview itself must
/// never become an external browser or a second privileged application origin.
#[cfg(feature = "local-owner-profile")]
pub(crate) fn packaged_navigation_allowed(url: &tauri::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return false;
    }

    matches!((url.scheme(), url.host_str()), ("tauri", Some("localhost")))
}

#[cfg(test)]
thread_local! {
    static TEST_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this binary was compiled as the local-owner deployment flavor.
pub(crate) const fn compiled_profile_enabled() -> bool {
    cfg!(feature = "local-owner-profile")
}

/// Tests opt in explicitly so enabling the feature for one compile does not
/// silently change every unrelated identity test.
pub(crate) fn profile_active() -> bool {
    #[cfg(test)]
    {
        TEST_ACTIVE.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        compiled_profile_enabled()
    }
}

fn sha256_label(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn normalize_pubkey(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("owner public key must be the complete 64-character hex key".to_string());
    }
    Ok(normalized)
}

fn owner_digest(pubkey: &str) -> Result<String, String> {
    let normalized = normalize_pubkey(pubkey)?;
    let bytes =
        hex::decode(normalized).map_err(|error| format!("decode owner public key: {error}"))?;
    Ok(sha256_label(&bytes))
}

fn validate_ratification() -> Result<LocalOwnerRatification, String> {
    let ratification: LocalOwnerRatification = serde_json::from_str(RATIFICATION_JSON)
        .map_err(|error| format!("parse local-owner ratification: {error}"))?;
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
    Ok(ratification)
}

fn validate_profile(raw: &str, compiled_digest: Option<&str>) -> Result<LocalOwnerProfile, String> {
    let ratification = validate_ratification()?;
    let profile: LocalOwnerProfile =
        serde_json::from_str(raw).map_err(|error| format!("parse local-owner profile: {error}"))?;

    if profile.schema_version != 1 || profile.profile != "local-owner" {
        return Err("local-owner profile schema/name mismatch".to_string());
    }
    if profile.bundle_identifier != BUNDLE_IDENTIFIER {
        return Err(format!(
            "local-owner bundle identifier must be {BUNDLE_IDENTIFIER}"
        ));
    }
    if profile.keyring_service != KEYRING_SERVICE {
        return Err(format!(
            "local-owner keyring service must be {KEYRING_SERVICE}"
        ));
    }
    if profile.relay_ws_url != RELAY_WS_URL {
        return Err(format!("local-owner relay must be {RELAY_WS_URL}"));
    }
    if !profile.owner_pin_required {
        return Err("local-owner public-key pin must remain required".to_string());
    }

    let normalized_owner = normalize_pubkey(&profile.owner_pubkey)?;
    if profile.owner_pubkey != normalized_owner {
        return Err("local-owner public key must be canonical lowercase hex".to_string());
    }
    if owner_digest(&normalized_owner)? != profile.owner_pubkey_sha256 {
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
    if profile.macos_signing.team_id.is_some() != profile.macos_signing.identity.is_some() {
        return Err("local-owner Team ID and signing identity must be filled together".to_string());
    }
    if let (Some(team_id), Some(identity)) = (
        profile.macos_signing.team_id.as_deref(),
        profile.macos_signing.identity.as_deref(),
    ) {
        if team_id.len() != 10
            || !team_id
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
        {
            return Err(
                "local-owner Team ID must be 10 uppercase ASCII letters or digits".to_string(),
            );
        }
        if identity.trim().is_empty() || identity != identity.trim() {
            return Err(
                "local-owner signing identity must be non-empty canonical text".to_string(),
            );
        }
    }
    if let Some(expected) = compiled_digest {
        let actual = sha256_label(raw.as_bytes());
        if actual != expected {
            return Err(
                "compiled local-owner profile digest does not match its source".to_string(),
            );
        }
    }

    Ok(profile)
}

pub(crate) fn profile() -> Result<&'static LocalOwnerProfile, String> {
    let compiled_digest = option_env!("BUZZ_DESKTOP_LOCAL_OWNER_PROFILE_SHA256");
    match PROFILE.get_or_init(|| validate_profile(PROFILE_JSON, compiled_digest)) {
        Ok(profile) => Ok(profile),
        Err(error) => Err(error.clone()),
    }
}

pub(crate) fn owner_matches(profile: &LocalOwnerProfile, pubkey: &str) -> Result<bool, String> {
    Ok(normalize_pubkey(pubkey)? == profile.owner_pubkey)
}

/// Require the exact ratified owner before an identity mutation or signing
/// path proceeds. An eight-character display prefix is never sufficient.
pub(crate) fn require_owner(pubkey: &str) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    if owner_matches(profile()?, pubkey)? {
        Ok(())
    } else {
        Err("local-owner build requires the ratified #local-dev owner identity".to_string())
    }
}

/// Explicit-key relay helpers exist for ordinary managed-agent and deferred
/// flows. In this flavor they may act only as the admitted owner, never as an
/// agent key or while the owner keyring is in recovery.
pub(crate) fn require_explicit_signer(
    state: &crate::app_state::AppState,
    keys: &nostr::Keys,
) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let admitted = state.signing_keys()?;
    if admitted.public_key() != keys.public_key() {
        return Err("local-owner build refuses explicit non-owner relay signing".to_string());
    }
    Ok(())
}

/// Require the pinned localhost relay before workspace state is changed.
pub(crate) fn require_relay(relay_url: &str) -> Result<(), String> {
    if !profile_active() || relay_url == RELAY_WS_URL {
        Ok(())
    } else {
        Err(format!(
            "local-owner build requires relay {RELAY_WS_URL}; got {relay_url:?}"
        ))
    }
}

/// Explicit HTTP helpers must resolve to the same compiled relay. This keeps
/// owner-signed NIP-98 requests from being redirected through a caller-supplied
/// relay URL even when the signer itself is valid.
pub(crate) fn require_relay_http_base(api_base_url: &str) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let expected = crate::relay::relay_http_base_url(RELAY_WS_URL);
    if api_base_url.trim().trim_end_matches('/') == expected {
        Ok(())
    } else {
        Err(format!(
            "local-owner build requires relay HTTP base {expected}; got {api_base_url:?}"
        ))
    }
}

/// Identity generation, recovery-key persistence, and destructive sign-out
/// are unavailable in this flavor. The existing owner may still be imported.
#[cfg(not(feature = "local-owner-profile"))]
pub(crate) fn deny_identity_replacement(operation: &str) -> Result<(), String> {
    if profile_active() {
        Err(format!(
            "local-owner build refuses {operation}; recover the existing #local-dev owner"
        ))
    } else {
        Ok(())
    }
}

/// Pairing and backup/export flows move owner key material out of the
/// production keyring. The pinned local-owner flavor never permits that.
#[cfg(not(feature = "local-owner-profile"))]
pub(crate) fn deny_identity_export(operation: &str) -> Result<(), String> {
    if profile_active() {
        Err(format!(
            "local-owner build refuses {operation}; owner key export and pairing are disabled"
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn recovery_active(state: &crate::app_state::AppState) -> bool {
    profile_active()
        && (state
            .identity_lost
            .load(std::sync::atomic::Ordering::Acquire)
            || state
                .keyring_locked
                .load(std::sync::atomic::Ordering::Acquire)
            || state
                .relaunch_required
                .load(std::sync::atomic::Ordering::Acquire)
            || state
                .reset_failed
                .load(std::sync::atomic::Ordering::Acquire))
}

pub(crate) fn require_identity_recovery_import(
    state: &crate::app_state::AppState,
) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    if state
        .reset_failed
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("identity import is disabled while reset recovery is incomplete".to_string());
    }
    let lost = state
        .identity_lost
        .load(std::sync::atomic::Ordering::Acquire);
    let locked = state
        .keyring_locked
        .load(std::sync::atomic::Ordering::Acquire);
    if !lost && !locked {
        return Err("local-owner identity replacement is disabled".to_string());
    }
    Ok(())
}

/// This deployment is an interaction surface, not an agent fleet manager.
/// Legacy process retirement is a bounded operations action, not a permanent
/// renderer capability; no path may create, start, stop, or delete an agent.
#[cfg(not(feature = "local-owner-profile"))]
pub(crate) fn deny_agent_activation(operation: &str) -> Result<(), String> {
    if profile_active() {
        Err(format!(
            "local-owner build refuses {operation}; request effects through governed DawsOS Operations"
        ))
    } else {
        Ok(())
    }
}

/// Repository and other machine effects belong behind the durable DawsOS
/// Operation boundary. Buzz remains the interaction surface in this flavor.
#[cfg(not(feature = "local-owner-profile"))]
pub(crate) fn deny_direct_effect(operation: &str) -> Result<(), String> {
    if profile_active() {
        Err(format!(
            "local-owner build refuses {operation}; request effects through governed DawsOS Operations"
        ))
    } else {
        Ok(())
    }
}

/// The generic webview signer is intentionally narrow in the local-owner
/// flavor. It may sign human interaction events, never fleet, workflow, or
/// repository-control events.
pub(crate) fn require_interaction_event_kind(kind: u32) -> Result<(), String> {
    if profile_active()
        && !matches!(
            kind,
            0 | 1
                | 3
                | 7
                | 9
                | 1_984
                | 20_001
                | 20_002
                | 24_810
                | 30_030
                | 30_078
                | 30_300
                | 30_315
                | 40_002
                | 40_003
                | 40_007
                | 40_008
                | 40_100
                | 41_010
                | 41_012
                | 42_000
                | 45_001
                | 45_003
        )
    {
        return Err(format!(
            "local-owner build refuses Nostr event kind {kind}; this signer is interaction-only"
        ));
    }
    Ok(())
}

fn is_reserved_agent_control(content: &str) -> bool {
    matches!(content.trim(), "!shutdown" | "!cancel" | "!rotate")
}

fn raw_tags_have_name(tags: &[Vec<String>], name: &str) -> bool {
    tags.iter()
        .any(|tag| tag.first().map(String::as_str) == Some(name))
}

fn is_project_control_tag(parts: &[String]) -> bool {
    parts.first().map(String::as_str) == Some("t")
        && parts.get(1).is_some_and(|label| {
            matches!(
                label.as_str(),
                "review-request" | "approval" | "changes-requested"
            )
        })
}

pub(crate) fn require_webview_interaction_event(
    kind: u32,
    content: &str,
    tags: &[Vec<String>],
) -> Result<(), String> {
    if profile_active() && !matches!(kind, 9 | 20_002 | 30_315) {
        return Err(
            format!("local-owner build refuses Nostr event kind {kind}; the webview signer supports only messages, typing, and user status"),
        );
    }
    if profile_active()
        && kind == 9
        && raw_tags_have_name(tags, "p")
        && is_reserved_agent_control(content)
    {
        return Err("local-owner build refuses legacy agent control messages".to_string());
    }
    Ok(())
}

pub(crate) fn require_webview_signed_event(event: &nostr::Event) -> Result<(), String> {
    let kind = u32::from(event.kind.as_u16());
    if profile_active() && !matches!(kind, 9 | 20_002 | 30_315) {
        return Err(
            format!("local-owner build refuses Nostr event kind {kind}; the webview signer supports only messages, typing, and user status"),
        );
    }
    if profile_active()
        && kind == 9
        && event_has_tag(event, "p", None)
        && is_reserved_agent_control(&event.content)
    {
        return Err("local-owner build refuses legacy agent control messages".to_string());
    }
    Ok(())
}

fn event_has_tag(event: &nostr::Event, name: &str, value: Option<&str>) -> bool {
    event.tags.iter().any(|tag| {
        let parts = tag.as_slice();
        parts.first().map(String::as_str) == Some(name)
            && value.is_none_or(|expected| parts.get(1).map(String::as_str) == Some(expected))
    })
}

/// Second, native publication choke point. Entry-point guards express the
/// semantic policy; this allowlist makes missed or future commands fail closed
/// before rate-limit state or network I/O.
pub(crate) fn require_native_interaction_event(event: &nostr::Event) -> Result<(), String> {
    if !profile_active() {
        return Ok(());
    }
    let kind = u32::from(event.kind.as_u16());
    if kind == 9 && event_has_tag(event, "p", None) && is_reserved_agent_control(&event.content) {
        return Err("local-owner build refuses legacy agent control messages".to_string());
    }
    let allowed = match kind {
        1 => !event
            .tags
            .iter()
            .any(|tag| is_project_control_tag(tag.as_slice())),
        5 => !event_has_tag(event, "a", None),
        _ => require_interaction_event_kind(kind).is_ok(),
    };
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "local-owner build refuses Nostr event kind {kind}; native publication is interaction-only"
        ))
    }
}

pub(crate) fn summary() -> Result<Option<LocalOwnerProfileSummary>, String> {
    if !profile_active() {
        return Ok(None);
    }
    let profile = profile()?;
    Ok(Some(LocalOwnerProfileSummary {
        profile: profile.profile.clone(),
        profile_sha256: sha256_label(PROFILE_JSON.as_bytes()),
        source_commit: option_env!("BUZZ_DESKTOP_SOURCE_COMMIT").map(str::to_string),
        source_tree: option_env!("BUZZ_DESKTOP_SOURCE_TREE").map(str::to_string),
        bundle_identifier: profile.bundle_identifier.clone(),
        keyring_service: profile.keyring_service.clone(),
        relay_ws_url: profile.relay_ws_url.clone(),
        owner_pubkey: profile.owner_pubkey.clone(),
        owner_pubkey_sha256: profile.owner_pubkey_sha256.clone(),
        macos_signing_required: profile.macos_signing.required,
        macos_signing_configured: profile.macos_signing.team_id.is_some()
            && profile.macos_signing.identity.is_some(),
    }))
}

#[cfg(feature = "local-owner-profile")]
pub(crate) fn log_boot_posture(state: &crate::app_state::AppState) {
    if !profile_active() {
        return;
    }
    let identity_lost = state
        .identity_lost
        .load(std::sync::atomic::Ordering::Acquire);
    let keyring_locked = state
        .keyring_locked
        .load(std::sync::atomic::Ordering::Acquire);
    let recovery = if identity_lost {
        "lost"
    } else if keyring_locked {
        "keyring-locked"
    } else {
        "ready"
    };
    match summary() {
        Ok(Some(profile)) => eprintln!(
            "buzz-desktop: local-owner profile={} profile_digest={} source_commit={} \
             source_tree={} \
             owner_digest={} relay={} identity_storage={} recovery={} signing_configured={}",
            profile.profile,
            profile.profile_sha256,
            profile.source_commit.as_deref().unwrap_or("unrecorded"),
            profile.source_tree.as_deref().unwrap_or("unrecorded"),
            profile.owner_pubkey_sha256,
            profile.relay_ws_url,
            state.identity_storage().as_str(),
            recovery,
            profile.macos_signing_configured,
        ),
        Ok(None) => {}
        Err(error) => eprintln!("buzz-desktop: local-owner profile summary failed: {error}"),
    }
}

#[cfg(test)]
pub(crate) fn with_test_profile_active<T>(operation: impl FnOnce() -> T) -> T {
    TEST_ACTIVE.with(|active| active.set(true));
    let result = operation();
    TEST_ACTIVE.with(|active| active.set(false));
    result
}

#[cfg(test)]
pub(crate) fn test_profile_for_owner(pubkey: &str) -> LocalOwnerProfile {
    LocalOwnerProfile {
        schema_version: 1,
        profile: "local-owner".to_string(),
        bundle_identifier: BUNDLE_IDENTIFIER.to_string(),
        keyring_service: KEYRING_SERVICE.to_string(),
        relay_ws_url: RELAY_WS_URL.to_string(),
        owner_pubkey: normalize_pubkey(pubkey).unwrap(),
        owner_pubkey_sha256: owner_digest(pubkey).unwrap(),
        owner_pin_required: true,
        macos_signing: MacosSigningPins {
            required: true,
            team_id: None,
            identity: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "ea840b3e14aceac2b09619de28aedda628e79fcb120dea462ed3ccc512875971";
    const OWNER_DIGEST: &str =
        "sha256:af3cd8c1007e504b9d0385c0090395f2a4fecef56e34fd91e66301093583637e";

    #[cfg(feature = "local-owner-profile")]
    fn navigation_allowed(raw: &str) -> bool {
        tauri::Url::parse(raw).is_ok_and(|url| packaged_navigation_allowed(&url))
    }

    #[cfg(feature = "local-owner-profile")]
    #[test]
    fn packaged_navigation_accepts_only_the_application_origins() {
        assert!(navigation_allowed("tauri://localhost/"));
        assert!(navigation_allowed(
            "tauri://localhost/inbox?channel=one#latest"
        ));

        for denied in [
            "http://tauri.localhost/",
            "https://tauri.localhost/settings",
            "https://example.com/",
            "http://tauri.localhost.example.com/",
            "http://tauri.localhost@evil.example/",
            "http://evil.example@tauri.localhost/",
            "http://tauri.localhost:3300/",
            "tauri://localhost.evil.example/",
            "tauri://user@localhost/",
            "tauri://localhost:3300/",
            "file:///etc/passwd",
            "data:text/html,<h1>external</h1>",
            "javascript:alert(1)",
            "about:blank",
            "buzz://community/example",
        ] {
            assert!(!navigation_allowed(denied), "accepted {denied}");
        }
    }

    #[test]
    fn compile_state_helper_matches_the_cargo_feature() {
        assert_eq!(
            compiled_profile_enabled(),
            cfg!(feature = "local-owner-profile")
        );
    }

    #[test]
    fn checked_in_profile_has_exact_ratified_owner() {
        let parsed = validate_profile(PROFILE_JSON, None).unwrap();
        assert_eq!(parsed.owner_pubkey, OWNER);
        assert_eq!(parsed.owner_pubkey_sha256, OWNER_DIGEST);
        assert!(owner_matches(&parsed, OWNER).unwrap());
        assert!(!owner_matches(&parsed, &"a".repeat(64)).unwrap());
    }

    #[test]
    fn self_consistent_different_owner_is_not_ratified() {
        let wrong_owner = "aa".repeat(32);
        let mut profile: serde_json::Value = serde_json::from_str(PROFILE_JSON).unwrap();
        profile["owner_pubkey"] = wrong_owner.clone().into();
        profile["owner_pubkey_sha256"] = owner_digest(&wrong_owner).unwrap().into();

        let error = validate_profile(&profile.to_string(), None).unwrap_err();
        assert!(error.contains("ratified #local-dev owner"));
    }

    #[test]
    fn signing_hold_is_explicit_and_paired() {
        let parsed = validate_profile(PROFILE_JSON, None).unwrap();
        assert!(parsed.macos_signing.required);
        assert!(parsed.macos_signing.team_id.is_none());
        assert!(parsed.macos_signing.identity.is_none());
    }

    #[test]
    fn runtime_environment_cannot_activate_profile() {
        std::env::set_var("BUZZ_DESKTOP_LOCAL_OWNER_PROFILE", "1");
        assert!(!profile_active());
        std::env::remove_var("BUZZ_DESKTOP_LOCAL_OWNER_PROFILE");
    }

    #[test]
    fn active_policy_rejects_wrong_owner_relay_and_replacement() {
        with_test_profile_active(|| {
            assert!(require_owner(OWNER).is_ok());
            assert!(require_owner(&"a".repeat(64)).is_err());
            assert!(require_relay(RELAY_WS_URL).is_ok());
            assert!(require_relay("ws://localhost:3000").is_err());
            assert!(require_relay_http_base("http://localhost:3300/").is_ok());
            assert!(require_relay_http_base("http://localhost:3000").is_err());
            assert!(require_interaction_event_kind(9).is_ok());
            assert!(require_interaction_event_kind(30_315).is_ok());
            assert!(require_interaction_event_kind(45_001).is_ok());
            assert!(require_interaction_event_kind(5).is_err());
            assert!(require_interaction_event_kind(30_620).is_err());
            assert!(require_webview_interaction_event(
                1,
                "project review",
                &[vec!["t".into(), "review-request".into()]],
            )
            .is_err());
            assert!(require_webview_interaction_event(
                9,
                "hello",
                &[vec!["h".into(), "channel".into()]],
            )
            .is_ok());
            assert!(require_webview_interaction_event(
                9,
                "!shutdown",
                &[vec!["p".into(), "agent".into()]],
            )
            .is_err());
        });
    }

    fn signed_event(kind: u16, tags: Vec<Vec<&str>>) -> nostr::Event {
        signed_event_with_content(kind, "test", tags)
    }

    fn signed_event_with_content(kind: u16, content: &str, tags: Vec<Vec<&str>>) -> nostr::Event {
        let tags = tags
            .into_iter()
            .map(|parts| nostr::Tag::parse(parts).unwrap())
            .collect::<Vec<_>>();
        nostr::EventBuilder::new(nostr::Kind::Custom(kind), content)
            .tags(tags)
            .sign_with_keys(&nostr::Keys::generate())
            .unwrap()
    }

    #[test]
    fn native_publication_allows_human_interactions_and_denies_control_events() {
        with_test_profile_active(|| {
            assert!(require_native_interaction_event(&signed_event(9, vec![])).is_ok());
            assert!(require_native_interaction_event(&signed_event(7, vec![])).is_ok());
            assert!(require_native_interaction_event(&signed_event(45_003, vec![])).is_ok());
            assert!(require_native_interaction_event(&signed_event(
                1,
                vec![vec!["t", "approval"]],
            ))
            .is_err());
            assert!(require_native_interaction_event(&signed_event(30_620, vec![])).is_err());
            assert!(require_native_interaction_event(&signed_event(9_035, vec![])).is_err());

            let ephemeral = signed_event(
                9_007,
                vec![
                    vec!["h", "d68df268-e2a7-4576-8f41-9137bb2436ce"],
                    vec!["visibility", "private"],
                    vec!["channel_type", "stream"],
                    vec!["ttl", "3600"],
                ],
            );
            assert!(require_native_interaction_event(&ephemeral).is_err());

            let leave = signed_event_with_content(
                9_022,
                "",
                vec![vec!["h", "d68df268-e2a7-4576-8f41-9137bb2436ce"]],
            );
            assert!(require_native_interaction_event(&leave).is_err());
            assert!(require_webview_signed_event(&leave).is_err());

            let legacy_control =
                signed_event_with_content(9, "!rotate", vec![vec!["p", &"a".repeat(64)]]);
            assert!(require_native_interaction_event(&legacy_control).is_err());
        });
    }

    #[test]
    fn workflow_coordinate_deletion_is_not_an_interaction() {
        with_test_profile_active(|| {
            let workflow_delete = signed_event(5, vec![vec!["a", "30620:owner:workflow"]]);
            assert!(require_native_interaction_event(&workflow_delete).is_err());
            assert!(require_webview_signed_event(&workflow_delete).is_err());
            let message_delete = signed_event(5, vec![vec!["e", &"a".repeat(64)]]);
            assert!(require_native_interaction_event(&message_delete).is_ok());
        });
    }
}
