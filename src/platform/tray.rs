use crossbeam::channel::Sender;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Toggle,
    Quit,
}

pub fn create_tray(
    icon: tray_icon::Icon,
    tx: Sender<TrayEvent>,
    hotkey: &str,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let show_hide = MenuItem::new("Show / Hide", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::with_items(&[&show_hide, &quit])?;

    let show_id = show_hide.id().clone();
    let quit_id = quit.id().clone();

    let tooltip = format!("Launchpad — {} to toggle", hotkey);
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip(tooltip)
        .build()?;

    // Use MenuEvent::receiver() which gives a dedicated channel.
    // This is more reliable than set_event_handler because it doesn't
    // depend on the platform event loop dispatching global callbacks.
    let receiver = MenuEvent::receiver();
    std::thread::spawn(move || {
        loop {
            if let Ok(event) = receiver.recv() {
                log::debug!("Tray menu event received");
                if event.id == show_id {
                    let _ = tx.send(TrayEvent::Toggle);
                    crate::app::wake_ui();
                } else if event.id == quit_id {
                    // Quit immediately without waiting for an egui frame
                    std::process::exit(0);
                }
            }
        }
    });

    Ok(tray)
}
