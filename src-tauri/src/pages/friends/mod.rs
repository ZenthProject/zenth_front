pub mod friends;
pub mod database;

// Re-export commands for easier access
pub use friends::{
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
};

// Re-export types
pub use friends::UserPublicInfoResponse;
pub use friends::SyncResult;
pub use database::queries::{FriendInfo, PendingRequestInfo};
