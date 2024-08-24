use std::rc;

use super::Value;

pub trait Listener {
    fn parameter_changed(&self, unique_id: &str, value: &Value);
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetError {
    NotFound,
    WrongType,
    InvalidValue,
    InternalError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetGrabbedError {
    NotFound,
    InternalError,
}

pub trait Store {
    fn get(&self, unique_id: &str) -> Option<Value>;

    fn get_info(&self, unique_id: &str) -> Option<super::Info>;

    /// Set a parameter value
    ///
    /// # Errors
    ///
    ///  - Returns `NotFound` if the no parameter with the given `unique_id` is in the store.
    ///  - Returns `WrongType` if the parameter with the given `unique_id` does not have a type that matches `value`.
    ///  - Returns `InvalidValue` if the provided `value` is out of the valid range for the parameter with the given `unique_id`.
    ///  - Returns `InternalError` if the store is unable to set the value due to a bad internal state
    fn set(&mut self, unique_id: &str, value: Value) -> Result<(), SetError>;

    /// Set the "grabbed" state of a parameter.
    ///
    /// # Errors
    ///
    ///  - Returns `NotFound` if the no parameter with the given `unique_id` is in the store.
    ///  - Returns `InternalError` if the store is unable to set the value due to a bad internal state
    fn set_grabbed(&mut self, unique_id: &str, grabbed: bool) -> Result<(), SetGrabbedError>;

    /// Note that there can only be one listener at a time!
    fn set_listener(&mut self, listener: rc::Weak<dyn Listener>);
}
