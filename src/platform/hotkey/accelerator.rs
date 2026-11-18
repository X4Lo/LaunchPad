//! Platform-neutral keyboard shortcut ("accelerator") type.
//!
//! Stored in config as a readable string such as `"Ctrl+Alt+S"` or `"Ctrl+Shift+F9"`.
//! Keys are *physical* key codes (US layout positions), which is what the OS hotkey
//! APIs register — so `Ctrl+Z` on AZERTY is the key physically labelled `W`.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Accelerator {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Windows key / Command / Super.
    pub meta: bool,
    pub code: Code,
}

impl Accelerator {
    pub fn new(ctrl: bool, alt: bool, shift: bool, meta: bool, code: Code) -> Self {
        Self { ctrl, alt, shift, meta, code }
    }

    pub fn to_hotkey(self) -> HotKey {
        let mut mods = Modifiers::empty();
        if self.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if self.alt {
            mods |= Modifiers::ALT;
        }
        if self.shift {
            mods |= Modifiers::SHIFT;
        }
        if self.meta {
            mods |= Modifiers::META;
        }
        HotKey::new((!mods.is_empty()).then_some(mods), self.code)
    }

    /// Label for the UI, using the platform's name for the meta key.
    pub fn label(&self) -> String {
        let meta = if cfg!(target_os = "macos") {
            "Cmd"
        } else if cfg!(target_os = "windows") {
            "Win"
        } else {
            "Super"
        };
        let alt = if cfg!(target_os = "macos") { "Option" } else { "Alt" };
        let mut parts: Vec<&str> = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push(alt);
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.meta {
            parts.push(meta);
        }
        let key = key_label(self.code);
        parts.push(&key);
        parts.join(" + ")
    }

    fn has_strong_modifier(&self) -> bool {
        self.ctrl || self.alt || self.meta
    }
}

/// Canonical storage form: `Ctrl+Alt+Shift+Meta+<Key>`.
impl fmt::Display for Accelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.meta {
            f.write_str("Meta+")?;
        }
        f.write_str(&key_label(self.code))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid shortcut: {}", self.0)
    }
}

impl FromStr for Accelerator {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut acc = Accelerator::new(false, false, false, false, Code::Unidentified);
        let mut key = None;
        for raw in s.split('+').map(str::trim) {
            match raw.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => acc.ctrl = true,
                "alt" | "option" => acc.alt = true,
                "shift" => acc.shift = true,
                "meta" | "super" | "win" | "cmd" | "command" => acc.meta = true,
                "" => return Err(ParseError(s.to_owned())),
                _ => {
                    if key.is_some() {
                        return Err(ParseError(s.to_owned()));
                    }
                    key = Some(parse_key(raw).ok_or_else(|| ParseError(s.to_owned()))?);
                }
            }
        }
        acc.code = key.ok_or_else(|| ParseError(s.to_owned()))?;
        Ok(acc)
    }
}

fn parse_key(s: &str) -> Option<Code> {
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return Code::from_str(&format!("Key{}", c.to_ascii_uppercase())).ok();
        }
        if c.is_ascii_digit() {
            return Code::from_str(&format!("Digit{c}")).ok();
        }
    }
    match s.to_ascii_lowercase().as_str() {
        "up" => return Some(Code::ArrowUp),
        "down" => return Some(Code::ArrowDown),
        "left" => return Some(Code::ArrowLeft),
        "right" => return Some(Code::ArrowRight),
        "esc" => return Some(Code::Escape),
        "pgup" => return Some(Code::PageUp),
        "pgdn" => return Some(Code::PageDown),
        "enter" | "return" => return Some(Code::Enter),
        "escape" => return Some(Code::Escape),
        "space" => return Some(Code::Space),
        _ => {}
    }
    // "Num5", and the older "NumAdd" / "NUMADD" spellings.
    if let Some(rest) = s.get(..3).filter(|p| p.eq_ignore_ascii_case("num")).map(|_| &s[3..]) {
        let mut cs = rest.chars();
        let rest = match cs.next() {
            Some(c) => c.to_ascii_uppercase().to_string() + &cs.as_str().to_ascii_lowercase(),
            None => String::new(),
        };
        if let Ok(code) = Code::from_str(&format!("Numpad{rest}")) {
            return Some(code);
        }
    }
    Code::from_str(s).ok().filter(|c| *c != Code::Unidentified)
}

/// Short human label for a key code: `KeyS` → `S`, `Digit1` → `1`, `ArrowUp` → `Up`.
pub fn key_label(code: Code) -> String {
    let name = code.to_string();
    if let Some(rest) = name.strip_prefix("Key").filter(|r| r.len() == 1) {
        return rest.to_owned();
    }
    if let Some(rest) = name.strip_prefix("Digit").filter(|r| r.len() == 1) {
        return rest.to_owned();
    }
    if let Some(rest) = name.strip_prefix("Arrow") {
        return rest.to_owned();
    }
    if let Some(rest) = name.strip_prefix("Numpad").filter(|r| r.len() == 1) {
        return format!("Num{rest}");
    }
    name
}

/// Outcome of a key press while the UI is in "press a shortcut" mode.
#[derive(Debug, PartialEq, Eq)]
pub enum Capture {
    Captured(Accelerator),
    /// Escape: leave the binding unchanged.
    Cancel,
    /// Backspace / Delete with no modifiers: remove the binding.
    Clear,
    /// A bare modifier press — keep waiting for the real key.
    Pending,
    Invalid(&'static str),
}

/// Translate a Slint `KeyEvent` (its `text` plus modifier flags) into a capture result.
pub fn capture_from_key_event(text: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Capture {
    use slint::platform::Key;

    let mut chars = text.chars();
    let Some(c) = chars.next() else { return Capture::Pending };
    if chars.next().is_some() {
        return Capture::Invalid("That key can't be used as a shortcut");
    }
    let no_mods = !(ctrl || alt || shift || meta);

    let is = |k: Key| c == char::from(k);
    if is(Key::Escape) {
        return Capture::Cancel;
    }
    if (is(Key::Backspace) || is(Key::Delete)) && no_mods {
        return Capture::Clear;
    }
    let modifier_keys = [
        Key::Shift,
        Key::ShiftR,
        Key::Control,
        Key::ControlR,
        Key::Alt,
        Key::AltGr,
        Key::Meta,
        Key::MetaR,
        Key::CapsLock,
    ];
    if modifier_keys.iter().any(|k| is(*k)) {
        return Capture::Pending;
    }

    let special: &[(Key, Code)] = &[
        (Key::Backspace, Code::Backspace),
        (Key::Delete, Code::Delete),
        (Key::Tab, Code::Tab),
        (Key::Return, Code::Enter),
        (Key::Space, Code::Space),
        (Key::UpArrow, Code::ArrowUp),
        (Key::DownArrow, Code::ArrowDown),
        (Key::LeftArrow, Code::ArrowLeft),
        (Key::RightArrow, Code::ArrowRight),
        (Key::Insert, Code::Insert),
        (Key::Home, Code::Home),
        (Key::End, Code::End),
        (Key::PageUp, Code::PageUp),
        (Key::PageDown, Code::PageDown),
        (Key::Pause, Code::Pause),
        (Key::ScrollLock, Code::ScrollLock),
        (Key::F1, Code::F1),
        (Key::F2, Code::F2),
        (Key::F3, Code::F3),
        (Key::F4, Code::F4),
        (Key::F5, Code::F5),
        (Key::F6, Code::F6),
        (Key::F7, Code::F7),
        (Key::F8, Code::F8),
        (Key::F9, Code::F9),
        (Key::F10, Code::F10),
        (Key::F11, Code::F11),
        (Key::F12, Code::F12),
        (Key::F13, Code::F13),
        (Key::F14, Code::F14),
        (Key::F15, Code::F15),
        (Key::F16, Code::F16),
        (Key::F17, Code::F17),
        (Key::F18, Code::F18),
        (Key::F19, Code::F19),
        (Key::F20, Code::F20),
        (Key::F21, Code::F21),
        (Key::F22, Code::F22),
        (Key::F23, Code::F23),
        (Key::F24, Code::F24),
    ];
    let code = special.iter().find(|(k, _)| is(*k)).map(|(_, code)| *code).or_else(|| printable_to_code(c));

    let Some(code) = code else {
        return Capture::Invalid("That key can't be used as a shortcut");
    };
    let acc = Accelerator::new(ctrl, alt, shift, meta, code);

    // Keys that can stand alone without hijacking normal typing.
    let standalone = matches!(
        code,
        Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
            | Code::F13
            | Code::F14
            | Code::F15
            | Code::F16
            | Code::F17
            | Code::F18
            | Code::F19
            | Code::F20
            | Code::F21
            | Code::F22
            | Code::F23
            | Code::F24
            | Code::Pause
            | Code::ScrollLock
            | Code::Insert
    );
    if !standalone && !acc.has_strong_modifier() {
        return Capture::Invalid("Add Ctrl, Alt or Win/Cmd so normal typing isn't hijacked");
    }
    Capture::Captured(acc)
}

/// Map a printable character (US layout) to its physical key code.
fn printable_to_code(c: char) -> Option<Code> {
    // Note: Slint reports Ctrl+S as text "s" (it reserves ASCII control chars for its
    // own special keys, e.g. U+0013 is AltGr), so no control-char translation is needed.
    if c.is_ascii_alphabetic() || c.is_ascii_digit() {
        return parse_key(&c.to_string());
    }
    Some(match c {
        ' ' => Code::Space,
        '-' | '_' => Code::Minus,
        '=' | '+' => Code::Equal,
        '[' | '{' => Code::BracketLeft,
        ']' | '}' => Code::BracketRight,
        '\\' | '|' => Code::Backslash,
        ';' | ':' => Code::Semicolon,
        '\'' | '"' => Code::Quote,
        ',' | '<' => Code::Comma,
        '.' | '>' => Code::Period,
        '/' | '?' => Code::Slash,
        '`' | '~' => Code::Backquote,
        '!' => Code::Digit1,
        '@' => Code::Digit2,
        '#' => Code::Digit3,
        '$' => Code::Digit4,
        '%' => Code::Digit5,
        '^' => Code::Digit6,
        '&' => Code::Digit7,
        '*' => Code::Digit8,
        '(' => Code::Digit9,
        ')' => Code::Digit0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::platform::Key;

    #[test]
    fn parse_and_display_roundtrip() {
        for s in ["Ctrl+Alt+S", "Ctrl+Shift+F9", "Alt+Meta+1", "F13", "Ctrl+Alt+Up", "Ctrl+Num5", "Ctrl+Alt+Minus"] {
            let acc: Accelerator = s.parse().unwrap();
            assert_eq!(acc.to_string(), s, "roundtrip of {s}");
        }
    }

    #[test]
    fn parse_is_lenient() {
        let a: Accelerator = "control + option + KeyS".parse().unwrap();
        assert_eq!(a, Accelerator::new(true, true, false, false, Code::KeyS));
        let b: Accelerator = "cmd+shift+m".parse().unwrap();
        assert_eq!(b, Accelerator::new(false, false, true, true, Code::KeyM));
    }

    #[test]
    fn parse_legacy_launchpad_spellings() {
        let a: Accelerator = "Ctrl+Alt+R".parse().unwrap();
        assert_eq!(a, Accelerator::new(true, true, false, false, Code::KeyR));
        let b: Accelerator = "Win+Space".parse().unwrap();
        assert_eq!(b, Accelerator::new(false, false, false, true, Code::Space));
        let c: Accelerator = "Ctrl+NUMADD".parse().unwrap();
        assert_eq!(c.code, Code::NumpadAdd);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!("".parse::<Accelerator>().is_err());
        assert!("Ctrl+".parse::<Accelerator>().is_err());
        assert!("Ctrl+Alt".parse::<Accelerator>().is_err());
        assert!("Ctrl+A+B".parse::<Accelerator>().is_err());
        assert!("Ctrl+NotAKey".parse::<Accelerator>().is_err());
    }

    #[test]
    fn capture_letters_need_modifier() {
        assert_eq!(
            capture_from_key_event("s", true, true, false, false),
            Capture::Captured(Accelerator::new(true, true, false, false, Code::KeyS))
        );
        assert!(matches!(capture_from_key_event("s", false, false, false, false), Capture::Invalid(_)));
        assert!(matches!(capture_from_key_event("S", false, false, true, false), Capture::Invalid(_)));
    }

    #[test]
    fn capture_shifted_symbols_map_to_physical_keys() {
        assert_eq!(
            capture_from_key_event("!", true, false, true, false),
            Capture::Captured(Accelerator::new(true, false, true, false, Code::Digit1))
        );
        assert_eq!(
            capture_from_key_event("S", true, false, true, false),
            Capture::Captured(Accelerator::new(true, false, true, false, Code::KeyS))
        );
    }

    #[test]
    fn capture_altgr_is_a_modifier_not_a_key() {
        // U+0013 is Slint's AltGr, not Ctrl+S.
        assert_eq!(capture_from_key_event("\u{13}", true, false, false, false), Capture::Pending);
    }

    #[test]
    fn capture_function_keys_alone() {
        let f9 = char::from(Key::F9).to_string();
        assert_eq!(
            capture_from_key_event(&f9, false, false, false, false),
            Capture::Captured(Accelerator::new(false, false, false, false, Code::F9))
        );
    }

    #[test]
    fn capture_special_actions() {
        assert_eq!(
            capture_from_key_event(&char::from(Key::Escape).to_string(), false, false, false, false),
            Capture::Cancel
        );
        assert_eq!(
            capture_from_key_event(&char::from(Key::Backspace).to_string(), false, false, false, false),
            Capture::Clear
        );
        assert_eq!(
            capture_from_key_event(&char::from(Key::Control).to_string(), true, false, false, false),
            Capture::Pending
        );
        assert_eq!(capture_from_key_event("", false, false, false, false), Capture::Pending);
    }

    #[test]
    fn label_is_pretty() {
        let a: Accelerator = "Ctrl+Shift+Up".parse().unwrap();
        assert_eq!(a.label(), "Ctrl + Shift + Up");
    }
}
