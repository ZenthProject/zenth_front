/// Settings for network routing and anonymity networks (Tor, I2P, Lokinet)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkSettings {
    pub default_relay: String,
    pub use_multiple_relays: i32,
    pub relay_circuit_length: i32,
    pub auto_change_circuit: i32,
    pub circuit_change_minutes: i32,
    pub force_onion_routing: i32,
    pub use_guards_nodes: i32,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            default_relay: "tor".to_string(),
            use_multiple_relays: 0,
            relay_circuit_length: 3,
            auto_change_circuit: 1,
            circuit_change_minutes: 10,
            force_onion_routing: 1,
            use_guards_nodes: 1,
        }
    }
}
