use crossbeam::channel::Sender;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::HotKey, hotkey::Modifiers, hotkey::Code};

/// Register the global hotkey `Ctrl+Alt+R`.
///
/// Returns the `GlobalHotKeyManager` which must be kept alive for the
/// hotkey to remain registered. Dropping it unregisters the hotkey.
pub fn register_hotkey(
    tx: Sender<()>,
) -> Result<GlobalHotKeyManager, Box<dyn std::error::Error>> {
    let manager = GlobalHotKeyManager::new()?;

    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);

    manager.register(hotkey)?;

    // Spawn a thread to listen for hotkey events and forward them to the channel.
    // `GlobalHotKeyEvent::receiver()` is a blocking iterator, so we need a dedicated thread.
    std::thread::spawn(move || {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            // This blocks until a hotkey event arrives
            if let Ok(_event) = receiver.recv() {
                // Send a toggle signal; ignore if the channel is disconnected
                let _ = tx.send(());
            }
        }
    });

    Ok(manager)
}
