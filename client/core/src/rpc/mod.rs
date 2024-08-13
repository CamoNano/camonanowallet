mod client;
mod manager;
mod result;
mod work;
mod wrapped;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use client::ClientRpc;
pub use manager::RpcManager;
pub use result::{RpcFailure, RpcFailures, RpcResult, RpcSuccess};
pub use work::{WorkHandle, WorkManager, WorkResult};
pub use wrapped::{Rpc, RpcCommands};

pub(super) fn get_current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

pub(super) fn get_ban_expiration(ban_seconds: u64) -> u64 {
    get_current_time().wrapping_add(ban_seconds)
}
