//! Session configuration.
//!
//! Controls cache lifetime, what is pre-loaded at login, and background sync behavior.
//! When user settings are wired to the database, update [`SessionConfig::load`] to read from there.

/// Controls cache lifetime and pre-loading behavior for a user session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// How long the session stays valid, in seconds.
    /// After this delay, the next command recreates the session (triggers Argon2id again).
    pub timeout_secs: u64,

    /// Whether to pre-load the friends list when the session is created.
    /// When `true`, [`list_friends`] returns instantly from memory with no database query.
    pub cache_friends: bool,

    /// Whether to run background sync (friend requests, responses, messages)
    /// immediately after login.
    pub background_sync: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 3600,
            cache_friends: true,
            background_sync: true,
        }
    }
}

impl SessionConfig {
    /// Returns the active session configuration.
    ///
    /// Currently returns [`Default::default`]. Once user settings are persisted
    /// to the database, this function should read from the `settings` table instead.
    pub fn load() -> Self {
        Self::default()
    }
}
