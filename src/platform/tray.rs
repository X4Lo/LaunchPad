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
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let show_hide = MenuItem::new("Show / Hide", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::with_items(&[&show_hide, &quit])?;

    let show_id = show_hide.id().clone();
    let quit_id = quit.id().clone();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("Launchpad — Ctrl+Alt+R to toggle")
        .build()?;

    // Use MenuEvent::set_event_handler for reliable event delivery
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == show_id {
            let _ = tx.send(TrayEvent::Toggle);
        } else if event.id == quit_id {
            let _ = tx.send(TrayEvent::Quit);
        }
    }));

    Ok(tray)
}
