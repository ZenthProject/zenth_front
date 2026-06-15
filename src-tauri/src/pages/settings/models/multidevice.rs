/// Settings for multi-device synchronization
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiDeviceSettings {
    pub allow_multi_device: i32,
    pub max_linked_devices: i32,
    pub sync_read_receipts: i32,
    pub sync_contacts: i32,
}

impl Default for MultiDeviceSettings {
    fn default() -> Self {
        Self {
            allow_multi_device: 0,
            max_linked_devices: 5,
            sync_read_receipts: 0,
            sync_contacts: 1,
        }
    }
}
