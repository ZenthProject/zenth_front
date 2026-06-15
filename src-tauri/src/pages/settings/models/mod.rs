pub mod appearance;
pub mod cryptography;
pub mod forward_secrecy;
pub mod ephemeral;
pub mod metadata;
pub mod network;
pub mod security;
pub mod identity;
pub mod multidevice;
pub mod storage;
pub mod notifications;
pub mod backup;
pub mod connection;
pub mod experimental;

use appearance::AppearanceSettings;
use cryptography::CryptographySettings;
use forward_secrecy::ForwardSecrecySettings;
use ephemeral::EphemeralSettings;
use metadata::MetadataSettings;
use network::NetworkSettings;
use security::SecuritySettings;
use identity::IdentitySettings;
use multidevice::MultiDeviceSettings;
use storage::StorageSettings;
use notifications::NotificationSettings;
use backup::BackupSettings;
use connection::ConnectionSettings;
use experimental::ExperimentalSettings;

/// Main settings structure composed of all setting categories
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    #[serde(flatten)]
    pub appearance: AppearanceSettings,

    #[serde(flatten)]
    pub cryptography: CryptographySettings,

    #[serde(flatten)]
    pub forward_secrecy: ForwardSecrecySettings,

    #[serde(flatten)]
    pub ephemeral: EphemeralSettings,

    #[serde(flatten)]
    pub metadata: MetadataSettings,

    #[serde(flatten)]
    pub network: NetworkSettings,

    #[serde(flatten)]
    pub security: SecuritySettings,

    #[serde(flatten)]
    pub identity: IdentitySettings,

    #[serde(flatten)]
    pub multidevice: MultiDeviceSettings,

    #[serde(flatten)]
    pub storage: StorageSettings,

    #[serde(flatten)]
    pub notifications: NotificationSettings,

    #[serde(flatten)]
    pub backup: BackupSettings,

    #[serde(flatten)]
    pub connection: ConnectionSettings,

    #[serde(flatten)]
    pub experimental: ExperimentalSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings::default(),
            cryptography: CryptographySettings::default(),
            forward_secrecy: ForwardSecrecySettings::default(),
            ephemeral: EphemeralSettings::default(),
            metadata: MetadataSettings::default(),
            network: NetworkSettings::default(),
            security: SecuritySettings::default(),
            identity: IdentitySettings::default(),
            multidevice: MultiDeviceSettings::default(),
            storage: StorageSettings::default(),
            notifications: NotificationSettings::default(),
            backup: BackupSettings::default(),
            connection: ConnectionSettings::default(),
            experimental: ExperimentalSettings::default(),
        }
    }
}

impl AppSettings {
    /// Helper method to get a flattened list of all field names
    pub fn field_names() -> Vec<&'static str> {
        vec![
            // Appearance
            "theme", "language", "font_size", "compact_mode", "message_bubble_style",
            // Cryptography
            "key_rotation_enabled", "key_rotation_days", "use_post_quantum",
            "signature_algorithm", "kem_algorithm",
            // Forward Secrecy
            "double_ratchet_enabled", "max_skip_message_keys", "ratchet_on_every_message",
            // Ephemeral
            "ephemeral_messages_default", "default_ephemeral_timer", "ephemeral_after_read",
            // Metadata
            "enable_message_padding", "padding_size", "random_delay_messages",
            "max_delay_seconds", "hide_message_size",
            // Network
            "default_relay", "use_multiple_relays", "relay_circuit_length",
            "auto_change_circuit", "circuit_change_minutes", "force_onion_routing",
            "use_guards_nodes",
            // Security
            "auto_lock_enabled", "auto_lock_timeout", "require_password_for_export",
            "wipe_after_failed_attempts", "max_failed_attempts", "secure_delete_messages",
            // Identity
            "require_safety_numbers", "warn_identity_change", "auto_accept_new_keys",
            "verify_all_devices",
            // MultiDevice
            "allow_multi_device", "max_linked_devices", "sync_read_receipts",
            "sync_contacts",
            // Storage
            "auto_download_files", "max_file_size_mb", "compress_images",
            "compress_quality", "auto_cleanup_old_messages", "cleanup_after_days",
            // Notifications
            "notifications_enabled", "notification_sound", "notification_preview",
            "notify_mentions_only",
            // Backup
            "encrypted_backup_enabled", "backup_frequency", "backup_location",
            "include_media_in_backup",
            // Connection
            "connection_timeout_seconds", "max_retry_attempts", "use_websocket_compression",
            "keepalive_interval_seconds",
            // Experimental
            "use_obfuscation", "quantum_resistant_mode", "paranoid_mode",
        ]
    }
}
