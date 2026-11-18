//! System tray icon. Created on the main thread once the event loop runs; events are
//! forwarded to the UI thread with `slint::invoke_from_event_loop`.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const MENU_TOGGLE: &str = "toggle";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    Toggle,
    Settings,
    Quit,
}

pub struct Tray {
    tray: TrayIcon,
}

impl Tray {
    /// `on_command` is invoked on the Slint event loop thread.
    pub fn new(
        hotkey_label: &str,
        on_command: impl Fn(TrayCommand) + Send + Sync + Clone + 'static,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let toggle = MenuItem::with_id(MENU_TOGGLE, "Show / hide Launchpad", true, None);
        let settings = MenuItem::with_id(MENU_SETTINGS, "Settings…", true, None);
        let quit = MenuItem::with_id(MENU_QUIT, "Quit Launchpad", true, None);
        menu.append_items(&[&toggle, &settings, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;

        let size = 32;
        let icon = Icon::from_rgba(crate::ui::icon::render(size), size, size).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(tooltip(hotkey_label))
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        let cmd = on_command.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let c = match e.id.0.as_str() {
                MENU_TOGGLE => TrayCommand::Toggle,
                MENU_SETTINGS => TrayCommand::Settings,
                MENU_QUIT => TrayCommand::Quit,
                _ => return,
            };
            let cmd = cmd.clone();
            let _ = slint::invoke_from_event_loop(move || cmd(c));
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if matches!(e, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                let cmd = on_command.clone();
                let _ = slint::invoke_from_event_loop(move || cmd(TrayCommand::Toggle));
            }
        }));
        Ok(Self { tray })
    }

    pub fn set_hotkey_label(&self, hotkey_label: &str) {
        let _ = self.tray.set_tooltip(Some(tooltip(hotkey_label)));
    }
}

fn tooltip(hotkey_label: &str) -> String {
    if hotkey_label.is_empty() {
        "Launchpad".into()
    } else {
        format!("Launchpad — {hotkey_label}")
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
    }
}
