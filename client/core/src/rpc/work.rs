use crate::rpc::{RpcManager, RpcResult};
use crate::CoreClientConfig;
use log::{debug, info};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use tokio::runtime::Handle as TokioHandle;
use tokio::task::{block_in_place, spawn, JoinHandle};

const TEN_MILLIS: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct WorkResult {
    pub work_hash: [u8; 32],
    pub rpc_result: RpcResult<[u8; 8]>,
}
pub type WorkHandle = JoinHandle<WorkResult>;

fn resolve_handle(handle: WorkHandle, work_hash: [u8; 32]) -> WorkResult {
    match block_in_place(|| TokioHandle::current().block_on(handle)) {
        Ok(result) => result,
        Err(err) => WorkResult {
            work_hash,
            rpc_result: Err(err.into()),
        },
    }
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

        let config = config.clone();
        let worker = spawn(async move {
            let as_hex = hex::encode(work_hash).to_uppercase();
            debug!("WorkManager: getting work for {as_hex}");
            let rpc_result = RpcManager().work_generate(&config, work_hash, None).await;
            debug!("WorkManager: got work for {as_hex}");
            WorkResult {
                work_hash,
                rpc_result,
            }
        });
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
                return resolve_handle(handle, work_hash);
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
            removed.push(resolve_handle(handle, work_hash))
        }
        removed
    }

    /// Returns how many requests are currently running.
    pub fn n_requests(&self) -> usize {
        self.handles.len()
    }
}
