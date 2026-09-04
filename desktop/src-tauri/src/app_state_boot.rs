use super::*;

/// Build the no-redirect client used for authenticated relay media fetches.
pub fn build_media_fetch_client() -> reqwest::Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .resolve("localhost", std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
        .pool_idle_timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(1)
        .redirect(reqwest::redirect::Policy::none());
    let builder = if crate::local_owner_profile::profile_active() {
        builder.no_proxy()
    } else {
        builder
    };
    builder.build()
}

pub fn build_app_state() -> AppState {
    let configured_keys = local_owner_identity::configured_identity_from_env();
    let (keys, identity_storage) = match configured_keys {
        Some(keys) => {
            eprintln!(
                "buzz-desktop: configured identity pubkey {}",
                keys.public_key().to_hex()
            );
            (keys, IdentityStorage::Environment)
        }
        None => (Keys::generate(), IdentityStorage::Ephemeral),
    };
    let http_client = local_owner_identity::build_http_client(
        reqwest::Client::builder()
            .resolve("localhost", std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
            .pool_idle_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(1),
    );

    AppState {
        keys: Mutex::new(keys),
        identity_storage: AtomicU8::new(identity_storage as u8),
        http_client,
        media_fetch_client: build_media_fetch_client().expect(
            "media_fetch_client must build without redirects; a fallback would leak media auth",
        ),
        relay_url_override: Mutex::new(None),
        #[cfg(not(feature = "local-owner-profile"))]
        managed_agent_restore_pending: AtomicBool::new(false),
        #[cfg(not(feature = "local-owner-profile"))]
        managed_agent_profile_reconcile_enabled: AtomicBool::new(true),
        #[cfg(not(feature = "local-owner-profile"))]
        shutdown_started: AtomicBool::new(false),
        #[cfg(not(feature = "local-owner-profile"))]
        managed_agent_runtime_transition: Mutex::new(()),
        identity_mutation: Mutex::new(()),
        #[cfg(not(feature = "local-owner-profile"))]
        managed_agents_store_lock: Mutex::new(()),
        #[cfg(not(feature = "local-owner-profile"))]
        channel_templates_store_lock: Mutex::new(()),
        #[cfg(not(feature = "local-owner-profile"))]
        managed_agent_processes: Mutex::new(HashMap::new()),
        #[cfg(not(feature = "local-owner-profile"))]
        session_config_cache: Mutex::new(HashMap::new()),
        #[cfg(not(feature = "local-owner-profile"))]
        huddle_state: Mutex::new(HuddleState::default()),
        #[cfg(not(feature = "local-owner-profile"))]
        huddle_audio: Default::default(),
        #[cfg(not(feature = "local-owner-profile"))]
        app_handle: Mutex::new(None),
        #[cfg(not(feature = "local-owner-profile"))]
        media_proxy_port: AtomicU16::new(0),
        #[cfg(not(feature = "local-owner-profile"))]
        prevent_sleep: Arc::new(Mutex::new(
            crate::prevent_sleep::PreventSleepState::default(),
        )),
        keyring_locked: AtomicBool::new(false),
        identity_lost: AtomicBool::new(false),
        relaunch_required: AtomicBool::new(false),
        reset_failed: AtomicBool::new(false),
        #[cfg(all(feature = "mesh-llm", not(feature = "local-owner-profile")))]
        mesh_llm_runtime: AsyncMutex::new(None),
        #[cfg(all(feature = "mesh-llm", not(feature = "local-owner-profile")))]
        mesh_recovery: crate::mesh_llm::MeshRecoveryState::default(),
        #[cfg(all(feature = "mesh-llm", not(feature = "local-owner-profile")))]
        mesh_coordinator: AsyncMutex::new(None),
        #[cfg(not(feature = "local-owner-profile"))]
        pending_owned_channels: Mutex::new(std::collections::HashSet::new()),
    }
}
