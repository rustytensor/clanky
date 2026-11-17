//! gpui-ce keystrokes to the bytes a terminal program reads. The encoding itself is today's
//! (`clanky::input::key_bytes`, checked against xterm's terminfo); this module only renames the key.

use gpui::Keystroke;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Bytes for a keystroke the pane handles itself, or None when the key is plain text that the input method
/// delivers (so a composed character is never sent twice).
pub fn bytes_for(keystroke: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
    let modifiers = &keystroke.modifiers;
    let mut state = ModifiersState::empty();
    state.set(ModifiersState::SHIFT, modifiers.shift);
    state.set(ModifiersState::ALT, modifiers.alt);
    state.set(ModifiersState::CONTROL, modifiers.control);
    let key = match named(&keystroke.key) {
        Some(named) => Key::Named(named),
        // Text keys go through the input method unless Ctrl or Alt turns them into a control sequence.
        None if !modifiers.control && !modifiers.alt => return None,
        None => Key::Character(keystroke.key.as_str().into()),
    };
    let text = keystroke.key_char.as_deref();
    let bytes = clanky::input::key_bytes(&key, text, state, false, app_cursor);
    (!bytes.is_empty()).then_some(bytes)
}

fn named(key: &str) -> Option<NamedKey> {
    Some(match key {
        "enter" => NamedKey::Enter,
        "backspace" => NamedKey::Backspace,
        "tab" => NamedKey::Tab,
        "escape" => NamedKey::Escape,
        "space" => NamedKey::Space,
        "up" => NamedKey::ArrowUp,
        "down" => NamedKey::ArrowDown,
        "left" => NamedKey::ArrowLeft,
        "right" => NamedKey::ArrowRight,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" => NamedKey::PageUp,
        "pagedown" => NamedKey::PageDown,
        "insert" => NamedKey::Insert,
        "delete" => NamedKey::Delete,
        "f1" => NamedKey::F1,
        "f2" => NamedKey::F2,
        "f3" => NamedKey::F3,
        "f4" => NamedKey::F4,
        "f5" => NamedKey::F5,
        "f6" => NamedKey::F6,
        "f7" => NamedKey::F7,
        "f8" => NamedKey::F8,
        "f9" => NamedKey::F9,
        "f10" => NamedKey::F10,
        "f11" => NamedKey::F11,
        "f12" => NamedKey::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(text: &str) -> Keystroke {
        Keystroke::parse(text).expect("the test keystrokes are valid")
    }

    #[test]
    fn same_bytes_as_xterm() {
        // Expected bytes from `infocmp -1 xterm-256color` and xterm's ctlseqs, as today's encoder's own test.
        assert_eq!(
            bytes_for(&stroke("up"), false).as_deref(),
            Some(&b"\x1b[A"[..])
        );
        assert_eq!(
            bytes_for(&stroke("up"), true).as_deref(),
            Some(&b"\x1bOA"[..])
        );
        assert_eq!(
            bytes_for(&stroke("ctrl-right"), false).as_deref(),
            Some(&b"\x1b[1;5C"[..])
        );
        assert_eq!(
            bytes_for(&stroke("f6"), false).as_deref(),
            Some(&b"\x1b[17~"[..])
        );
        assert_eq!(
            bytes_for(&stroke("shift-tab"), false).as_deref(),
            Some(&b"\x1b[Z"[..])
        );
        assert_eq!(
            bytes_for(&stroke("ctrl-c"), false).as_deref(),
            Some(&b"\x03"[..])
        );
        assert_eq!(
            bytes_for(&stroke("enter"), false).as_deref(),
            Some(&b"\r"[..])
        );
        assert_eq!(
            bytes_for(&stroke("backspace"), false).as_deref(),
            Some(&b"\x7f"[..])
        );
    }

    #[test]
    fn plain_text_is_left_to_the_input_method() {
        assert_eq!(bytes_for(&stroke("a"), false), None);
        assert_eq!(bytes_for(&stroke("shift-a"), false), None);
    }
}
