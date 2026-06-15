/// Settings for notifications and alerts
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NotificationSettings {
    pub notifications_enabled: i32,
    pub notification_sound: i32,
    pub notification_preview: i32,
    pub notify_mentions_only: i32,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            notifications_enabled: 1,
            notification_sound: 1,
            notification_preview: 0,
            notify_mentions_only: 0,
        }
    }
}
