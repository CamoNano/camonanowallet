#![warn(unused_crate_dependencies, unsafe_code)]

mod error;
mod init;
mod logging;
mod storage;

use clap::Parser;
use client::{
    core::{SecretBytes, WalletSeed},
    Client, ClientError, Command, WalletFrontend,
};
use error::CliError;
use init::{prompt_password, Init};
use log::debug;
use std::io::{stdin, stdout, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use storage::{load_config, save_config, save_wallet_overriding};
use tokio::runtime::Runtime;
use tokio::task;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// The wallet will only ever save to disk this often in the work cache loop.
/// Note that this does not mean that we will *always* save this often:
/// This is just a speed limit.
const SAVE_TIMER: Duration = Duration::from_millis(2000);

#[derive(Debug, Zeroize, ZeroizeOnDrop)]
struct CliClient {
    name: String,
    key: SecretBytes<32>,
    client: Client,
}
impl CliClient {
    fn new(seed: WalletSeed, name: String, key: SecretBytes<32>) -> Result<CliClient, CliError> {
        let client = Client::new(seed, load_config()?)?;
        Ok(CliClient { name, key, client })
    }

    fn save_to_disk(&mut self) -> Result<(), CliError> {
        debug!("Saving wallet to disk");
        save_config(self.client.core.config.clone().into())?;
        save_wallet_overriding(self, &self.name, &self.key)
    }

    async fn work_cache_loop(mut self, stop: Receiver<()>) -> Result<CliClient, CliError> {
        // Try not to spam the disk:
        // Save at most once per 2 seconds.
        let mut last_save = Instant::now();
        let mut should_save = false;

        loop {
            let message = stop.recv_timeout(Duration::from_millis(10));
            // No stop signal (timeout)
            if let Err(RecvTimeoutError::Timeout) = message {
                // Save to disk if cache has been updated
                should_save |= self.client.update_work_cache().await?;

                if should_save && last_save.elapsed() >= SAVE_TIMER {
                    self.save_to_disk()?;
                    last_save = Instant::now();
                    should_save = false;
                }
            }
            // Yes stop signal
            else {
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

            let work_cache_loop = task::spawn(self.work_cache_loop(receiver));

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
                Err(err) => println!("{err:?}: {err}"),
            }
        }
    }

    fn start(self) {
        let rt = Runtime::new().expect("could not create Tokio runtime");
        rt.block_on(self._start_cli());
    }
}
impl WalletFrontend for CliClient {
    fn println(s: &str) {
        println!("{s}");
    }

    fn clear_screen() {
        print!("{}[2J", 27 as char);
    }

    fn authenticate(&self) -> Result<(), client::ClientError> {
        if prompt_password()? == self.key {
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

    if let Err(err) = logger.start_logging() {
        println!("Failed to start logging: {err}");
    }

    // May be None if not opening a wallet
    if let Some(client) = client {
        client.start();
    }
}
