#![allow(missing_docs)]
#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

use conformal_component::parameters;
use conformal_core::parameters::store;

mod preferences_convert;
mod protocol;
mod server;
mod web_ui;

pub trait ParameterStore {
    fn get(&self, unique_id: &str) -> Option<parameters::Value>;

    fn get_info(&self, unique_id: &str) -> Option<parameters::Info>;

    /// Sets a value on the parameter store.
    ///
    /// # Errors
    ///
    /// - `SetError::NotFound` if there is no parameter with the given unique ID.
    /// - `SetError::InvalidValue` if the value is not valid for the parameter.
    /// - `SetError::WrongType` if the parameter is not a type that matches `value`.
    fn set(&mut self, unique_id: &str, value: parameters::Value) -> Result<(), store::SetError>;

    /// Sets the grabbed state of the parameter store.
    ///
    /// # Errors
    ///
    /// - `SetError::NotFound` if there is no parameter with the given unique ID.
    fn set_grabbed(&mut self, unique_id: &str, grabbed: bool)
        -> Result<(), store::SetGrabbedError>;
}

pub use web_ui::Size;
pub use web_ui::Ui;
pub use wry::raw_window_handle;
