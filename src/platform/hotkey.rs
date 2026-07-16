use crossbeam::channel::Sender;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};

/// Parse a hotkey string like "Ctrl+Alt+R" into a HotKey.
/// Returns None if the format is invalid.
pub fn parse_hotkey(s: &str) -> Option<HotKey> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.len() < 2 {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    let mut code: Option<Code> = None;

    for part in &parts {
        match *part {
            "Ctrl" | "Control" => modifiers |= Modifiers::CONTROL,
            "Alt" => modifiers |= Modifiers::ALT,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Win" | "Super" | "Windows" => modifiers |= Modifiers::SUPER,
            other => {
                // Try to parse as a key code
                code = parse_key(other);
            }
        }
    }

    code.map(|c| HotKey::new(Some(modifiers), c))
}

fn parse_key(s: &str) -> Option<Code> {
    match s.to_uppercase().as_str() {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        "SPACE" => Some(Code::Space),
        "TAB" => Some(Code::Tab),
        "ESC" | "ESCAPE" => Some(Code::Escape),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "BACKSPACE" => Some(Code::Backspace),
        "DELETE" => Some(Code::Delete),
        "UP" => Some(Code::ArrowUp),
        "DOWN" => Some(Code::ArrowDown),
        "LEFT" => Some(Code::ArrowLeft),
        "RIGHT" => Some(Code::ArrowRight),
        "HOME" => Some(Code::Home),
        "END" => Some(Code::End),
        "PAGEUP" => Some(Code::PageUp),
        "PAGEDOWN" => Some(Code::PageDown),
        "PRINTSCREEN" => Some(Code::PrintScreen),
        "SCROLLLOCK" => Some(Code::ScrollLock),
        "PAUSE" => Some(Code::Pause),
        "INSERT" => Some(Code::Insert),
        "CAPSLOCK" => Some(Code::CapsLock),
        "NUMLOCK" => Some(Code::NumLock),
        "NUM0" | "NUMPAD0" => Some(Code::Numpad0),
        "NUM1" | "NUMPAD1" => Some(Code::Numpad1),
        "NUM2" | "NUMPAD2" => Some(Code::Numpad2),
        "NUM3" | "NUMPAD3" => Some(Code::Numpad3),
        "NUM4" | "NUMPAD4" => Some(Code::Numpad4),
        "NUM5" | "NUMPAD5" => Some(Code::Numpad5),
        "NUM6" | "NUMPAD6" => Some(Code::Numpad6),
        "NUM7" | "NUMPAD7" => Some(Code::Numpad7),
        "NUM8" | "NUMPAD8" => Some(Code::Numpad8),
        "NUM9" | "NUMPAD9" => Some(Code::Numpad9),
        "NUMADD" => Some(Code::NumpadAdd),
        "NUMSUBTRACT" => Some(Code::NumpadSubtract),
        "NUMMULTIPLY" => Some(Code::NumpadMultiply),
        "NUMDIVIDE" => Some(Code::NumpadDivide),
        "NUMDECIMAL" => Some(Code::NumpadDecimal),
        "NUMENTER" => Some(Code::NumpadEnter),
        _ => None,
    }
}

/// Register the global hotkey.
///
/// `hotkey_str` should be like "Ctrl+Alt+R".
/// Returns the `GlobalHotKeyManager` which must be kept alive.
pub fn register_hotkey(
    tx: Sender<()>,
    hotkey_str: &str,
) -> Result<GlobalHotKeyManager, Box<dyn std::error::Error>> {
    let manager = GlobalHotKeyManager::new()?;

    let hotkey =
        parse_hotkey(hotkey_str).ok_or_else(|| format!("Invalid hotkey string: {}", hotkey_str))?;

    manager.register(hotkey)?;

    let hk = hotkey_str.to_string();
    std::thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        log::info!("Hotkey listener thread started");
        loop {
            if let Ok(event) = receiver.recv() {
                // Only trigger on key release to avoid double-fire from key-down
                if event.state == global_hotkey::HotKeyState::Released {
                    log::info!("Hotkey released!");
                    let _ = tx.send(());
                    crate::app::wake_ui();
                }
            }
        }
    });

    log::info!("Global hotkey registered: {}", hk);
    Ok(manager)
}
