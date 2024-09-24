#![warn(unused_crate_dependencies, unsafe_code)]
#![allow(unused)] // TODO: remove

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

/// Initialize the wallet software. Should only be run once.
#[wasm_bindgen]
pub fn init() {
    let logger: Logger = get_log_level().unwrap().into();
    match logger.start_logging() {
        Ok(()) => (),
        Err(err) => web_api::alert!("Failed to start logging: {err}"),
    }
}

#[wasm_bindgen]
pub fn new_client() -> Result<AppClient, String> {
    init::new().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn import_client() -> Result<AppClient, String> {
    init::import().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn load_client() -> Result<Option<AppClient>, String> {
    init::load().map_err(|err| err.to_string())
}

#[wasm_bindgen]
pub fn launch_client() -> Result<AppClient, String> {
    let loaded = init::load().map_err(|err| err.to_string())?;
    web_api::log!("Creating new wallet");
    Ok(loaded.unwrap_or(new_client()?))
}

#[wasm_bindgen]
pub fn main() {
    init();

    // let client: AppClient = match launch_client() {
    //     Ok(client) => client,
    //     Err(err) => {
    //         web_api::alert!("{:?}", err);
    //         return;
    //     }
    // };

    // client.start();
}