//! The selected row of a list that the arrow keys move through: the palette's results, the agent picker's agents,
//! the completion list's candidates and a menu's items. The window draws the list; this keeps which row is selected
//! as the list grows, shrinks and is filtered, and turns the keys a list answers (Up, Down, Home, End and Enter) into
//! what they do, so every window moves through a list the same way.

use crate::keys::{Chord, Key};

/// A step the keyboard asks of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListStep {
    Up,
    Down,
    First,
    Last,
}

impl ListStep {
    /// The step a key asks for: Up, Down, Home and End with no modifier.
    pub fn of(chord: Chord) -> Option<Self> {
        let mods = chord.mods;
        if mods.ctrl || mods.alt || mods.shift {
            return None;
        }
        match chord.key {
            Key::Up => Some(ListStep::Up),
            Key::Down => Some(ListStep::Down),
            Key::Home => Some(ListStep::First),
            Key::End => Some(ListStep::Last),
            _ => None,
        }
    }
}

/// What a key did to a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKey {
    /// The key moved the selection (or would have, at an end): the list redraws.
    Moved,
    /// Enter picked this row.
    Pick(usize),
    /// Not a key the list answers, or Enter with no row selected: the key goes on to whatever else takes it.
    Ignored,
}

/// Which row of a list of `len` rows is selected, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListSelection {
    len: usize,
    selected: Option<usize>,
}

impl ListSelection {
    /// A list of `len` rows with none selected.
    pub fn new(len: usize) -> Self {
        Self {
            len,
            selected: None,
        }
    }

    /// A list of `len` rows with its first row selected, as a filtered list starts after each keystroke; none when
    /// the list is empty.
    pub fn first_of(len: usize) -> Self {
        Self {
            len,
            selected: (len > 0).then_some(0),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Selects a row; a row past the end selects nothing.
    pub fn select(&mut self, row: usize) {
        self.selected = (row < self.len).then_some(row);
    }

    /// The list now holds `len` rows: a selection past the new end moves to the last row.
    pub fn resize(&mut self, len: usize) {
        self.len = len;
        self.selected = match self.selected {
            Some(_) if len == 0 => None,
            Some(row) => Some(row.min(len - 1)),
            None => None,
        };
    }

    /// Moves the selection one row up or down, or to either end. It stops at the ends rather than wrapping, as the
    /// window's menus do; from no selection, Down and First select the first row and Up and Last the last.
    pub fn step(&mut self, step: ListStep) {
        if self.len == 0 {
            return;
        }
        let last = self.len - 1;
        self.selected = Some(match (step, self.selected) {
            (ListStep::First, _) | (ListStep::Down, None) => 0,
            (ListStep::Last, _) | (ListStep::Up, None) => last,
            (ListStep::Down, Some(row)) => (row + 1).min(last),
            (ListStep::Up, Some(row)) => row.saturating_sub(1),
        });
    }

    /// A key typed while the list has the keyboard: the arrows, Home and End move the selection, and plain Enter picks
    /// the selected row.
    pub fn key(&mut self, chord: Chord) -> ListKey {
        if let Some(step) = ListStep::of(chord) {
            self.step(step);
            return ListKey::Moved;
        }
        let plain = !(chord.mods.ctrl || chord.mods.alt || chord.mods.shift);
        match (chord.key, self.selected) {
            (Key::Enter, Some(row)) if plain => ListKey::Pick(row),
            _ => ListKey::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Mods;

    fn plain(key: Key) -> Chord {
        Chord::new(Mods::NONE, key)
    }

    #[test]
    fn arrows_move_and_enter_picks_the_selected_row() {
        let mut list = ListSelection::new(3);
        assert_eq!(
            list.key(plain(Key::Enter)),
            ListKey::Ignored,
            "nothing selected yet"
        );
        assert_eq!(list.key(plain(Key::Down)), ListKey::Moved);
        assert_eq!(list.key(plain(Key::Down)), ListKey::Moved);
        assert_eq!(list.key(plain(Key::Enter)), ListKey::Pick(1));
        assert_eq!(list.key(plain(Key::End)), ListKey::Moved);
        assert_eq!(list.key(plain(Key::Enter)), ListKey::Pick(2));
        assert_eq!(list.key(plain(Key::Home)), ListKey::Moved);
        assert_eq!(
            list.key(plain(Key::Up)),
            ListKey::Moved,
            "stopped at the first row"
        );
        assert_eq!(list.key(plain(Key::Enter)), ListKey::Pick(0));
    }

    #[test]
    fn modified_keys_and_other_keys_are_left_alone() {
        let mut list = ListSelection::first_of(3);
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        assert_eq!(list.key(Chord::new(shift, Key::Down)), ListKey::Ignored);
        assert_eq!(
            list.key(Chord::new(Mods::CTRL, Key::Enter)),
            ListKey::Ignored
        );
        assert_eq!(list.key(plain(Key::Char('j'))), ListKey::Ignored);
        assert_eq!(list.key(plain(Key::Tab)), ListKey::Ignored);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn down_from_nothing_selects_the_first_row_and_stops_at_the_last() {
        let mut list = ListSelection::new(3);
        assert_eq!(list.selected(), None);
        list.step(ListStep::Down);
        assert_eq!(list.selected(), Some(0));
        list.step(ListStep::Down);
        list.step(ListStep::Down);
        list.step(ListStep::Down);
        assert_eq!(list.selected(), Some(2));
    }

    #[test]
    fn up_from_nothing_selects_the_last_row_and_stops_at_the_first() {
        let mut list = ListSelection::new(3);
        list.step(ListStep::Up);
        assert_eq!(list.selected(), Some(2));
        list.step(ListStep::Up);
        list.step(ListStep::Up);
        list.step(ListStep::Up);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn first_and_last_jump_to_the_ends() {
        let mut list = ListSelection::first_of(5);
        list.step(ListStep::Last);
        assert_eq!(list.selected(), Some(4));
        list.step(ListStep::First);
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn an_empty_list_selects_nothing_whatever_the_step() {
        let mut list = ListSelection::first_of(0);
        assert!(list.is_empty());
        for step in [
            ListStep::Up,
            ListStep::Down,
            ListStep::First,
            ListStep::Last,
        ] {
            list.step(step);
            assert_eq!(list.selected(), None);
        }
    }

    #[test]
    fn filtering_the_list_shorter_keeps_the_selection_inside_it() {
        let mut list = ListSelection::first_of(6);
        list.select(5);
        list.resize(2);
        assert_eq!((list.len(), list.selected()), (2, Some(1)));
        list.resize(0);
        assert_eq!(list.selected(), None);
        list.resize(4);
        assert_eq!(list.selected(), None);
        list.select(4);
        assert_eq!(list.selected(), None, "a row past the end is no row");
    }
}
