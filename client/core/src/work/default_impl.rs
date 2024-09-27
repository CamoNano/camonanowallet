use super::WorkResult;
use crate::rpc::RpcManager;
use crate::{CoreClientConfig, CoreClientError};
use log::debug;
use tokio::runtime::Handle as TokioHandle;
use tokio::task::{block_in_place, spawn, JoinHandle};

pub use std::thread::sleep;

#[derive(Debug)]
pub struct WorkHandle(JoinHandle<WorkResult>);
impl WorkHandle {
    pub fn new(config: CoreClientConfig, work_hash: [u8; 32]) -> WorkHandle {
        let handle = spawn(async move {
            let as_hex = hex::encode(work_hash).to_uppercase();
            debug!("WorkManager: getting work for {as_hex}");

            let rpc_future = RpcManager().work_generate(&config, work_hash, None);
            let rpc_result = tokio::runtime::Runtime::new()
                .expect("non-WASM WorkHandle::new() failed to create new Tokio runtime")
                .block_on(rpc_future);

            debug!("WorkManager: got work for {as_hex}");
            WorkResult {
                work_hash,
                rpc_result,
            }
        });
        WorkHandle(handle)
    }

    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }

    pub fn resolve(self, work_hash: [u8; 32]) -> WorkResult {
        match block_in_place(|| TokioHandle::current().block_on(self.0)) {
            Ok(result) => result,
            Err(err) => WorkResult {
                work_hash,
                rpc_result: Err(CoreClientError::ThreadHandleError(err.to_string())),
            },
        }
    }
}
