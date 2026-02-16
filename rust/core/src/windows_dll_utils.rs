/// Information extracted from the current DLL's version info resource.
#[derive(Debug, PartialEq, Clone)]
pub struct DllInfo {
    /// The `CompanyName` field from the DLL's `VS_VERSION_INFO` resource.
    pub company_name: String,
    /// The `InternalName` field from the DLL's `VS_VERSION_INFO` resource.
    pub internal_name: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GetDllInfoError {
    /// The DLL info could not be determined.
    UnexpectedError,
}

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(lptstr_filename: *const u16, lpdw_handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(
        lptstr_filename: *const u16,
        dw_handle: u32,
        dw_len: u32,
        lp_data: *mut core::ffi::c_void,
    ) -> i32;
    fn VerQueryValueW(
        p_block: *const core::ffi::c_void,
        lp_sub_block: *const u16,
        lplp_buffer: *mut *const core::ffi::c_void,
        pu_len: *mut u32,
    ) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Extracts a named string from the version info block by first reading the
/// `\VarFileInfo\Translation` table to determine the language/codepage pair.
unsafe fn query_string(data: &[u8], key: &str) -> Result<String, GetDllInfoError> {
    let translation_query = to_wide("\\VarFileInfo\\Translation");
    let mut ptr: *const core::ffi::c_void = std::ptr::null();
    let mut len: u32 = 0;

    unsafe {
        if VerQueryValueW(
            data.as_ptr().cast(),
            translation_query.as_ptr(),
            &mut ptr,
            &mut len,
        ) == 0
            || ptr.is_null()
            || len < 4
        {
            return Err(GetDllInfoError::UnexpectedError);
        }

        let lang_codepage = ptr.cast::<u16>();
        let lang = *lang_codepage;
        let codepage = *lang_codepage.add(1);

        let sub_block = to_wide(&format!(
            "\\StringFileInfo\\{lang:04x}{codepage:04x}\\{key}"
        ));
        let mut value_ptr: *const core::ffi::c_void = std::ptr::null();
        let mut value_len: u32 = 0;

        if VerQueryValueW(
            data.as_ptr().cast(),
            sub_block.as_ptr(),
            &mut value_ptr,
            &mut value_len,
        ) == 0
            || value_ptr.is_null()
            || value_len == 0
        {
            return Err(GetDllInfoError::UnexpectedError);
        }

        // value_len includes the null terminator
        let slice =
            std::slice::from_raw_parts(value_ptr.cast::<u16>(), (value_len - 1) as usize);
        String::from_utf16(slice).map_err(|_| GetDllInfoError::UnexpectedError)
    }
}

/// Get info for the DLL this was compiled into, including version resource fields.
///
/// # Errors
///
/// Returns `GetDllInfoError::UnexpectedError` if the DLL path cannot be determined
/// or the version info resource cannot be read.
pub fn get_current_dll_info() -> Result<DllInfo, GetDllInfoError> {
    let path = process_path::get_dylib_path().ok_or(GetDllInfoError::UnexpectedError)?;
    let path_wide = to_wide(
        path.to_str()
            .ok_or(GetDllInfoError::UnexpectedError)?,
    );

    unsafe {
        let mut handle: u32 = 0;
        let size = GetFileVersionInfoSizeW(path_wide.as_ptr(), &mut handle);
        if size == 0 {
            return Err(GetDllInfoError::UnexpectedError);
        }

        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path_wide.as_ptr(), handle, size, data.as_mut_ptr().cast()) == 0 {
            return Err(GetDllInfoError::UnexpectedError);
        }

        let company_name = query_string(&data, "CompanyName")?;
        let internal_name = query_string(&data, "InternalName")?;

        Ok(DllInfo {
            company_name,
            internal_name,
        })
    }
}
