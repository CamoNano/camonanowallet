use super::WorkResult;
use crate::rpc::RpcManager;
use crate::CoreClientConfig;
use log::debug;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen_futures::spawn_local;

pub use wasm_thread::sleep;

#[derive(Debug)]
pub struct WorkHandle {
    result: Rc<RefCell<Option<WorkResult>>>,
    is_finished: Rc<AtomicBool>,
}
impl WorkHandle {
    pub fn new(config: CoreClientConfig, work_hash: [u8; 32]) -> WorkHandle {
        let result = Rc::new(RefCell::new(None));
        let result_clone = Rc::clone(&result);

        let is_finished = Rc::new(AtomicBool::new(false));
        let is_finished_clone = Rc::clone(&is_finished);

        spawn_local(async move {
            let as_hex = hex::encode(work_hash).to_uppercase();
            debug!("WorkManager: getting work for {as_hex}");

            let rpc_future = RpcManager().work_generate(&config, work_hash, None);
            let rpc_result = rpc_future.await;

            debug!("WorkManager: got work for {as_hex}");
            let work_result = WorkResult {
                work_hash,
                rpc_result,
            };
            *result_clone.borrow_mut() = Some(work_result);
            is_finished_clone.store(true, Ordering::SeqCst);
        });

        WorkHandle {
            result,
            is_finished,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.load(Ordering::SeqCst)
    }

    pub fn resolve(self, _: [u8; 32]) -> WorkResult {
        while !self.is_finished() {}
        self.result
            .borrow_mut()
            .take()
            .expect("Broken WASM WorkHandle::new() code: marked unfinished request as finished")
    }
}
