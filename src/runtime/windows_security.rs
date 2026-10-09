//! No elevation: current-user/SYSTEM ACLs are installed at pipe creation.
use interprocess::os::windows::security_descriptor::{AsSecurityDescriptor, SecurityDescriptor};
use std::{
    io,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::Authorization::*,
    Security::*,
    System::{Com::CoTaskMemFree, Threading::*},
    UI::Shell::*,
};
pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {} // Unique owned HANDLE; Windows handles are usable across threads.
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
pub fn sid(pid: Option<u32>) -> io::Result<String> {
    let process =
        pid.map(|p| Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, p) }));
    let raw = process
        .as_ref()
        .map(|p| p.0)
        .unwrap_or_else(|| unsafe { GetCurrentProcess() });
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(raw, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(token);
    let mut size = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size);
    }
    if size == 0 {
        return Err(io::Error::last_os_error());
    }
    // Aligned storage for TOKEN_USER followed by its SID data.
    let mut bytes = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            bytes.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*bytes.as_ptr().cast::<TOKEN_USER>() };
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // Convert copies into Rust ownership before releasing the Win32 allocation.
    let result = unsafe { widestring::U16CStr::from_ptr_str(text) }.to_string_lossy();
    unsafe {
        LocalFree(text.cast());
    }
    Ok(result)
}
pub fn descriptor() -> io::Result<SecurityDescriptor> {
    let text = widestring::U16CString::from_str(format!(
        "D:P(A;OICI;GA;;;SY)(A;OICI;GA;;;{})",
        sid(None)?
    ))
    .map_err(|_| io::Error::other("invalid SID"))?;
    SecurityDescriptor::deserialize(&text)
}
pub fn protect_path(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_attributes() & 0x400 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "reparse point rejected",
        ));
    }
    let p = widestring::U16CString::from_os_str(path.as_os_str())
        .map_err(|_| io::Error::other("invalid path"))?;
    let sd = descriptor()?;
    if unsafe {
        SetFileSecurityW(
            p.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            sd.as_sd().cast_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn local_app_data() -> io::Result<PathBuf> {
    let mut text = ptr::null_mut();
    let result =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, ptr::null_mut(), &mut text) };
    if result < 0 {
        return Err(io::Error::other("LocalAppData unavailable"));
    }
    let path = PathBuf::from(unsafe { widestring::U16CStr::from_ptr_str(text) }.to_os_string());
    unsafe {
        CoTaskMemFree(text.cast());
    }
    Ok(path)
}
