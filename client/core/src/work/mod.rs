#[cfg(not(target_arch = "wasm32"))]
mod default_impl;
#[cfg(not(target_arch = "wasm32"))]
use default_impl as work_impl;

#[cfg(target_arch = "wasm32")]
mod wasm_impl;
#[cfg(target_arch = "wasm32")]
use wasm_impl as work_impl;

use crate::rpc::RpcResult;
use crate::CoreClientConfig;
use log::info;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use work_impl::{sleep, WorkHandle};

const TEN_MILLIS: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct WorkResult {
    pub work_hash: [u8; 32],
    pub rpc_result: RpcResult<[u8; 8]>,
}

#[derive(Debug, Default)]
pub struct WorkManager {
    handles: HashMap<[u8; 32], WorkHandle>,
}
impl WorkManager {
    /// Returns immediately.
    ///
    /// If already requested and in progress, the request is ignored.
    pub fn request_work(&mut self, config: &CoreClientConfig, work_hash: [u8; 32]) {
        if self.handles.contains_key(&work_hash) {
            return;
        }
        let worker = WorkHandle::new(config.clone(), work_hash);
        self.handles.insert(work_hash, worker);
    }

    /// Wait for a work request to resolve.
    ///
    /// Panics if work has not been requested for this hash.
    pub fn wait_on(&mut self, work_hash: [u8; 32]) -> WorkResult {
        let time = SystemTime::now();
        let mut last_log_time = 0;

        let handle = self
            .handles
            .remove(&work_hash)
            .expect("Attempted to wait on work which hasn't been requested");

        loop {
            if handle.is_finished() {
                return handle.resolve(work_hash);
            }

            if let Ok(elapsed) = time.elapsed() {
                if elapsed.as_secs() > last_log_time {
                    info!(
                        "Waiting on work for hash {}...",
                        hex::encode(work_hash).to_uppercase()
                    );
                    last_log_time += 1;
                }
            }
            sleep(TEN_MILLIS);
        }
    }

    /// Return all finished requests.
    pub async fn get_results(&mut self) -> Vec<WorkResult> {
        let mut to_remove = vec![];
        for (work_hash, handle) in self.handles.iter() {
            if handle.is_finished() {
                to_remove.push(*work_hash)
            }
        }
        let mut removed = vec![];
        for work_hash in to_remove {
            let handle = self
                .handles
                .remove(&work_hash)
                .expect("broken WorkManager::get_results() code: failed to remove handle");
            removed.push(handle.resolve(work_hash))
        }
        removed
    }

    /// Returns how many requests are currently running.
    pub fn n_requests(&self) -> usize {
        self.handles.len()
    }
}
