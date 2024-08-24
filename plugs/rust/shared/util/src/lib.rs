//! This crate is a grab-bag of miscellaneous utilities used by components in this repo.
//!
//! We may break out some of these utilities into their own crates in the future, but
//! this acts a convenient place to put them for now.

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

pub mod f32;
pub mod iter;
pub mod slice_ops;
pub mod window;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
