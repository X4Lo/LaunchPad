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
//!
//! ## Config sync
//!
//! On startup, the app checks whether the registry value exists
//! and updates `auto_start` in config.json to match reality.
//! This handles cases where the user manually removed the key.

#[cfg(windows)]
fn subkey_wide() -> Vec<u16> {
    "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn value_name_wide() -> Vec<u16> {
    "Launchpad"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// Check whether the Launchpad auto-start registry value exists.
#[cfg(windows)]
pub fn is_auto_start_enabled() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
    };

    let subkey = subkey_wide();
    let value_name = value_name_wide();

    unsafe {
        let mut hkey = std::mem::zeroed();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_name.as_ptr()),
            None,
            None,
            None,
            None,
        );
        result.is_ok()
    }
}

#[cfg(not(windows))]
pub fn is_auto_start_enabled() -> bool {
    false
}

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

    let subkey = subkey_wide();
    let value_name = value_name_wide();

    unsafe {
        let mut hkey = std::mem::zeroed();
        if RegCreateKeyW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            &mut hkey,
        )
        .is_err()
        {
            log::error!("Failed to open registry key for auto-start");
            return false;
        }

        if enable {
            let data: &[u8] = std::slice::from_raw_parts(
                wide_exe.as_ptr() as *const u8,
                (wide_exe.len() - 1) * 2,
            );
            if RegSetValueExW(
                hkey,
                PCWSTR::from_raw(value_name.as_ptr()),
                0,
                REG_SZ,
                Some(data),
            )
            .is_err()
            {
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
