use super::error::AppError;
use super::storage::{load_config, save_config, save_wallet};
use super::web_api;
use client::{
    core::{SecretBytes, WalletSeed},
    Client, ClientError, Command, WalletFrontend,
};
use log::debug;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use zeroize::{Zeroize, ZeroizeOnDrop};

// /// The wallet will only ever save to disk this often in the work cache loop.
// /// Note that this does not mean that we will *always* save this often:
// /// This is just a speed limit.
// const SAVE_TIMER: Duration = Duration::from_millis(2000);

#[wasm_bindgen]
#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct AppClient {
    pub(crate) key: SecretBytes<32>,
    pub(crate) client: Client,
}
impl AppClient {
    pub(crate) fn new(seed: WalletSeed, key: SecretBytes<32>) -> Result<AppClient, AppError> {
        let client = Client::new(seed, load_config()?)?;
        Ok(AppClient { key, client })
    }

    fn save_to_disk(&mut self) -> Result<(), AppError> {
        debug!("Saving wallet to disk");
        save_config(self.client.core.config.clone().into())?;
        save_wallet(self, &self.key)
    }

    // async fn work_cache_loop(mut self, stop: Receiver<()>) -> Result<AppClient, AppError> {
    //     // Try not to spam the disk:
    //     // Save at most once per 2 seconds.
    //     let mut last_save = Instant::now();
    //     let mut should_save = false;

    //     loop {
    //         let message = stop.recv_timeout(Duration::from_millis(10));
    //         // No stop signal (timeout)
    //         if let Err(RecvTimeoutError::Timeout) = message {
    //             // Save to disk if cache has been updated
    //             should_save |= self.client.update_work_cache().await?;

    //             if should_save && last_save.elapsed() >= SAVE_TIMER {
    //                 self.save_to_disk()?;
    //                 last_save = Instant::now();
    //                 should_save = false;
    //             }
    //         }
    //         // Yes stop signal
    //         else {
    //             break;
    //         }
    //     }
    //     Ok(self)
    // }

    async fn _start_cli(mut self) {
        loop {
            let mut input = String::new();

            // print!("> ");
            // stdout().flush().expect("failed to flush stdout");

            // let (sender, receiver) = channel();

            // let work_cache_loop = spawn(self.work_cache_loop(receiver));

            let mut input = web_api::prompt!("Enter command: ").unwrap();
            // stdin().read_line(&mut input).expect("failed to read stdin");

            // sender.send(()).expect("Failed to stop work cache loop");
            // self = work_cache_loop
            //     .await
            //     .expect("Failed to await work cache loop")
            //     .expect("Error in work cache loop");

            let result = Command::execute(&mut self, &input).await;
            self.save_to_disk().expect("Failed to save wallet to disk");

            match result {
                Ok(true) => (),
                Ok(false) => break,
                Err(err) => web_api::alert!("{err:?}: {err}"),
            }
        }
    }

    pub(crate) fn start(self) {
        spawn_local(async move {
            self._start_cli().await;
        });
    }
}
impl WalletFrontend for AppClient {
    fn println(s: &str) {
        web_api::log!("{s}");
    }

    fn clear_screen() {
        web_api::log!("--------------------");
    }

    fn authenticate(&self) -> Result<(), client::ClientError> {
        if web_api::prompt_password()? == self.key {
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
