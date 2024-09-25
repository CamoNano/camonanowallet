use crate::rpc::{RpcManager, RpcResult};
use crate::{CoreClientConfig, CoreClientError};
use log::{debug, info};
use std::collections::HashMap;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

#[cfg(not(target_arch = "wasm32"))]
use std::thread;
#[cfg(target_arch = "wasm32")]
use wasm_thread as thread;

use thread::{spawn as spawn_thread, JoinHandle};

const TEN_MILLIS: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct WorkResult {
    pub work_hash: [u8; 32],
    pub rpc_result: RpcResult<[u8; 8]>,
}
pub type WorkHandle = JoinHandle<WorkResult>;

fn resolve_handle(handle: WorkHandle, work_hash: [u8; 32]) -> WorkResult {
    match handle.join() {
        Ok(result) => result,
        Err(_) => WorkResult {
            work_hash,
            rpc_result: Err(CoreClientError::ThreadHandleError),
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
        let worker = spawn_thread(move || {
            let as_hex = hex::encode(work_hash).to_uppercase();
            debug!("WorkManager: getting work for {as_hex}");

            // TODO: can all these `expect()`s somehow be recovered from? If so, implement error handling.
            #[cfg(not(target_arch = "wasm32"))]
            let rpc_result = {
                let rpc_future = RpcManager().work_generate(&config, work_hash, None);
                let rt = tokio::runtime::Runtime::new()
                    .expect("WorkManager::request_work() failed to create new Tokio runtime");
                rt.block_on(rpc_future)
            };
            #[cfg(target_arch = "wasm32")]
            let rpc_result = {
                // "If it's stupid and it works, it's not stupid"
                let (tx, rx) = std::sync::mpsc::channel();
                let config = config.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let rpc_future = RpcManager().work_generate(&config, work_hash, None);
                    tx.send(rpc_future.await).expect(
                        "WorkManager::request_work() failed to send RPC result through channel",
                    );
                });
                rx.recv().expect(
                    "WorkManager::request_work() failed to receive RPC result through channel",
                )
            };

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
