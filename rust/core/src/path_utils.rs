#![allow(missing_docs)]

use std::{
    ffi::{CStr, CString},
    path::PathBuf,
};

use objc::{runtime::Object, sel, sel_impl};

#[derive(Debug, PartialEq, Clone)]
pub struct BundleInfo {
    pub identifier: String,
    pub resource_path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GetBundleInfoError {
    UnexpectedError,
}

/// Get the bundle info for the bundle this was compiled into.
///
/// # Errors
///
/// Returns a `GetBundleInfoError::UnexpectedError` if we receive an invalid string
/// from the OS API calls to find the bundle.
pub fn get_current_bundle_info() -> Result<BundleInfo, GetBundleInfoError> {
    let dylib_path = process_path::get_dylib_path().ok_or(GetBundleInfoError::UnexpectedError)?;
    let bundle_root = dylib_path
        .parent()
        .ok_or(GetBundleInfoError::UnexpectedError)?
        .parent()
        .ok_or(GetBundleInfoError::UnexpectedError)?
        .parent()
        .ok_or(GetBundleInfoError::UnexpectedError)?
        .to_str()
        .ok_or(GetBundleInfoError::UnexpectedError)?;

    unsafe {
        let nsstring = objc::class!(NSString);
        let cstring = CString::new(bundle_root).map_err(|_| GetBundleInfoError::UnexpectedError)?;

        let bundle_path: *mut Object =
            objc::msg_send![nsstring, stringWithUTF8String: cstring.as_ptr()];

        let nsbundle = objc::class!(NSBundle);
        let bundle: *mut Object = objc::msg_send![nsbundle, bundleWithPath: bundle_path];
        if bundle.is_null() {
            return Err(GetBundleInfoError::UnexpectedError);
        }
        let resource_root_nsstring: *mut Object = objc::msg_send![bundle, resourcePath];
        if resource_root_nsstring.is_null() {
            return Err(GetBundleInfoError::UnexpectedError);
        }
        let resource_path_str = CStr::from_ptr(objc::msg_send![resource_root_nsstring, UTF8String])
            .to_str()
            .map_err(|_| GetBundleInfoError::UnexpectedError)?;
        let resource_path = PathBuf::from(resource_path_str);
        let identifier_nsstring: *mut Object = objc::msg_send![bundle, bundleIdentifier];
        if identifier_nsstring.is_null() {
            return Err(GetBundleInfoError::UnexpectedError);
        }
        let identifier = CStr::from_ptr(objc::msg_send![identifier_nsstring, UTF8String])
            .to_str()
            .map_err(|_| GetBundleInfoError::UnexpectedError)?
            .to_string();
        Ok(BundleInfo {
            identifier,
            resource_path,
        })
    }
}
