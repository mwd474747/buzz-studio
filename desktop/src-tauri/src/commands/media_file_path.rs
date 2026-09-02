/// Resolve the real filesystem path of an already-opened file descriptor.
///
/// Returns the path the kernel associates with the inode, not the pathname
/// used to open it. Immune to post-open renames/symlink swaps.
#[cfg(target_os = "macos")]
pub(super) fn fd_real_path(file: &std::fs::File) -> Result<std::path::PathBuf, String> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let mut buf = vec![0u8; libc::PATH_MAX as usize];
    let ret = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) };
    if ret == -1 {
        return Err(format!(
            "fcntl F_GETPATH failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let nul = buf.iter().position(|&byte| byte == 0).unwrap_or(buf.len());
    let path = std::str::from_utf8(&buf[..nul]).map_err(|error| error.to_string())?;
    Ok(std::path::PathBuf::from(path))
}

#[cfg(target_os = "linux")]
pub(super) fn fd_real_path(file: &std::fs::File) -> Result<std::path::PathBuf, String> {
    use std::os::unix::io::AsRawFd;
    std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
pub(super) fn fd_real_path(file: &std::fs::File) -> Result<std::path::PathBuf, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED,
    };
    let handle = file.as_raw_handle() as *mut core::ffi::c_void;
    let mut buffer = vec![0u16; 1024];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED,
        )
    };
    if len == 0 {
        return Err(format!(
            "GetFinalPathNameByHandleW failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let path = String::from_utf16_lossy(&buffer[..len as usize]);
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path);
    Ok(std::path::PathBuf::from(path))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(super) fn fd_real_path(_file: &std::fs::File) -> Result<std::path::PathBuf, String> {
    Err("fd_real_path not supported on this platform".to_string())
}
