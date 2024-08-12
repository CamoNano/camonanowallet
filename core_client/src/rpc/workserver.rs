use crate::rpc::{RpcManager, RpcResult};
use crate::CoreClientConfig;
use log::{debug, warn};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::task::yield_now;

const RECV_TIMEOUT: Duration = Duration::from_millis(10);

/// Create a work server and work client
pub fn create_work_server(config: CoreClientConfig) -> (WorkClient, WorkServer) {
    let (requests_sender, requests_receiver) = channel::<WorkRequest>();
    let (results_sender, results_receiver) = channel::<WorkResult>();
    let client = WorkClient {
        requests_channel: requests_sender,
        results_channel: results_receiver,
        requests: vec![],
        results: vec![],
    };
    let server = WorkServer {
        requests_channel: requests_receiver,
        results_channel: results_sender,
        requests: vec![],
        config,
    };
    (client, server)
}

macro_rules! remove {
    ($self:tt.$vec:ident.hash, $value:expr) => {{
        let index = $self.$vec.iter().position(|item| item.hash == $value);
        index.map(|index| $self.$vec.remove(index))
    }};

    ($self:tt.$vec:ident, $value:expr) => {{
        let index = $self.$vec.iter().position(|hash| hash == $value);
        index.map(|index| $self.$vec.remove(index))
    }};
}

#[derive(Debug, Error)]
pub enum WorkServerDisconnected {
    #[error("Work client unable to contact work server (request channel)")]
    SendRequest,
    #[error("Work client unable to contact work server (result channel)")]
    ReceiveResult,
    #[error("Work server unable to contact work client (result channel)")]
    SendResult,
    #[error("Work server unable to contact work client (request channel)")]
    ReceiveRequest,
}
impl WorkServerDisconnected {
    fn to_work_result(self, hash: [u8; 32]) -> WorkResult {
        WorkResult {
            hash,
            rpc_result: Err(self.into()),
        }
    }
}

#[derive(Debug)]
struct WorkRequest {
    hash: [u8; 32],
    config: CoreClientConfig,
}

#[derive(Debug)]
pub struct WorkResult {
    pub hash: [u8; 32],
    pub rpc_result: RpcResult<[u8; 8]>,
}

#[derive(Debug)]
pub struct WorkClient {
    requests_channel: Sender<WorkRequest>,
    results_channel: Receiver<WorkResult>,
    /// Requests that we have sent, but have not heard back on
    requests: Vec<[u8; 32]>,
    /// Results that we have acknowledged
    results: Vec<WorkResult>,
}
impl WorkClient {
    fn ack_request(&mut self, request: [u8; 32]) {
        // remove request from "waiting list"
        remove!(self.requests, &request);
        self.requests.push(request)
    }

    fn ack_result(&mut self, result: WorkResult) {
        // remove request from "waiting list"
        remove!(self.requests, &result.hash);
        self.results.push(result)
    }

    /// Acknowledge all results that we have received
    fn update(&mut self) -> Result<(), WorkServerDisconnected> {
        loop {
            match self.results_channel.recv_timeout(RECV_TIMEOUT) {
                Ok(result) => self.ack_result(result),
                Err(RecvTimeoutError::Timeout) => return Ok(()),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(WorkServerDisconnected::ReceiveResult)
                }
            };
        }
    }

    /// Send a work request to the server. Returns immediately.
    ///
    /// The only way for this to error is if the server has disconnected.
    pub fn request_work(
        &mut self,
        config: &CoreClientConfig,
        hash: [u8; 32],
    ) -> Result<(), WorkServerDisconnected> {
        self.ack_request(hash);
        let request = WorkRequest {
            hash,
            config: config.clone(),
        };
        if self.requests_channel.send(request).is_err() {
            warn!("Work client lost connection to server (send-request channel)");
            return Err(WorkServerDisconnected::SendRequest);
        }
        Ok(())
    }

    /// Wait for a work request to resolve.
    ///
    /// Panics if work has not been requested for this hash
    pub async fn wait_on(&mut self, hash: [u8; 32]) -> WorkResult {
        if remove!(self.requests, &hash).is_none() {
            panic!("Attempted to wait on work which hasn't been requested from the server");
        }

        let time = SystemTime::now();
        let mut last_log_time = 0;
        loop {
            if let Err(err) = self.update() {
                remove!(self.requests, &hash);
                return err.to_work_result(hash);
            }
            if let Some(result) = remove!(self.results.hash, hash) {
                remove!(self.requests, &hash);
                return result;
            }

            if let Ok(elapsed) = time.elapsed() {
                if elapsed.as_secs() > last_log_time {
                    log::debug!(
                        "WorkClient::wait_on(): waiting on work for {}",
                        hex::encode(hash).to_uppercase()
                    );
                    last_log_time += 1;
                }
            }
            yield_now().await;
        }
    }
}

#[derive(Debug)]
pub struct WorkServer {
    results_channel: Sender<WorkResult>,
    requests_channel: Receiver<WorkRequest>,
    /// Requests that we have acknowledged
    requests: Vec<[u8; 32]>,
    /// Most recent config that we have received from the client
    config: CoreClientConfig,
}
impl WorkServer {
    fn ack_request(&mut self, request: WorkRequest) {
        self.config = request.config;
        remove!(self.requests, &request.hash);
        self.requests.push(request.hash)
    }

    /// Acknowledge all requests and purge any duplicates received while processing a batch of work.
    ///
    /// The only way to get an error is if the channel has disconnected.
    fn update(&mut self, finished: &[[u8; 32]]) -> Result<(), WorkServerDisconnected> {
        // Acknowledge new requests
        loop {
            match self.requests_channel.recv_timeout(RECV_TIMEOUT) {
                Ok(request) => self.ack_request(request),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(WorkServerDisconnected::ReceiveRequest)
                }
            }
        }
        // Purge duplicates
        for hash in finished {
            remove!(self.requests, hash);
        }
        Ok(())
    }

    /// Start the work server. Will stop if the client has disconnected.
    pub async fn start(mut self) -> Result<(), WorkServerDisconnected> {
        let rpc = RpcManager();
        loop {
            let mut results = vec![];
            // TODO: handle multiple hashes at once
            if let Some(request) = self.requests.pop() {
                debug!(
                    "WorkServer: getting work for {}",
                    hex::encode(request).to_uppercase()
                );
                let rpc_result = rpc.work_generate(&self.config, request, None).await;
                results.push(WorkResult {
                    hash: request,
                    rpc_result,
                });
                debug!(
                    "WorkServer: got work for {}",
                    hex::encode(request).to_uppercase()
                );
            }

            let hashes = results
                .iter()
                .map(|result| result.hash)
                .collect::<Vec<[u8; 32]>>();
            for result in results {
                // Send results back
                if self.results_channel.send(result).is_err() {
                    // The only way to get an error is if the channel has disconnected
                    warn!("Work server lost connection to client (send-result channel)");
                    return Err(WorkServerDisconnected::SendResult);
                }
            }
            if let Err(err) = self.update(&hashes) {
                warn!("Work server lost connection to client (receive-request channel)");
                return Err(err);
            }
            yield_now().await;
        }
    }
}
