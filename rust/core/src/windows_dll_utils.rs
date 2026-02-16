use std::path::PathBuf;

/// Information about the DLL this code was compiled into.
#[derive(Debug, PartialEq, Clone)]
pub struct DllInfo {
    /// The full path to the DLL on disk.
    pub path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GetDllInfoError {
    /// The DLL path could not be determined.
    UnexpectedError,
}

/// Get info for the DLL this was compiled into.
///
/// # Errors
///
/// Returns `GetDllInfoError::UnexpectedError` if the DLL path cannot be determined.
pub fn get_current_dll_info() -> Result<DllInfo, GetDllInfoError> {
    let path = process_path::get_dylib_path().ok_or(GetDllInfoError::UnexpectedError)?;
    Ok(DllInfo { path })
}
