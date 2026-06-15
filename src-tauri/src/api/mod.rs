pub mod errors;
pub mod register;
pub mod login;
pub mod friend;
pub mod prekeys;
pub mod delete;
pub mod update;
pub mod sync;
pub mod recovery;

// Re-export for convenience
pub use register::{RegisterConfig, DarknetType};
pub use friend::FriendConfig;
pub use delete::DeleteApiClient;
pub use sync::SyncApiClient;
pub use recovery::RecoveryApiClient;
