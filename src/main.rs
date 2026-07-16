#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod app;
pub mod commands;
pub mod config;
pub mod models;
pub mod platform;
pub mod ui;
pub mod utils;

use app::LaunchpadApp;
use config::manager::ConfigManager;
use platform::{hotkey, tray};
use utils::generate_tray_icon;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Launchpad starting...");

    let config_manager = ConfigManager::new()?;
    let config = config_manager.load()?;
    log::info!("Configuration loaded ({} items)", config.items.len());

    let (hotkey_tx, hotkey_rx) = crossbeam::channel::unbounded::<()>();
    let (tray_tx, tray_rx) = crossbeam::channel::unbounded::<tray::TrayEvent>();

    // System tray
    let tray_icon = generate_tray_icon();
    let _tray = tray::create_tray(tray_icon, tray_tx)?;
    log::info!("System tray created");

    // Global hotkey
    match hotkey::register_hotkey(hotkey_tx) {
        Ok(mgr) => {
            std::mem::forget(mgr);
            log::info!("Global hotkey registered (Ctrl+Alt+R)");
        }
        Err(e) => log::warn!("Hotkey unavailable: {}", e),
    }

    // Window: always visible, always on top, frameless
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Launchpad")
            .with_inner_size([640.0, 480.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(true),
        ..Default::default()
    };

    let app = LaunchpadApp::new(hotkey_rx, tray_rx, config, config_manager);

    log::info!("Starting egui...");
    eframe::run_native(
        "Launchpad",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    log::info!("Launchpad exited");
    Ok(())
}
