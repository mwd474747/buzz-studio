use crate::app_state::AppState;

const DEFAULT_RELAY_WS_URL: &str = "ws://localhost:3000";

fn configured_env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn relay_ws_url() -> String {
    if crate::local_owner_profile::profile_active() {
        return crate::local_owner_profile::RELAY_WS_URL.to_string();
    }
    configured_env_var("BUZZ_RELAY_URL")
        .or_else(|| option_env!("BUZZ_DESKTOP_BUILD_RELAY_URL").map(str::to_string))
        .unwrap_or_else(|| DEFAULT_RELAY_WS_URL.to_string())
}

fn workspace_relay_override(state: &AppState) -> Option<String> {
    state
        .relay_url_override
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

pub fn relay_ws_url_with_override(state: &AppState) -> String {
    if crate::local_owner_profile::profile_active() {
        return crate::local_owner_profile::RELAY_WS_URL.to_string();
    }
    workspace_relay_override(state).unwrap_or_else(relay_ws_url)
}

pub fn relay_api_base_url_with_override(state: &AppState) -> String {
    if crate::local_owner_profile::profile_active() {
        return relay_http_base_url(crate::local_owner_profile::RELAY_WS_URL);
    }
    workspace_relay_override(state)
        .map(|url| relay_http_base_url(&url))
        .unwrap_or_else(relay_api_base_url)
}

pub fn relay_http_base_url(relay_url: &str) -> String {
    let trimmed = relay_url.trim().trim_end_matches('/');
    if let Some(suffix) = trimmed.strip_prefix("wss://") {
        return format!("https://{suffix}");
    }
    if let Some(suffix) = trimmed.strip_prefix("ws://") {
        return format!("http://{suffix}");
    }
    trimmed.to_string()
}

pub fn relay_api_base_url() -> String {
    if let Some(base) = configured_env_var("BUZZ_RELAY_HTTP") {
        return base.trim_end_matches('/').to_string();
    }
    if let Some(base) = option_env!("BUZZ_DESKTOP_BUILD_RELAY_HTTP") {
        return base.trim().trim_end_matches('/').to_string();
    }
    relay_http_base_url(&relay_ws_url())
}
