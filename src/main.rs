// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Launchpad — a fast, lightweight app launcher.
//!
//! Module map:
//! - `models`    the item tree (apps, groups, folders)
//! - `commands`  mutations of the item tree
//! - `config`    JSON persistence, portable mode
//! - `platform`  hotkey, tray, autostart, single instance, shell (launching + icons)
//! - `ui`        app icon, themes, Slint helpers (markup lives in `ui/*.slint`)
//! - `app`       main-thread controller wiring it all together

mod app;
mod commands;
mod config;
mod models;
mod platform;
mod ui;

slint::include_modules!();

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("launchpad=info")).init();
    log::info!("Launchpad {} starting", env!("CARGO_PKG_VERSION"));

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Launchpad {}\n\n  {}   start in the tray",
            env!("CARGO_PKG_VERSION"),
            platform::autostart::MINIMIZED_ARG
        );
        return;
    }

    let listener = match platform::instance::acquire() {
        platform::instance::Instance::AlreadyRunning => {
            log::info!("Launchpad is already running; asked it to show its window");
            return;
        }
        platform::instance::Instance::Primary(l) => Some(l),
        platform::instance::Instance::Unguarded => {
            log::warn!("single-instance port busy; continuing without the guard");
            None
        }
    };

    let backend = slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into()).select();
    if let Err(e) = backend {
        eprintln!("Launchpad: cannot initialise the UI backend: {e}");
        std::process::exit(1);
    }

    let start_hidden = args.iter().any(|a| a == platform::autostart::MINIMIZED_ARG);
    if let Err(e) = app::App::start(start_hidden) {
        eprintln!("Launchpad: {e}");
        std::process::exit(1);
    }
    if let Some(l) = listener {
        platform::instance::listen(l, || {
            let _ = slint::invoke_from_event_loop(|| {
                app::with_app(|a| a.show_window());
            });
        });
    }

    // Keep running with the window hidden: the tray and hotkey live on.
    if let Err(e) = slint::run_event_loop_until_quit() {
        log::error!("event loop: {e}");
    }
    app::App::shutdown();
    log::info!("Launchpad exited");
}
