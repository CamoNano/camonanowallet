#![warn(unused_crate_dependencies, unsafe_code)]
#![allow(unused)] // TODO: remove

// We don't use `getrandom` directly, as it is a subdependency.
// However, we include this in Cargo.toml to force usage of the "js" feature,
// which is necessary for WASM compilation to succeed.
use getrandom as _;

mod app_client;
mod error;
mod init;
mod logging;
mod storage;
mod web_api;

use app_client::AppClient;
use client::{core::SecretBytes, ClientError};
use logging::Logger;
use storage::get_log_level;
use wasm_bindgen::prelude::*;

/// Initialize the software. Should only be run once.
#[wasm_bindgen]
pub fn init_client() {
    let logger: Logger = get_log_level().unwrap().into();
    match logger.start_logging() {
        Ok(()) => (),
        Err(err) => web_api::alert!("Failed to start logging: {err}"),
    }
}

#[wasm_bindgen]
pub fn new_wallet() -> Result<AppClient, String> {
    init::new().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn import_wallet() -> Result<AppClient, String> {
    init::import().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn load_wallet() -> Result<Option<AppClient>, String> {
    init::load().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn launch_wallet() -> Result<AppClient, String> {
    let loaded = init::load().map_err(|err| err.to_string())?;
    if let Some(wallet) = loaded {
        Ok(wallet)
    } else {
        web_api::log!("Creating new wallet");
        Ok(loaded.unwrap_or(new_wallet()?))
    }
}

#[wasm_bindgen]
pub fn main() {
    init_client();

    // let client: AppClient = match launch_client() {
    //     Ok(client) => client,
    //     Err(err) => {
    //         web_api::alert!("{:?}", err);
    //         return;
    //     }
    // };

    // client.start();
}