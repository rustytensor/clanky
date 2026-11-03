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

pub fn key_bytes(
    key: &Key,
    text: Option<&str>,
    modifiers: ModifiersState,
    composing: bool,
) -> Vec<u8> {
    // Logical characters and fallback text can duplicate IME preedit delivery.
    if composing
        && (matches!(key, Key::Character(_))
            || text.is_some_and(|s| s.chars().any(|ch| !ch.is_control())))
    {
        return Vec::new();
    }
    let mut bytes = match key {
        Key::Named(named) => match named {
            NamedKey::Enter => vec![b'\r'],
            NamedKey::Backspace => vec![127],
            NamedKey::Tab => vec![b'\t'],
            NamedKey::Escape => vec![27],
            NamedKey::ArrowUp => b"\x1b[A".to_vec(),
            NamedKey::ArrowDown => b"\x1b[B".to_vec(),
            NamedKey::ArrowRight => b"\x1b[C".to_vec(),
            NamedKey::ArrowLeft => b"\x1b[D".to_vec(),
            NamedKey::Home => b"\x1b[H".to_vec(),
            NamedKey::End => b"\x1b[F".to_vec(),
            NamedKey::Insert => b"\x1b[2~".to_vec(),
            NamedKey::Delete => b"\x1b[3~".to_vec(),
            NamedKey::PageUp => b"\x1b[5~".to_vec(),
            NamedKey::PageDown => b"\x1b[6~".to_vec(),
            NamedKey::F1 => b"\x1bOP".to_vec(),
            NamedKey::F2 => b"\x1bOQ".to_vec(),
            NamedKey::F3 => b"\x1bOR".to_vec(),
            NamedKey::F4 => b"\x1bOS".to_vec(),
            NamedKey::F5 => b"\x1b[15~".to_vec(),
            NamedKey::F6 => b"\x1b[17~".to_vec(),
            NamedKey::F7 => b"\x1b[18~".to_vec(),
            NamedKey::F8 => b"\x1b[19~".to_vec(),
            NamedKey::F9 => b"\x1b[20~".to_vec(),
            NamedKey::F10 => b"\x1b[21~".to_vec(),
            NamedKey::F11 => b"\x1b[23~".to_vec(),
            NamedKey::F12 => b"\x1b[24~".to_vec(),
            _ => text.unwrap_or("").as_bytes().to_vec(),
        },
        Key::Character(character)
            if modifiers.control_key() && character.len() == 1 && character.is_ascii() =>
        {
            vec![character.as_bytes()[0] & 0x1f]
        }
        Key::Character(character) => text.unwrap_or(character.as_str()).as_bytes().to_vec(),
        _ => text.unwrap_or("").as_bytes().to_vec(),
    };
    if modifiers.alt_key() && !bytes.is_empty() {
        bytes.insert(0, 27);
    }
    bytes
}
