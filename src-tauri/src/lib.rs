//! Application entry point.
//!
//! Declares all internal modules, imports Tauri commands and starts the application.

#![allow(dead_code, unused_variables, unused_mut, unused_imports)]

// Internal modules
mod db;
mod pages;
mod utils;
mod api;
mod commands;
mod websocket;
mod plugins;
pub mod session;
pub mod config;

use db::{MasterDb, paths::init_db_dir};
use tauri::Manager;

/// Warns in debug mode if TLS certificate validation is disabled.
///
/// This function is compiled out entirely in release builds.
#[cfg(debug_assertions)]
fn check_dev_tls_config() {
    if !config::accept_invalid_certs() {
        eprintln!("[zenth] dev: strict TLS - set ZENTH_ACCEPT_INVALID_CERTS=1 in .env to allow self-signed certificates");
    }
}

use pages::vault::{get_vault_status, set_vault_password, remove_vault_password, verify_vault_password, unlock_vault, lock_vault};
use pages::recovery::{init_recovery_key, verify_recovery_words, get_recovery_status, export_backup, import_backup, publish_recovery_key_dht, submit_recovery_claim};
use pages::{
    keygen::keygen::generate_random_string_chunk,
    login::login::{login, logout, get_ws_auth, configure_wipe, check_session},
    register::register::register,
    settings::{
        save_setting,
        load_settings,
        save_all_settings,
        get_setting,
        delete_setting,
        reset_settings
    },
    friends::{
        search_user,
        send_friend_request,
        list_pending_requests,
        accept_friend_request,
        reject_friend_request,
        list_friends,
        remove_friend,
        get_my_public_key,
        sync_friend_requests,
        sync_friend_responses,
        cancel_friend_request,
        retry_friend_request,
        block_friend,
        unblock_friend,
        list_blocked_friends,
        set_my_avatar,
        get_my_avatar,
        set_friend_avatar,
        rename_friend,
        get_friend_fingerprint,
        mark_friend_verified,
        sync_accepted_contacts,
        lock_conversation,
        check_conversation_pin,
        remove_conversation_lock,
    },
    chat::{
        send_message,
        get_messages,
        sync_messages,
        mark_message_read,
        clear_all_sessions,
        delete_message_secure,
        init_self_space,
        get_chat_ttl,
        set_chat_ttl,
        file_transfer::{
            prepare_file_transfer,
            start_file_download,
            cancel_file_transfer,
        },
    },
    update::{
        check_update, 
        download_update, 
        apply_update
    },
    sync::{
        sync_accounts_user,
        publish_pairing_keys,
        generate_pairing_qr,
        verify_pairing_qr,
        send_sync_key,
        fetch_sync_key,
        relay_pull_messages,
        get_relay_status,
        list_paired_devices,
        revoke_paired_device,
        relay_push_all_contacts,
        relay_push_manifest,
    },
    wipe::{
        wipe_user_data, 
        delete_account
    }
};

use utils::emergency::{
    get_emergency_by_country,
    get_all_emergency_numbers,
    list_emergency_countries,
};

use commands::{
    analyze_file,
    sanitize_file,
    sanitize_by_type,
    get_supported_formats,
    share_text,
    synchronize_accounts
};

use websocket::{
    ws_connect,
    ws_send,
    ws_disconnect,
    ws_is_connected,
};

use plugins::keystore::{
    store_credential, 
    retrieve_credential, 
    delete_credential
};

/// Starts the Tauri application.
///
/// Initializes the environment, the master database, registers all Tauri commands
/// and runs the event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    {
        let _ = dotenvy::dotenv();
    }

    #[cfg(debug_assertions)]
    check_dev_tls_config();

    tauri::Builder::default()
        .setup(|app| {
            if let Ok(data_dir) = app.path().app_data_dir() {
                init_db_dir(data_dir);
            }
            MasterDb::open().expect("Failed to initialize master database");
            pages::chat::file_transfer::init();
            Ok(())
        })
        .plugin(plugins::keystore::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_websocket::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            sync_accounts_user,
            publish_pairing_keys,
            generate_pairing_qr,
            verify_pairing_qr,
            send_sync_key,
            fetch_sync_key,
            relay_pull_messages,
            get_relay_status,
            list_paired_devices,
            revoke_paired_device,
            relay_push_all_contacts,
            relay_push_manifest,
            register,
            login,
            logout,
            check_session,
            get_ws_auth,
            configure_wipe,
            generate_random_string_chunk,
            save_setting,
            load_settings,
            save_all_settings,
            get_setting,
            delete_setting,
            reset_settings,
            search_user,
            send_friend_request,
            list_pending_requests,
            accept_friend_request,
            reject_friend_request,
            list_friends,
            remove_friend,
            get_my_public_key,
            sync_friend_requests,
            sync_friend_responses,
            cancel_friend_request,
            retry_friend_request,
            block_friend,
            unblock_friend,
            list_blocked_friends,
            set_my_avatar,
            get_my_avatar,
            set_friend_avatar,
            rename_friend,
            get_friend_fingerprint,
            mark_friend_verified,
            lock_conversation,
            check_conversation_pin,
            remove_conversation_lock,
            send_message,
            get_messages,
            sync_messages,
            mark_message_read,
            clear_all_sessions,
            delete_message_secure,
            init_self_space,
            get_chat_ttl,
            set_chat_ttl,
            get_emergency_by_country,
            get_all_emergency_numbers,
            list_emergency_countries,
            analyze_file,
            sanitize_file,
            sanitize_by_type,
            get_supported_formats,
            ws_connect,
            ws_send,
            ws_disconnect,
            ws_is_connected,
            wipe_user_data,
            delete_account,
            prepare_file_transfer,
            start_file_download,
            cancel_file_transfer,
            get_vault_status,
            set_vault_password,
            remove_vault_password,
            verify_vault_password,
            unlock_vault,
            lock_vault,
            check_update,
            download_update,
            apply_update,
            share_text,
            synchronize_accounts,
            store_credential,
            retrieve_credential,
            delete_credential,
            init_recovery_key,
            verify_recovery_words,
            get_recovery_status,
            export_backup,
            import_backup,
            publish_recovery_key_dht,
            submit_recovery_claim,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| eprintln!("Tauri application error: {e}"));
}
