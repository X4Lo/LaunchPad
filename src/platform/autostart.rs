//! Auto-start with Windows via registry.
//!
//! ## How it works
//!
//! When enabled, writes a REG_SZ value to:
//!
//! ```text
//! HKEY_CURRENT_USER
//!   Software
//!     Microsoft
//!       Windows
//!         CurrentVersion
//!           Run
//!             "Launchpad" = "C:\path\to\launchpad.exe"
//! ```
//!
//! Windows runs all values under this key at user login.
//! When disabled, the value is deleted.
//!
//! This is the standard per-user startup mechanism — no admin
//! rights required, no Startup folder shortcut needed.

/// Enable or disable auto-start with Windows via registry.
#[cfg(windows)]
pub fn set_auto_start(enable: bool) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCreateKeyW, RegDeleteValueW, RegSetValueExW, HKEY_CURRENT_USER, REG_SZ,
    };

    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let exe_str = exe_path.to_string_lossy();
    let wide_exe: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "Launchpad"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = std::mem::zeroed();
        let result = RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            &mut hkey,
        );
        if result.is_err() {
            log::error!("Failed to open registry key for auto-start");
            return false;
        }

        if enable {
            let data: &[u8] = std::slice::from_raw_parts(
                wide_exe.as_ptr() as *const u8,
                (wide_exe.len() - 1) * 2, // exclude null terminator
            );
            let result = RegSetValueExW(
                hkey,
                PCWSTR::from_raw(value_name.as_ptr()),
                0,
                REG_SZ,
                Some(data),
            );
            if result.is_err() {
                log::error!("Failed to write auto-start registry value");
                return false;
            }
            log::info!("Auto-start enabled");
        } else {
            let _ = RegDeleteValueW(hkey, PCWSTR::from_raw(value_name.as_ptr()));
            log::info!("Auto-start disabled");
        }
        true
    }
}

#[cfg(not(windows))]
pub fn set_auto_start(_enable: bool) -> bool {
    log::warn!("Auto-start is only supported on Windows");
    false
}
