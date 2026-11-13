//! Platform-independent IME state and terminal key encoding.
use winit::{
    event::Ime,
    keyboard::{Key, ModifiersState, NamedKey},
};

#[derive(Default)]
pub struct InputState {
    composing: bool,
}

impl InputState {
    pub fn on_ime(&mut self, event: &Ime) -> Option<Vec<u8>> {
        match event {
            Ime::Preedit(text, _) => {
                self.composing = !text.is_empty();
                None
            }
            Ime::Commit(text) => {
                self.composing = false;
                Some(text.as_bytes().to_vec())
            }
            Ime::Disabled => {
                self.composing = false;
                None
            }
            Ime::Enabled => None,
        }
    }

    pub fn focus_lost(&mut self) {
        self.composing = false;
    }

    pub fn composing(&self) -> bool {
        self.composing
    }
}

/// Bytes a key sends, as xterm (TERM=xterm-256color) encodes them: cursor keys use
/// ESC O x in application cursor mode (DECCKM); modified cursor, editing and function
/// keys use xterm's CSI 1;m x and CSI n;m ~ forms, m = 1 + Shift + 2 Alt + 4 Ctrl.
pub fn key_bytes(
    key: &Key,
    text: Option<&str>,
    modifiers: ModifiersState,
    composing: bool,
    app_cursor: bool,
) -> Vec<u8> {
    // Logical characters and fallback text can duplicate IME preedit delivery.
    if composing
        && (matches!(key, Key::Character(_))
            || text.is_some_and(|s| s.chars().any(|ch| !ch.is_control())))
    {
        return Vec::new();
    }
    let m = 1 + u8::from(modifiers.shift_key()) + 2 * u8::from(modifiers.alt_key()) + 4 * u8::from(modifiers.control_key());
    // Cursor and F1-F4 keys: SS3 x unmodified (cursor keys only in application mode), else CSI 1;m x.
    let letter = |x: u8, ss3: bool| -> Vec<u8> {
        if m > 1 { format!("\x1b[1;{m}{}", x as char).into_bytes() }
        else if ss3 { vec![0x1b, b'O', x] } else { vec![0x1b, b'[', x] }
    };
    let tilde = |n: u8| -> Vec<u8> {
        if m > 1 { format!("\x1b[{n};{m}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() }
    };
    let mut bytes = match key {
        Key::Named(named) => {
            let encoded = match named {
                NamedKey::ArrowUp => Some(letter(b'A', app_cursor)),
                NamedKey::ArrowDown => Some(letter(b'B', app_cursor)),
                NamedKey::ArrowRight => Some(letter(b'C', app_cursor)),
                NamedKey::ArrowLeft => Some(letter(b'D', app_cursor)),
                NamedKey::Home => Some(letter(b'H', app_cursor)),
                NamedKey::End => Some(letter(b'F', app_cursor)),
                NamedKey::F1 => Some(letter(b'P', true)),
                NamedKey::F2 => Some(letter(b'Q', true)),
                NamedKey::F3 => Some(letter(b'R', true)),
                NamedKey::F4 => Some(letter(b'S', true)),
                NamedKey::Insert => Some(tilde(2)),
                NamedKey::Delete => Some(tilde(3)),
                NamedKey::PageUp => Some(tilde(5)),
                NamedKey::PageDown => Some(tilde(6)),
                NamedKey::F5 => Some(tilde(15)),
                NamedKey::F6 => Some(tilde(17)),
                NamedKey::F7 => Some(tilde(18)),
                NamedKey::F8 => Some(tilde(19)),
                NamedKey::F9 => Some(tilde(20)),
                NamedKey::F10 => Some(tilde(21)),
                NamedKey::F11 => Some(tilde(23)),
                NamedKey::F12 => Some(tilde(24)),
                NamedKey::Tab if modifiers.shift_key() => Some(b"\x1b[Z".to_vec()),
                _ => None,
            };
            // Modifiers are already in these sequences; Alt does not add another ESC.
            if let Some(encoded) = encoded { return encoded; }
            match named {
                NamedKey::Enter => vec![b'\r'],
                NamedKey::Backspace if modifiers.control_key() => vec![8],
                NamedKey::Backspace => vec![127],
                NamedKey::Tab => vec![b'\t'],
                NamedKey::Escape => vec![27],
                NamedKey::Space if modifiers.control_key() => vec![0],
                _ => text.unwrap_or("").as_bytes().to_vec(),
            }
        }
        Key::Character(character)
            if modifiers.control_key() && character.len() == 1 && character.is_ascii() =>
        {
            let byte = character.as_bytes()[0];
            match byte {
                b'@' | b' ' | b'2' => vec![0],
                b'[' | b'3' => vec![27],
                b'\\' | b'4' => vec![28],
                b']' | b'5' => vec![29],
                b'^' | b'6' => vec![30],
                b'_' | b'/' | b'7' => vec![31],
                b'8' | b'?' => vec![127],
                _ => vec![byte & 0x1f],
            }
        }
        Key::Character(character) => text.unwrap_or(character.as_str()).as_bytes().to_vec(),
        _ => text.unwrap_or("").as_bytes().to_vec(),
    };
    if modifiers.alt_key() && !bytes.is_empty() {
        bytes.insert(0, 27);
    }
    bytes
}

#[cfg(test)]
#[test]
fn xterm_key_bytes() {
    // Expected bytes from `infocmp -1 xterm-256color` (kcuu1=\EOA in keypad mode, kf6=\E[17~,
    // kri=\E[1;2A, kRIT5=\E[1;5C, kcbt=\E[Z) and xterm's ctlseqs for normal cursor mode.
    let none = ModifiersState::empty();
    let named = |k: NamedKey, m: ModifiersState, app: bool| key_bytes(&Key::Named(k), None, m, false, app);
    assert_eq!(named(NamedKey::ArrowUp, none, false), b"\x1b[A");
    assert_eq!(named(NamedKey::ArrowUp, none, true), b"\x1bOA");
    assert_eq!(named(NamedKey::End, none, true), b"\x1bOF");
    assert_eq!(named(NamedKey::ArrowUp, ModifiersState::SHIFT, true), b"\x1b[1;2A");
    assert_eq!(named(NamedKey::ArrowRight, ModifiersState::CONTROL, false), b"\x1b[1;5C");
    assert_eq!(named(NamedKey::F6, none, false), b"\x1b[17~");
    assert_eq!(named(NamedKey::F5, ModifiersState::CONTROL, false), b"\x1b[15;5~");
    assert_eq!(named(NamedKey::Tab, ModifiersState::SHIFT, false), b"\x1b[Z");
    assert_eq!(key_bytes(&Key::Character("x".into()), Some("x"), ModifiersState::ALT, false, false), b"\x1bx");
}
