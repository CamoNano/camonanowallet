use super::app_client::AppClient;
use super::error::AppError;
use super::storage::{load_wallet, save_wallet, wallet_exists};
use super::web_api::{alert, prompt, prompt_new_password, prompt_password};
use client::{core::WalletSeed, types::Hex32Bytes as Seed};
use std::str::FromStr;

pub fn new() -> Result<AppClient, AppError> {
    let seed = WalletSeed::from(rand::random::<[u8; 32]>());
    alert!("seed: {}", seed.as_hex());

    let cli_client = AppClient::new(seed, prompt_new_password()?)?;
    save_wallet(&cli_client, &cli_client.key)?;
    Ok(cli_client)
}

pub fn import() -> Result<AppClient, AppError> {
    let seed = WalletSeed::from(Seed::from_str(&prompt!("Enter 64-character hex seed:")?)?.0);
    let cli_client = AppClient::new(seed, prompt_new_password()?)?;
    save_wallet(&cli_client, &cli_client.key)?;
    Ok(cli_client)
}

pub fn load() -> Result<Option<AppClient>, AppError> {
    if wallet_exists() {
        let client = load_wallet(prompt_password()?)?;
        Ok(client)
    } else {
        Ok(None)
    }
}
