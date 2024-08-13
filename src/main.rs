#![warn(unused_crate_dependencies, unsafe_code)]

mod error;
mod init;
mod logging;
mod storage;

use clap::Parser;
use client::{
    core::{rpc::workserver::WorkServer, SecretBytes, WalletSeed},
    CliFrontend, Client, ClientError, Command,
};
use error::CliError;
use init::{prompt_password, Init};
use std::io::{stdin, stdout, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;
use storage::{load_config, save_config, save_wallet_overriding};
use tokio::runtime::Runtime;
use tokio::task;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
struct CliClient {
    client: Client,
}
impl CliClient {
    fn new(
        seed: WalletSeed,
        name: String,
        key: SecretBytes<32>,
    ) -> Result<(CliClient, WorkServer), CliError> {
        let (client, work_server) = Client::new(seed, name, key, load_config()?)?;
        Ok((CliClient { client }, work_server))
    }

    fn save_to_disk(&mut self) -> Result<(), CliError> {
        save_config(self.client.internal.config.clone().into())?;
        save_wallet_overriding(self, &self.client.name, &self.client.key)
    }

    fn work_cache_loop(mut self, stop: Receiver<()>) -> Result<CliClient, CliError> {
        loop {
            let message = stop.recv_timeout(Duration::from_millis(10));
            if let Err(RecvTimeoutError::Timeout) = message {
                self.client.update_work_cache()?;
            } else {
                break;
            }
        }
        Ok(self)
    }

    async fn _start_cli(mut self) {
        loop {
            print!("> ");
            stdout().flush().expect("failed to flush stdout");

            let (sender, receiver) = channel();

            let work_cache_loop = task::spawn(async move { self.work_cache_loop(receiver) });

            let mut input = String::new();
            stdin().read_line(&mut input).expect("failed to read stdin");

            sender.send(()).expect("Failed to stop work cache loop");
            self = work_cache_loop
                .await
                .expect("Failed to await work cache loop")
                .expect("Error in work cache loop");

            let result = Command::execute(&mut self, &input).await;
            self.save_to_disk().expect("Failed to save wallet to disk");

            match result {
                Ok(true) => (),
                Ok(false) => break,
                Err(err) => println!("{:?}", err),
            }
        }
    }

    fn start(self, work_server: WorkServer) {
        let rt = Runtime::new().expect("could not create Tokio runtime");
        rt.spawn(async move {
            let _ = work_server.start().await;
        });
        rt.block_on(self._start_cli());
    }
}
impl CliFrontend for CliClient {
    fn print(s: &str) {
        println!("{s}");
    }

    fn clear_screen() {
        print!("{}[2J", 27 as char);
    }

    fn authenticate(&self) -> Result<(), client::ClientError> {
        if prompt_password()? == self.client.key {
            Ok(())
        } else {
            Err(ClientError::InvalidPassword(aes_gcm::Error))
        }
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }
}

fn main() {
    let init = Init::parse().execute();
    let (client, logger) = match init {
        Ok((client, logger)) => (client, logger),
        Err(err) => {
            println!("{:?}", err);
            return;
        }
    };

    let (client, work_server) = match client {
        Some((client, work_server)) => (client, work_server),
        None => return,
    };

    match logger.start_logging() {
        Ok(()) => (),
        Err(err) => println!("Failed to start logging: {err}"),
    }

    client.start(work_server);
}
