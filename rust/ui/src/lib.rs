#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]
#![doc = include_str!("../../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

use conformal_component::parameters;

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
    fn set(
        &mut self,
        unique_id: &str,
        value: parameters::Value,
    ) -> Result<(), parameters::store::SetError>;

    /// Sets the grabbed state of the parameter store.
    ///
    /// # Errors
    ///
    /// - `SetError::NotFound` if there is no parameter with the given unique ID.
    fn set_grabbed(
        &mut self,
        unique_id: &str,
        grabbed: bool,
    ) -> Result<(), parameters::store::SetGrabbedError>;
}

pub use web_ui::Size;
pub use web_ui::Ui;
pub use wry::raw_window_handle;
