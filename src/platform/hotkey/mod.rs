//! The global "show / hide Launchpad" shortcut.
//!
//! Uses `global-hotkey` (`RegisterHotKey` on Windows). The manager must live on the main
//! (event-loop) thread; the press handler forwards to the UI with
//! `slint::invoke_from_event_loop`. Re-registering is instant, so changing the shortcut
//! in Settings no longer needs a restart.

mod accelerator;

pub use accelerator::{capture_from_key_event, Accelerator, Capture};

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub struct Hotkey {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
    id: Arc<AtomicU32>,
    on_release: Arc<AtomicBool>,
}

impl Hotkey {
    /// `on_fire` runs on whatever thread the OS delivers the event on.
    pub fn new(on_fire: impl Fn() + Send + Sync + 'static) -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let id = Arc::new(AtomicU32::new(0));
        let on_release = Arc::new(AtomicBool::new(true));
        let (want_id, want_release) = (id.clone(), on_release.clone());
        GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
            if ev.id() != want_id.load(Ordering::Relaxed) {
                return;
            }
            let wanted =
                if want_release.load(Ordering::Relaxed) { HotKeyState::Released } else { HotKeyState::Pressed };
            if ev.state() == wanted {
                on_fire();
            }
        }));
        Ok(Self { manager, current: None, id, on_release })
    }

    /// Register `text` (e.g. "Ctrl+Alt+R"), replacing the previous shortcut.
    pub fn apply(&mut self, text: &str, on_release: bool) -> Result<(), String> {
        self.suspend();
        self.on_release.store(on_release, Ordering::Relaxed);
        if text.trim().is_empty() {
            return Ok(());
        }
        let acc: Accelerator = text.parse().map_err(|e| format!("{e}"))?;
        let hk = acc.to_hotkey();
        self.manager.register(hk).map_err(|e| {
            log::warn!("register {text} failed: {e}");
            format!("{} is already used by another program", acc.label())
        })?;
        self.id.store(hk.id(), Ordering::Relaxed);
        self.current = Some(hk);
        log::info!("Global hotkey registered: {text}");
        Ok(())
    }

    /// Unregister (while recording a new shortcut, so the combo reaches our window).
    pub fn suspend(&mut self) {
        if let Some(hk) = self.current.take() {
            let _ = self.manager.unregister(hk);
        }
        self.id.store(0, Ordering::Relaxed);
    }
}

impl Drop for Hotkey {
    fn drop(&mut self) {
        self.suspend();
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
    }
}

/// Human-readable label for a stored shortcut ("Ctrl + Alt + R"), or the raw text.
pub fn label(text: &str) -> String {
    text.parse::<Accelerator>().map(|a| a.label()).unwrap_or_else(|_| text.to_owned())
}
