//! Contract-only admission helpers. Compiled for tests, not the production boot
//! path. A boolean or caller-supplied string is not macOS signing proof.

use super::*;
use std::path::{Path, PathBuf};

pub const FRONTEND_DIST: &str = "../dist";
pub const MAC_PACKAGED_APP_BUILD_LEFTOVER: &str = "mac-packaged-app-build";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayProbe {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportObservation {
    Ready,
    Failed { reason: &'static str },
    FailedBecauseDesktopAbsent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePathKind {
    State,
    Log,
}

pub fn admit_relay(configured: &str, probe: RelayProbe) -> Verdict {
    let configuration = admit_relay_configuration(configured);
    if !configuration.is_accept() {
        return configuration;
    }
    match probe {
        RelayProbe::Available => Verdict::Accept {
            reason: format!("relay {PRODUCTION_RELAY_WS_URL} probe reported available"),
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

pub fn admit_transport(observation: TransportObservation) -> Verdict {
    match observation {
        TransportObservation::Ready => Verdict::Accept {
            reason: "Desktop is optional to buzz_transport; transport is required by Desktop"
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

/// Independent leftover check. Caller-supplied identity/team/notarization
/// strings are ignored. A real `.app` plus host codesign tools are required.
pub fn admit_macos_app_artifact(app_path: Option<&Path>) -> Verdict {
    match independent_macos_app_evidence(app_path) {
        Ok(reason) => Verdict::Accept { reason },
        Err(reason) => Verdict::Deny {
            case: DenyCase::MacAppUnproven,
            reason,
        },
    }
}

fn independent_macos_app_evidence(app_path: Option<&Path>) -> Result<String, String> {
    let profile = load_in_tree_profile()?;
    if profile.macos_signing_pin_required
        && (profile.approved_team_id.is_none() || profile.approved_codesign_identity.is_none())
    {
        return Err(
            "approved_team_id / approved_codesign_identity compiled pins are empty; \
             leftover approved-macos-signing-pin stays needed (do not invent a Team ID; \
             any Apple-notarized app is not admitted)"
                .to_string(),
        );
    }
    let path = app_path.ok_or_else(|| {
        format!(
            "signed macOS .app requires a real bundle path and independent \
             codesign/Team ID/Gatekeeper/stapler checks; leftover \
             {MAC_PACKAGED_APP_BUILD_LEFTOVER} stays needed (a boolean or \
             caller string is not proof)"
        )
    })?;
    if !path.exists() {
        return Err(format!("{} does not exist; fail closed", path.display()));
    }
    let is_app = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".app"));
    if !is_app {
        return Err("path is not a .app bundle; fail closed".to_string());
    }
    if cfg!(target_os = "macos") {
        return Err(
            "macos independent verification is not stubbed in-process; fail closed".to_string(),
        );
    }
    Err(format!(
        "this host cannot run codesign, spctl, or stapler; leftover \
         {MAC_PACKAGED_APP_BUILD_LEFTOVER} stays needed"
    ))
}

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
