#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    rustdoc::private_doc_tests,
    rustdoc::unescaped_backticks,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]
#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

pub mod parameters;
