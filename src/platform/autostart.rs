//! Start with Windows.
//!
//! Writes the per-user `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value
//! "Launchpad" (no admin rights needed) via the `auto-launch` crate. The entry passes
//! `--minimized`, so a login start goes straight to the tray.

pub const MINIMIZED_ARG: &str = "--minimized";

fn auto_launch() -> Result<auto_launch::AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // auto-launch writes `<path> <args>` verbatim, so quote the path for folders with spaces.
    let quoted = format!("\"{}\"", exe.to_string_lossy());
    let mut b = auto_launch::AutoLaunchBuilder::new();
    b.set_app_name("Launchpad").set_app_path(&quoted).set_args(&[MINIMIZED_ARG]);
    b.build().map_err(|e| e.to_string())
}

pub fn is_enabled() -> bool {
    auto_launch().and_then(|a| a.is_enabled().map_err(|e| e.to_string())).unwrap_or(false)
}

/// Enable (pointing at the running exe) or disable. Only called when the user flips the
/// switch: re-registering on every start would let a second copy (a USB build, a dev
/// build) silently take over the entry.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let al = auto_launch()?;
    if enabled {
        al.enable().map_err(|e| e.to_string())
    } else if al.is_enabled().unwrap_or(false) {
        al.disable().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
