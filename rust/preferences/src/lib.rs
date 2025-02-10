#![allow(unexpected_cfgs)]
#![allow(missing_docs)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Value {
    Switch(bool),
}

/// This internal trait represents the raw OS store
trait OSStore {
    // only enabled for non-miri tests
    #[cfg(all(test, not(miri)))]
    fn reset(&mut self);
    fn get(&self, unique_id: &str) -> Option<Value>;
    fn set(&mut self, unique_id: &str, value: Value);
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use macos::create_os_store;

#[cfg(any(test, feature = "test-utils"))]
mod fake_os;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StoreError {
    UnknownKey,
}

pub trait Store {
    /// # Errors
    ///  - `StoreError::UnknownKey` if the key is not found
    fn get(&self, unique_id: &str) -> Result<Value, StoreError>;

    /// # Errors
    /// - `StoreError::UnknownKey` if the key is not found
    fn set(&mut self, unique_id: &str, value: Value) -> Result<(), StoreError>;
}

/// This trait stores "preference" data for the user. This will use
/// appropriate OS APIs to store this information
struct StoreImpl<O> {
    os_store: O,
    defaults: HashMap<String, Value>,
}

#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn create_store(domain: &str, defaults: HashMap<String, Value>) -> impl Store {
    StoreImpl {
        os_store: create_os_store(domain),
        defaults,
    }
}

#[must_use]
#[allow(clippy::implicit_hasher)]
#[cfg(any(test, feature = "test-utils"))]
pub fn create_with_fake_os_store(defaults: HashMap<String, Value>) -> impl Store {
    StoreImpl {
        os_store: fake_os::Store::default(),
        defaults,
    }
}

impl<O: OSStore> Store for StoreImpl<O> {
    fn get(&self, unique_id: &str) -> Result<Value, StoreError> {
        match self.defaults.get(unique_id) {
            Some(value) => Ok(self.os_store.get(unique_id).unwrap_or(value.clone())),
            None => Err(StoreError::UnknownKey),
        }
    }
    fn set(&mut self, unique_id: &str, value: Value) -> Result<(), StoreError> {
        match self.defaults.get(unique_id) {
            Some(_) => {
                self.os_store.set(unique_id, value);
                Ok(())
            }
            None => Err(StoreError::UnknownKey),
        }
    }
}
