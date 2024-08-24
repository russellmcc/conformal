use super::{OSStore, Value};
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Store {
    pub values: HashMap<String, Value>,
}

impl OSStore for Store {
    #[cfg(all(test, not(miri)))]
    fn reset(&mut self) {
        self.values.clear();
    }

    fn get(&self, unique_id: &str) -> Option<Value> {
        self.values.get(unique_id).cloned()
    }

    fn set(&mut self, unique_id: &str, value: Value) {
        self.values.insert(unique_id.to_string(), value);
    }
}
