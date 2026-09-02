fn install_local_owner_invoke(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let handler: Box<tauri::ipc::InvokeHandler<tauri::Wry>> = Box::new(tauri::generate_handler![
        title_bar_double_click,
        get_identity,
        get_local_owner_profile,
        import_identity,
        get_profile,
        update_profile,
        update_profile_at_relay,
        get_user_profile,
        get_users_batch,
        search_users,
        get_presence,
        get_default_relay_url,
        auto_connect_default_relay_enabled,
        is_shared_identity,
        get_relay_ws_url,
        get_relay_http_url,
        apply_workspace,
        get_channels,
        get_channel_details,
        get_channel_members,
        get_canvas,
        get_feed,
        search_messages,
        send_channel_message,
        get_forum_posts,
        get_forum_thread,
        get_thread_replies,
        get_channel_window,
        get_channel_messages_before,
        add_reaction,
        remove_reaction,
        get_event,
        open_dm,
        hide_dm,
        pick_and_upload_media,
        pick_and_upload_image,
        upload_media_bytes,
        fetch_media_bytes,
        download_image,
        download_file,
        copy_image_to_clipboard,
        copy_text_to_clipboard,
        relay_requires_membership,
        get_my_relay_membership,
        fetch_join_policy,
        get_contact_list,
        set_contact_list,
        sign_event,
        create_auth_event,
        nip44_encrypt_to_self,
        nip44_decrypt_from_self,
        fetch_workspace_icon,
        set_window_vibrancy,
    ]);

    builder.invoke_handler(move |invoke| {
        let command = invoke.message.command();
        let recovery_command = matches!(
            command,
            "get_local_owner_profile" | "get_identity" | "import_identity"
        );
        if !recovery_command {
            let state = invoke.message.state_ref().get::<AppState>();
            if crate::local_owner_profile::recovery_active(&state) {
                invoke
                    .resolver
                    .reject("identity recovery is required before Buzz can perform this action");
                return true;
            }
        }
        handler(invoke)
    })
}
