use super::error::AppError;
use crate::{ClientError, SecretBytes};
use client::core::nanopyrs::hashes::blake2b256;
use wasm_bindgen::prelude::*;
use web_sys::{Storage, Window};
use zeroize::Zeroize;

fn get_key(mut password: String) -> Result<SecretBytes<32>, ClientError> {
    let key = blake2b256(password.as_bytes());
    password.zeroize();
    Ok(key)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    #[allow(unsafe_code)]
    pub(crate) fn _log(s: &str);
}

/// API for `console.log(...)`
macro_rules! log {
    ($($t:tt)*) => {{
        use $crate::web_api::_log;
        _log(&format_args!($($t)*).to_string())
    }}
}
pub(crate) use log;

/// API for `alert(...)`
///
/// Also runs `console.log(...)`
macro_rules! alert {
    ($($t:tt)*) => {{
        $crate::web_api::log!($($t)*);
        use $crate::web_api::_alert;
        let result = _alert(&format_args!($($t)*).to_string());
        if let Err(err) = result {
            $crate::web_api::log!("Failed to send alert to browser: {err}")
        }
    }}
}
pub(crate) use alert;

/// API for `console.log(...)`
macro_rules! prompt {
    ($($t:tt)*) => {{
        use $crate::web_api::_prompt;
        _prompt(&format_args!($($t)*).to_string())
    }}
}
pub(crate) use prompt;

/// Get the browser window
fn get_window() -> Result<Window, AppError> {
    web_sys::window().ok_or(AppError::WindowUnavailable)
}

pub(crate) fn _alert(message: &str) -> Result<(), AppError> {
    get_window()?.alert_with_message(message).map_err(|err| {
        AppError::AlertError(
            err.as_string()
                .unwrap_or("N/A (unknown JsValue error)".into()),
        )
    })
}

pub(crate) fn _prompt(message: &str) -> Result<String, AppError> {
    let result = get_window()?.prompt_with_message(message).map_err(|err| {
        AppError::AlertError(
            err.as_string()
                .unwrap_or("N/A (unknown JsValue error)".into()),
        )
    })?;
    Ok(result.unwrap_or("".into()))
}

pub(crate) fn prompt_password() -> Result<SecretBytes<32>, ClientError> {
    let password = _prompt("Enter password:")
        .map_err(|err| ClientError::FailedToReadPassword(err.to_string()))?;
    get_key(password)
}

pub(crate) fn prompt_new_password() -> Result<SecretBytes<32>, ClientError> {
    let password = _prompt("Enter new password:")
        .map_err(|err| ClientError::FailedToReadPassword(err.to_string()))?;
    get_key(password)
}

/// Get the browser's storage
pub(crate) fn get_storage() -> Result<Storage, AppError> {
    get_window()?
        .local_storage()
        .map_err(|err| {
            AppError::StorageUnavailable(
                err.as_string()
                    .unwrap_or("N/A (unknown JsValue error)".into()),
            )
        })?
        .ok_or(AppError::StorageUnavailable("N/A (returned None)".into()))
}

/// Load data into local storage with the given key
pub(crate) fn set_item(storage: &Storage, key: &str, value: &str) -> Result<(), AppError> {
    storage.set_item(key, value).map_err(|err| {
        AppError::StorageSetError(
            err.as_string()
                .unwrap_or("N/A (unknown JsValue error)".into()),
        )
    })
}

/// Get a key's associated data from local storage
pub(crate) fn get_item(storage: &Storage, key: &str) -> Result<Option<String>, AppError> {
    storage.get_item(key).map_err(|err| {
        AppError::StorageGetError(
            err.as_string()
                .unwrap_or("N/A (unknown JsValue error)".into()),
        )
    })
}

/// Remove a key from local storage
pub(crate) fn remove_item(storage: &Storage, key: &str) -> Result<(), AppError> {
    storage.remove_item(key).map_err(|err| {
        AppError::StorageRemoveError(
            err.as_string()
                .unwrap_or("N/A (unknown JsValue error)".into()),
        )
    })
}
