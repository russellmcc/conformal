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
pub fn create_store(domain: &str, defaults: HashMap<String, Value>) -> impl Store + use<> {
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

#[cfg(test)]
mod tests {
    use super::*;
    const KEY: &str = "test";

    // Don't run these tests under miri
    #[cfg(not(miri))]
    mod os_store {
        use super::*;
        #[test]
        fn starts_empty() {
            let mut store = create_os_store("com.p61.test.starts_empty");
            store.reset();
            assert_eq!(store.get(KEY), None);
        }

        #[test]
        fn can_set() {
            let mut store = create_os_store("com.p61.test.can_set");
            store.reset();
            assert_eq!(store.get(KEY), None);
            store.set(KEY, Value::Switch(true));
            assert_eq!(store.get(KEY), Some(Value::Switch(true)));
            store.set(KEY, Value::Switch(false));
            assert_eq!(store.get(KEY), Some(Value::Switch(false)));
        }

        #[test]
        fn domains_dont_conflict() {
            let mut store1 = create_os_store("com.p61.test.domains_dont_conflict1");
            let mut store2 = create_os_store("com.p61.test.domains_dont_conflict2");
            store1.reset();
            store2.reset();
            store1.set(KEY, Value::Switch(true));
            assert_eq!(store1.get(KEY), Some(Value::Switch(true)));
            assert_eq!(store2.get(KEY), None);
        }

        #[test]
        fn can_reset() {
            let mut store = create_os_store("com.p61.test.can_reset");
            store.reset();
            store.set(KEY, Value::Switch(true));
            assert_eq!(store.get(KEY), Some(Value::Switch(true)));
            store.reset();
            assert_eq!(store.get(KEY), None);
        }

        #[test]
        fn persists() {
            {
                let mut store = create_os_store("com.p61.test.persists");
                store.reset();
                store.set(KEY, Value::Switch(true));
            }

            let store = create_os_store("com.p61.test.persists");
            assert_eq!(store.get(KEY), Some(Value::Switch(true)));
        }
    }

    #[test]
    fn cannot_set_unknown_key() {
        let mut store = create_with_fake_os_store(HashMap::new());
        assert_eq!(
            store.set(KEY, Value::Switch(true)),
            Err(StoreError::UnknownKey)
        );
    }

    #[test]
    fn cannot_get_unknown_key() {
        let store = create_with_fake_os_store(HashMap::new());
        assert_eq!(store.get(KEY), Err(StoreError::UnknownKey));
    }

    #[test]
    fn get_key_returns_default() {
        let store =
            create_with_fake_os_store(HashMap::from_iter([(KEY.to_string(), Value::Switch(true))]));
        assert_eq!(store.get(KEY), Ok(Value::Switch(true)));
    }

    #[test]
    fn can_set_key() {
        let mut store = create_with_fake_os_store(HashMap::from_iter([(
            KEY.to_string(),
            Value::Switch(false),
        )]));
        assert_eq!(store.set(KEY, Value::Switch(true)), Ok(()));
        assert_eq!(store.get(KEY), Ok(Value::Switch(true)));
    }
}
