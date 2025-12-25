//! Platform abstraction layer
//!
//! This module provides a trait-based abstraction for platform-specific operations.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Platform-specific operations trait
pub trait Platform {
    /// Get file permissions (Unix mode)
    fn get_permissions(&self, path: &Path) -> std::io::Result<u32>;

    /// Set file permissions
    fn set_permissions(&self, path: &Path, mode: u32) -> std::io::Result<()>;

    /// Get file modification time
    fn get_mtime(&self, path: &Path) -> std::io::Result<SystemTime>;

    /// Set file modification time
    fn set_mtime(&self, path: &Path, mtime: SystemTime) -> std::io::Result<()>;

    /// Check if path is a symbolic link
    fn is_symlink(&self, path: &Path) -> bool;

    /// Read symbolic link target
    fn read_symlink(&self, path: &Path) -> std::io::Result<std::path::PathBuf>;

    /// Create symbolic link
    fn create_symlink(&self, target: &Path, link: &Path) -> std::io::Result<()>;

    /// Check if path is a directory
    fn is_directory(&self, path: &Path) -> bool;

    /// Check if path exists
    fn exists(&self, path: &Path) -> bool;

    /// Create directory with parents
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;

    /// Get the default permissions for a file
    fn default_file_permissions(&self) -> u32;

    /// Get the default permissions for a directory
    fn default_dir_permissions(&self) -> u32;
}

/// Unix platform implementation
#[cfg(unix)]
pub struct UnixPlatform;

#[cfg(unix)]
impl Platform for UnixPlatform {
    fn get_permissions(&self, path: &Path) -> std::io::Result<u32> {
        use std::fs;
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(path)?;
        Ok(meta.mode() & 0o7777)
    }

    fn set_permissions(&self, path: &Path, mode: u32) -> std::io::Result<()> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(mode);
        fs::set_permissions(path, perms)
    }

    fn get_mtime(&self, path: &Path) -> std::io::Result<SystemTime> {
        use std::fs;
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(path)?;
        let mtime = meta.mtime();
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(mtime as u64))
    }

    fn set_mtime(&self, _path: &Path, _mtime: SystemTime) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let duration = _mtime
                .duration_since(UNIX_EPOCH)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "mtime before epoch"))?;
            let secs = duration.as_secs() as libc::time_t;
            let usecs = duration.subsec_micros() as libc::suseconds_t;
            let times = [
                libc::timeval {
                    tv_sec: secs,
                    tv_usec: usecs,
                },
                libc::timeval {
                    tv_sec: secs,
                    tv_usec: usecs,
                },
            ];
            let c_path = std::ffi::CString::new(_path.as_os_str().as_bytes())
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))?;
            let ret = unsafe { libc::utimes(c_path.as_ptr(), times.as_ptr()) };
            if ret != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    fn is_symlink(&self, path: &Path) -> bool {
        use std::fs;
        fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn read_symlink(&self, path: &Path) -> std::io::Result<std::path::PathBuf> {
        use std::fs;
        fs::read_link(path)
    }

    fn create_symlink(&self, target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    fn is_directory(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn default_file_permissions(&self) -> u32 {
        0o644
    }

    fn default_dir_permissions(&self) -> u32 {
        0o755
    }
}

/// Windows platform implementation
#[cfg(windows)]
pub struct WindowsPlatform;

#[cfg(windows)]
impl Platform for WindowsPlatform {
    fn get_permissions(&self, _path: &Path) -> std::io::Result<u32> {
        // Windows uses readonly flag, not full Unix permissions
        Ok(0o644)
    }

    fn set_permissions(&self, path: &Path, mode: u32) -> std::io::Result<()> {
        use std::fs;
        let mut perms = fs::metadata(path)?.permissions();
        // Set readonly if write bit is not set
        let readonly = (mode & 0o200) == 0;
        perms.set_readonly(readonly);
        fs::set_permissions(path, perms)
    }

    fn get_mtime(&self, path: &Path) -> std::io::Result<SystemTime> {
        use std::fs;
        let meta = fs::metadata(path)?;
        meta.modified()
    }

    fn set_mtime(&self, _path: &Path, _mtime: SystemTime) -> std::io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, SetFileTime, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_WRITE, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::SystemServices::FILETIME;

        let duration = _mtime
            .duration_since(UNIX_EPOCH)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "mtime before epoch"))?;
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos() as u64;
        let windows_ticks = (secs * 10_000_000) + (nanos / 100);
        let windows_epoch_adjust = 11644473600u64 * 10_000_000u64;
        let ft = windows_ticks
            .checked_add(windows_epoch_adjust)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "mtime overflow"))?;

        let filetime = FILETIME {
            dwLowDateTime: ft as u32,
            dwHighDateTime: (ft >> 32) as u32,
        };

        let mut wide: Vec<u16> = _path.as_os_str().encode_wide().collect();
        wide.push(0);

        let handle: HANDLE = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_BACKUP_SEMANTICS,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let ok = unsafe { SetFileTime(handle, std::ptr::null(), std::ptr::null(), &filetime) };
        unsafe { CloseHandle(handle) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn is_symlink(&self, _path: &Path) -> bool {
        // Symlinks are not well-supported on Windows
        false
    }

    fn read_symlink(&self, path: &Path) -> std::io::Result<std::path::PathBuf> {
        use std::fs;
        fs::read_link(path)
    }

    fn create_symlink(&self, _target: &Path, _link: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks not supported on Windows",
        ))
    }

    fn is_directory(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn default_file_permissions(&self) -> u32 {
        0o644
    }

    fn default_dir_permissions(&self) -> u32 {
        0o755
    }
}

/// Get the platform implementation for the current OS
pub fn current_platform() -> &'static impl Platform {
    #[cfg(unix)]
    {
        static PLATFORM: UnixPlatform = UnixPlatform;
        &PLATFORM
    }

    #[cfg(windows)]
    {
        static PLATFORM: WindowsPlatform = WindowsPlatform;
        &PLATFORM
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Fallback for other platforms
        static PLATFORM: GenericPlatform = GenericPlatform;
        &PLATFORM
    }
}

#[cfg(windows)]
pub fn utf8_to_utf16(input: &str) -> std::io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = std::ffi::OsStr::new(input).encode_wide().collect();
    wide.push(0);
    Ok(wide)
}

#[cfg(windows)]
pub fn utf16_to_utf8(input: &[u16]) -> std::io::Result<String> {
    let nul_pos = input.iter().position(|c| *c == 0).unwrap_or(input.len());
    String::from_utf16(&input[..nul_pos])
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid UTF-16"))
}

/// Generic platform implementation for other OSes
#[cfg(not(any(unix, windows)))]
pub struct GenericPlatform;

#[cfg(not(any(unix, windows)))]
impl Platform for GenericPlatform {
    fn get_permissions(&self, _path: &Path) -> std::io::Result<u32> {
        Ok(0o644)
    }

    fn set_permissions(&self, _path: &Path, _mode: u32) -> std::io::Result<()> {
        Ok(())
    }

    fn get_mtime(&self, path: &Path) -> std::io::Result<SystemTime> {
        use std::fs;
        let meta = fs::metadata(path)?;
        meta.modified()
    }

    fn set_mtime(&self, _path: &Path, _mtime: SystemTime) -> std::io::Result<()> {
        Ok(())
    }

    fn is_symlink(&self, _path: &Path) -> bool {
        false
    }

    fn read_symlink(&self, path: &Path) -> std::io::Result<std::path::PathBuf> {
        use std::fs;
        fs::read_link(path)
    }

    fn create_symlink(&self, _target: &Path, _link: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks not supported",
        ))
    }

    fn is_directory(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn default_file_permissions(&self) -> u32 {
        0o644
    }

    fn default_dir_permissions(&self) -> u32 {
        0o755
    }
}

/// Helper function to convert system time to DOS datetime
pub fn system_time_to_dosDateTime(time: SystemTime) -> Option<u32> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    let secs = duration.as_secs();

    if secs < 315532800 {
        // Before 1980-01-01, not supported by DOS datetime
        return None;
    }

    // DOS datetime can represent up to 2107-12-31
    if secs > 4354819200 {
        // After 2107-12-31
        return None;
    }

    let mut days_since_1980 = (secs - 315532800) / 86400;

    // Calculate year, month, day
    let mut year = 1980;
    let mut month: u32 = 1;
    let mut day: u32;

    // Days in each month for leap and non-leap years
    while days_since_1980 >= 366 || (days_since_1980 >= 365 && !is_leap_year(year)) {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        days_since_1980 -= days_in_year;
        year += 1;
    }

    let days_in_month = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    day = days_since_1980 as u32;  // 0-indexed day within the year

    for (m, &dim) in days_in_month.iter().enumerate() {
        if day < dim as u32 {
            month = (m + 1) as u32;
            day = day + 1;  // Convert to 1-indexed
            break;
        }
        day -= dim as u32;
    }

    // Calculate time
    let secs_in_day = (secs % 86400) as u32;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;

    // DOS datetime format:
    // YYYYYYYM MMMDDDDD hhhhhmmm mmmsssss
    // |   year   | month | day | hour | minute | second |
    let dos_time: u32 = (hour << 11) | (minute << 5) | (second / 2);
    let dos_date: u32 = (((year - 1980) as u32) << 9) | (month << 5) | day;

    Some((dos_date << 16) | dos_time)
}

/// Helper function to convert DOS datetime to SystemTime
pub fn dosDateTime_to_systemTime(dosDateTime: u32) -> Option<SystemTime> {
    let dos_date = (dosDateTime >> 16) as u32;
    let dos_time = (dosDateTime & 0xFFFF) as u32;

    let year = ((dos_date >> 9) & 0x7F) as u64 + 1980;
    let month = ((dos_date >> 5) & 0x0F) as u32;
    let day = (dos_date & 0x1F) as u32;
    let hour = ((dos_time >> 11) & 0x1F) as u64;
    let minute = ((dos_time >> 5) & 0x3F) as u64;
    let second = ((dos_time & 0x1F) * 2) as u64;

    // Validate ranges
    if month < 1 || month > 12 || day < 1 || day > 31 {
        return None;
    }

    // Calculate total days from 1970-01-01 (Unix epoch)
    let mut days = 0u64;

    // Days from 1970 to year-1
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    // Days from Jan 1 to target date in the target year
    for m in 1..month {
        days += days_in_month(year, m) as u64;
    }
    days += day as u64 - 1;  // day-1 because day 1 means 0 extra days

    // Convert to seconds since Unix epoch
    let secs = days * 86400 + hour * 3600 + minute * 60 + second;
    Some(UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: u64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dos_datetime_roundtrip() {
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let dos = system_time_to_dosDateTime(time).unwrap();
        let back = dosDateTime_to_systemTime(dos).unwrap();

        let diff = back
            .duration_since(time)
            .unwrap_or(time.duration_since(back).unwrap())
            .as_secs();

        // Should be within 10 seconds (DOS precision + rounding)
        assert!(diff <= 10);
    }
}
