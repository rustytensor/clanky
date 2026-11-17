//! Key bindings as data: a key type that belongs to no window toolkit, the window-wide actions, and the table that
//! joins them. Each window turns its toolkit's key events into a [`Chord`] and asks [`action_for`]; the command
//! palette and the Settings keys page show the same table through [`Chord::label`]. Whether an action applies at the
//! moment (Copy only with a terminal focused, say) is the window's decision: the table only names the action.

/// The modifier keys held with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        shift: false,
        alt: false,
    };
    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        shift: true,
        alt: false,
    };
}

/// A key, as printed on the keyboard: characters are kept lower case, and Shift is a modifier, so Ctrl+Shift+T is
/// `Char('t')` with Ctrl and Shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1 to F24.
    F(u8),
}

/// A key with its modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

impl Chord {
    pub const fn new(mods: Mods, key: Key) -> Self {
        Self { mods, key }
    }

    /// How the chord is written for people: "Ctrl+Shift+T", "Ctrl+=", "Ctrl+Tab".
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.mods.ctrl {
            parts.push("Ctrl".into());
        }
        if self.mods.alt {
            parts.push("Alt".into());
        }
        if self.mods.shift {
            parts.push("Shift".into());
        }
        parts.push(match self.key {
            Key::Char(ch) => ch.to_uppercase().collect(),
            Key::Enter => "Enter".into(),
            Key::Escape => "Escape".into(),
            Key::Tab => "Tab".into(),
            Key::Backspace => "Backspace".into(),
            Key::Delete => "Delete".into(),
            Key::Up => "Up".into(),
            Key::Down => "Down".into(),
            Key::Left => "Left".into(),
            Key::Right => "Right".into(),
            Key::Home => "Home".into(),
            Key::End => "End".into(),
            Key::PageUp => "Page Up".into(),
            Key::PageDown => "Page Down".into(),
            Key::F(n) => format!("F{n}"),
        });
        parts.join("+")
    }
}

/// The window-wide commands a key can start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Revoke every agent's access and cancel its pending waits.
    StopAgents,
    ToggleSettings,
    ToggleCommandPalette,
    ToggleAgentPicker,
    /// Move the keyboard between the focused pane's block cards.
    CycleCardFocus,
    ToggleToolsPanel,
    OpenCompose,
    FindInScrollback,
    OpenFile,
    NewTab,
    NextTab,
    SplitRight,
    SplitDown,
    ClosePane,
    Copy,
    Paste,
    ToggleAgentAccess,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

/// One row of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub chord: Chord,
    pub action: Action,
}

const fn bind(mods: Mods, key: Key, action: Action) -> Binding {
    Binding {
        chord: Chord::new(mods, key),
        action,
    }
}

/// Today's window-wide bindings. A key that is not here goes to the focused pane's program.
pub const BINDINGS: &[Binding] = &[
    bind(Mods::CTRL_SHIFT, Key::Escape, Action::StopAgents),
    bind(Mods::CTRL, Key::Char(','), Action::ToggleSettings),
    bind(Mods::CTRL_SHIFT, Key::Char('p'), Action::ToggleCommandPalette),
    bind(Mods::CTRL_SHIFT, Key::Char('g'), Action::ToggleAgentPicker),
    bind(Mods::CTRL_SHIFT, Key::F(6), Action::CycleCardFocus),
    bind(Mods::CTRL_SHIFT, Key::Char('b'), Action::ToggleToolsPanel),
    bind(Mods::CTRL_SHIFT, Key::Enter, Action::OpenCompose),
    bind(Mods::CTRL_SHIFT, Key::Char('f'), Action::FindInScrollback),
    bind(Mods::CTRL_SHIFT, Key::Char('o'), Action::OpenFile),
    bind(Mods::CTRL_SHIFT, Key::Char('t'), Action::NewTab),
    bind(Mods::CTRL, Key::Tab, Action::NextTab),
    bind(Mods::CTRL_SHIFT, Key::Char('d'), Action::SplitRight),
    bind(Mods::CTRL_SHIFT, Key::Char('e'), Action::SplitDown),
    bind(Mods::CTRL_SHIFT, Key::Char('w'), Action::ClosePane),
    bind(Mods::CTRL_SHIFT, Key::Char('c'), Action::Copy),
    bind(Mods::CTRL_SHIFT, Key::Char('v'), Action::Paste),
    bind(Mods::CTRL_SHIFT, Key::Char('a'), Action::ToggleAgentAccess),
    bind(Mods::CTRL, Key::Char('='), Action::ZoomIn),
    bind(Mods::CTRL, Key::Char('+'), Action::ZoomIn),
    bind(Mods::CTRL, Key::Char('-'), Action::ZoomOut),
    bind(Mods::CTRL, Key::Char('0'), Action::ZoomReset),
];

/// The action bound to a chord, if any.
pub fn action_for(chord: Chord) -> Option<Action> {
    BINDINGS
        .iter()
        .find(|binding| binding.chord == chord)
        .map(|binding| binding.action)
}

/// The chords that start an action, in table order (Zoom in has two), for the palette's and Settings' labels.
pub fn chords_for(action: Action) -> impl Iterator<Item = Chord> {
    BINDINGS
        .iter()
        .filter(move |binding| binding.action == action)
        .map(|binding| binding.chord)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Today's shortcuts, read off the winit window's key handler (`crates/clanky/src/main.rs`, `App::key`), not
    /// from the table, so a row lost or changed in the table fails here.
    const TODAY: &[(&str, Action)] = &[
        ("Ctrl+Shift+Escape", Action::StopAgents),
        ("Ctrl+,", Action::ToggleSettings),
        ("Ctrl+Shift+P", Action::ToggleCommandPalette),
        ("Ctrl+Shift+G", Action::ToggleAgentPicker),
        ("Ctrl+Shift+F6", Action::CycleCardFocus),
        ("Ctrl+Shift+B", Action::ToggleToolsPanel),
        ("Ctrl+Shift+Enter", Action::OpenCompose),
        ("Ctrl+Shift+F", Action::FindInScrollback),
        ("Ctrl+Shift+O", Action::OpenFile),
        ("Ctrl+Shift+T", Action::NewTab),
        ("Ctrl+Tab", Action::NextTab),
        ("Ctrl+Shift+D", Action::SplitRight),
        ("Ctrl+Shift+E", Action::SplitDown),
        ("Ctrl+Shift+W", Action::ClosePane),
        ("Ctrl+Shift+C", Action::Copy),
        ("Ctrl+Shift+V", Action::Paste),
        ("Ctrl+Shift+A", Action::ToggleAgentAccess),
        ("Ctrl+=", Action::ZoomIn),
        ("Ctrl++", Action::ZoomIn),
        ("Ctrl+-", Action::ZoomOut),
        ("Ctrl+0", Action::ZoomReset),
    ];

    #[test]
    fn the_table_holds_todays_shortcuts_and_nothing_else() {
        let table: Vec<(String, Action)> = BINDINGS
            .iter()
            .map(|binding| (binding.chord.label(), binding.action))
            .collect();
        let today: Vec<(String, Action)> = TODAY
            .iter()
            .map(|(label, action)| ((*label).to_owned(), *action))
            .collect();
        assert_eq!(table, today);
    }

    #[test]
    fn no_chord_is_bound_twice() {
        for (index, binding) in BINDINGS.iter().enumerate() {
            assert!(
                BINDINGS[index + 1..].iter().all(|other| other.chord != binding.chord),
                "{} is bound twice",
                binding.chord.label()
            );
        }
    }

    #[test]
    fn lookups() {
        assert_eq!(
            action_for(Chord::new(Mods::CTRL_SHIFT, Key::Char('t'))),
            Some(Action::NewTab)
        );
        // Shift matters: Ctrl+T is the program's, not New tab.
        assert_eq!(action_for(Chord::new(Mods::CTRL, Key::Char('t'))), None);
        assert_eq!(action_for(Chord::new(Mods::NONE, Key::Char('a'))), None);
        let zoom_in: Vec<String> = chords_for(Action::ZoomIn).map(|chord| chord.label()).collect();
        assert_eq!(zoom_in, ["Ctrl+=", "Ctrl++"]);
    }
}
