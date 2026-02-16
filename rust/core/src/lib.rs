#![allow(missing_docs)]
#![allow(unexpected_cfgs)]
#![doc = include_str!("../docs_boilerplate.md")]
#![doc = include_str!("../README.md")]

pub mod parameters;

#[cfg(target_os = "macos")]
pub mod mac_bundle_utils;

#[cfg(target_os = "windows")]
pub mod windows_dll_utils;
