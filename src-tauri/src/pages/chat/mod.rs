pub mod messages;
pub mod database;
pub mod crypto;
pub mod file_transfer;

// Re-export commands for easier access
pub use messages::{
    send_message,
    get_messages,
    sync_messages,
    mark_message_read,
    clear_all_sessions,
    delete_message_secure,
    init_self_space,
    get_chat_ttl,
    set_chat_ttl,
};

// Re-export types
pub use messages::{MessageInfo, MessageSyncResult};
