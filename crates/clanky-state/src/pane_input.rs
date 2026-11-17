//! One pane's command input: what its keys do to the text, when it steps aside for the program, what Enter sends,
//! Tab completion, and the history it learns from the pane's finished blocks. The window turns its key events into
//! a [`Chord`], sends the bytes a [`Step`] names, and draws the input from [`PaneInput::view`].

use std::path::Path;

use serde_json::Value;

use crate::command_input::{self, Candidate, Highlighter, History, LineEdit, Tok, Word};
use crate::keys::{Chord, Key};

/// Why the input steps aside: keys go to the program, and the input says so, dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aside {
    Running,
    FullScreen,
    NoMarks,
}

impl Aside {
    /// Where keys go, in the mockup's words.
    pub fn text(self) -> &'static str {
        match self {
            Aside::Running => "A command is running: keys go to it. Ctrl+C stops it.",
            Aside::FullScreen => "A full-screen program has the keys.",
            Aside::NoMarks => "No shell marks: keys go to the shell.",
        }
    }
}

/// What a key asks of the window, beyond the change to the text the input already made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Not the input's key: it goes to the program.
    Ignored,
    /// Taken; draw the input again.
    Done,
    /// Enter: send the text ([`PaneInput::submit`]).
    Submit,
    /// Tab: complete the word before the caret ([`PaneInput::complete`]).
    Complete,
    /// These bytes go straight to the shell (Ctrl+C, Ctrl+D or Ctrl+L on an empty line).
    Shell(&'static [u8]),
}

/// The completion list: the word it replaces, its candidates and the highlighted one.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    pub word: Word,
    pub items: Vec<Candidate>,
    pub selected: usize,
}

/// One pane's input.
#[derive(Clone, Debug, Default)]
pub struct PaneInput {
    edit: LineEdit,
    /// The prompt (its mark's number) a command was last sent at: the input waits for the next one.
    sent_seq: u64,
    /// History recall: the text typed before the first Up, and the entry shown.
    recall: Option<(String, Option<usize>)>,
    popup: Option<Popup>,
    /// Escape hid the ghost text until the next edit.
    ghost_off: bool,
    notice: Option<String>,
}

/// One character of the text as drawn: the character, its bash colour role, and whether it is selected.
pub type Shown = (char, Tok, bool);

/// The input as the window draws it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub lines: Vec<Vec<Shown>>,
    /// The caret's line and column (in characters).
    pub caret: (usize, usize),
    /// The rest of a history entry the text starts, drawn in grey after the caret.
    pub ghost: String,
    pub notice: Option<String>,
    /// The completion list's items (shown text and kind), the highlighted one, and the column of the word.
    pub popup: Option<(Vec<(String, &'static str)>, usize, usize)>,
}

/// Why a pane's input steps aside now, from the prompt state the server sends with each snapshot (`prompt` of the
/// block view), or None when it takes keys.
pub fn aside(prompt: &Value, alternate_screen: bool, sent_seq: u64) -> Option<Aside> {
    if prompt["integrated"] != true {
        return Some(Aside::NoMarks);
    }
    if alternate_screen {
        return Some(Aside::FullScreen);
    }
    let seq = prompt["seq"].as_u64().unwrap_or(0);
    if prompt["at_prompt"] == true && seq > sent_seq {
        None
    } else {
        Some(Aside::Running)
    }
}

/// Commands the pane's finished blocks ran join the history once each (`seen` holds the block ids already taken).
/// Masked commands (a run of stars) are left out: they would run as literal stars.
pub fn learn(history: &mut History, seen: &mut std::collections::BTreeSet<String>, view: &Value) {
    for block in view["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["complete"] == true)
    {
        let (Some(id), Some(command)) = (block["block_id"].as_str(), block["command"].as_str())
        else {
            continue;
        };
        if seen.insert(id.to_owned()) && !command.contains("****") {
            history.push(command);
        }
    }
}

impl PaneInput {
    pub fn sent_seq(&self) -> u64 {
        self.sent_seq
    }

    pub fn is_empty(&self) -> bool {
        self.edit.is_empty()
    }

    fn edited(&mut self) {
        self.recall = None;
        self.ghost_off = false;
    }

    /// A key while the input takes keys. `text` is what the key types, if anything.
    pub fn key(&mut self, chord: Chord, text: Option<&str>, history: &History) -> Step {
        let (ctrl, shift, alt) = (chord.mods.ctrl, chord.mods.shift, chord.mods.alt);
        self.notice = None;
        // The completion list takes Tab, Enter, Up, Down and Escape; any other key closes it first.
        if let Some(popup) = self.popup.take() {
            let n = popup.items.len().max(1);
            match chord.key {
                Key::Tab | Key::Enter if !ctrl && !alt => {
                    if let Some(item) = popup.items.get(popup.selected) {
                        self.edit
                            .replace(popup.word.start..popup.word.end, &item.insert);
                    }
                    return if chord.key == Key::Tab {
                        Step::Done
                    } else {
                        Step::Submit
                    };
                }
                Key::Down => {
                    self.popup = Some(Popup {
                        selected: (popup.selected + 1) % n,
                        ..popup
                    });
                    return Step::Done;
                }
                Key::Up => {
                    self.popup = Some(Popup {
                        selected: (popup.selected + n - 1) % n,
                        ..popup
                    });
                    return Step::Done;
                }
                Key::Escape => return Step::Done,
                _ => {}
            }
        }
        if ctrl {
            return self.control_key(chord);
        }
        match chord.key {
            Key::Enter if shift => {
                self.edit.insert("\n");
                self.edited();
            }
            Key::Enter => return Step::Submit,
            Key::Tab if !shift => return Step::Complete,
            Key::Escape => self.ghost_off = true,
            Key::Backspace => {
                self.edit.backspace();
                self.edited();
            }
            Key::Delete => {
                self.edit.delete();
                self.edited();
            }
            Key::Left => self.edit.left(shift),
            Key::Home => self.edit.home(shift),
            Key::Right | Key::End => {
                let ghost = (!shift && self.edit.at_end() && !self.ghost_off)
                    .then(|| history.ghost(self.edit.text()).map(str::to_owned))
                    .flatten();
                if let Some(rest) = ghost {
                    self.edit.insert(&rest);
                    self.edited();
                } else if chord.key == Key::End {
                    self.edit.end(shift);
                } else {
                    self.edit.right(shift);
                }
            }
            Key::Up | Key::Down => self.recall_step(chord.key == Key::Down, shift, history),
            Key::PageUp | Key::PageDown | Key::F(_) | Key::Tab => {}
            Key::Char(_) if alt => {}
            Key::Char(_) => {
                if let Some(text) =
                    text.filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
                {
                    self.edit.insert(text);
                    self.edited();
                }
            }
        }
        Step::Done
    }

    fn control_key(&mut self, chord: Chord) -> Step {
        let shift = chord.mods.shift;
        match chord.key {
            Key::Enter if !shift => return Step::Submit,
            Key::Char('z') => {
                if shift {
                    self.edit.redo();
                } else {
                    self.edit.undo();
                }
                self.edited();
            }
            Key::Char('y') => {
                self.edit.redo();
                self.edited();
            }
            Key::Char('a') => self.edit.home(shift),
            Key::Char('e') => self.edit.end(shift),
            Key::Char('u') => {
                let end = self.edit.cursor();
                let start = self.edit.offset(self.edit.caret().0, 0);
                self.edit.replace(start..end, "");
                self.edited();
            }
            Key::Char('k') => {
                let start = self.edit.cursor();
                let end = self.edit.offset(self.edit.caret().0, usize::MAX);
                self.edit.replace(start..end, "");
                self.edited();
            }
            // Ctrl+C clears the text; on an empty line it goes to the shell, as in a terminal.
            Key::Char('c') if !self.edit.is_empty() => {
                self.edit.clear();
                self.edited();
            }
            Key::Char('c') => return Step::Shell(b"\x03"),
            Key::Char('d') if self.edit.is_empty() => return Step::Shell(b"\x04"),
            Key::Char('d') => {
                self.edit.delete();
                self.edited();
            }
            Key::Char('l') => return Step::Shell(b"\x0c"),
            _ => {}
        }
        Step::Done
    }

    fn recall_step(&mut self, down: bool, shift: bool, history: &History) {
        if self.edit.vertical(down, shift) || shift {
            return;
        }
        if self.recall.is_none() && down {
            return;
        }
        let (prefix, from) = self
            .recall
            .clone()
            .unwrap_or_else(|| (self.edit.text().to_owned(), None));
        match history.step(&prefix, from, !down) {
            Some(i) => {
                self.edit.set(&history.entries()[i]);
                self.recall = Some((prefix, Some(i)));
            }
            None if down => {
                self.edit.set(&prefix);
                self.recall = None;
            }
            None => {}
        }
    }

    /// Text from the input method or a paste: line breaks kept, other control characters as spaces.
    pub fn insert_text(&mut self, text: &str) {
        let clean: String = text
            .replace("\r\n", "\n")
            .chars()
            .map(|c| match c {
                '\r' => '\n',
                c if c.is_control() && c != '\n' && c != '\t' => ' ',
                c => c,
            })
            .collect();
        self.popup = None;
        self.edit.insert(&clean);
        self.edited();
    }

    /// Enter: the bytes to send (one bracketed paste and a carriage return), or None for blank text or text that
    /// cannot be sent (the notice says why). `seq` is the prompt the command goes to.
    pub fn submit(&mut self, bracketed: bool, seq: u64, history: &mut History) -> Option<Vec<u8>> {
        let text = self.edit.text().to_owned();
        if text.trim().is_empty() {
            return None;
        }
        match command_input::submit_bytes(&text, bracketed) {
            Ok(bytes) => {
                self.sent_seq = seq;
                self.edit = LineEdit::default();
                self.edited();
                history.push(&text);
                Some(bytes)
            }
            Err(why) => {
                self.notice = Some(why);
                None
            }
        }
    }

    /// Tab: one candidate goes in at once; more open the list. `cwd` is the shell's folder.
    pub fn complete(&mut self, cwd: &Path, home: Option<&Path>, path: Option<&std::ffi::OsStr>) {
        let (lo, hi) = self.edit.selection();
        if lo != hi {
            return;
        }
        let (word, items) = command_input::complete(self.edit.text(), hi, cwd, home, path);
        match items.as_slice() {
            [] => {}
            [only] => {
                self.edit.replace(word.start..word.end, &only.insert);
                self.recall = None;
            }
            _ => {
                self.popup = Some(Popup {
                    word,
                    items,
                    selected: 0,
                })
            }
        }
    }

    /// A click or drag in the text: the caret to that line and column.
    pub fn click(&mut self, line: usize, col: usize, extend: bool) {
        self.popup = None;
        self.edit.click(line, col, extend);
    }

    /// The input as drawn. `highlighter` colours bash; without it the text is plain.
    pub fn view(&self, history: &History, highlighter: Option<&mut Highlighter>) -> View {
        let text = self.edit.text();
        let toks = highlighter.map_or_else(|| vec![Tok::Plain; text.len()], |h| h.tokens(text));
        let (lo, hi) = self.edit.selection();
        let lines = self
            .edit
            .lines()
            .into_iter()
            .map(|(start, end)| {
                text[start..end]
                    .char_indices()
                    .map(|(i, ch)| {
                        (
                            ch,
                            toks.get(start + i).copied().unwrap_or(Tok::Plain),
                            start + i >= lo && start + i < hi,
                        )
                    })
                    .collect()
            })
            .collect();
        let caret = self.edit.caret();
        let ghost = (self.popup.is_none() && !self.ghost_off && self.edit.at_end())
            .then(|| history.ghost(text).map(str::to_owned))
            .flatten()
            .unwrap_or_default();
        let popup = self.popup.as_ref().map(|popup| {
            (
                popup
                    .items
                    .iter()
                    .map(|c| (c.shown.clone(), c.kind.label()))
                    .collect(),
                popup.selected,
                self.edit.line_col(popup.word.start).1,
            )
        });
        View {
            lines,
            caret,
            ghost,
            notice: self.notice.clone(),
            popup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Mods;
    use serde_json::json;

    fn press(input: &mut PaneInput, mods: Mods, key: Key, history: &History) -> Step {
        input.key(Chord::new(mods, key), None, history)
    }

    fn typed(input: &mut PaneInput, text: &str, history: &History) {
        for ch in text.chars() {
            input.key(
                Chord::new(Mods::NONE, Key::Char(ch)),
                Some(&ch.to_string()),
                history,
            );
        }
    }

    #[test]
    fn the_input_steps_aside_until_the_next_prompt() {
        let at = |seq| json!({"integrated": true, "seq": seq, "at_prompt": true});
        assert_eq!(
            aside(&json!({"integrated": false}), false, 0),
            Some(Aside::NoMarks)
        );
        assert_eq!(aside(&at(3), true, 0), Some(Aside::FullScreen));
        assert_eq!(aside(&at(3), false, 3), Some(Aside::Running));
        assert_eq!(aside(&at(4), false, 3), None);
        assert_eq!(
            aside(
                &json!({"integrated": true, "seq": 4, "at_prompt": false}),
                false,
                3
            ),
            Some(Aside::Running)
        );
    }

    /// Enter sends the text as a bracketed paste and a carriage return, and the input waits for the next prompt.
    #[test]
    fn enter_sends_the_text_and_remembers_it() {
        let mut history = History::default();
        let mut input = PaneInput::default();
        typed(&mut input, "ls -la", &history);
        assert_eq!(
            press(&mut input, Mods::NONE, Key::Enter, &history),
            Step::Submit
        );
        assert_eq!(
            input.submit(true, 7, &mut history),
            Some(b"\x1b[200~ls -la\x1b[201~\r".to_vec())
        );
        assert_eq!((input.is_empty(), input.sent_seq()), (true, 7));
        assert_eq!(history.entries(), ["ls -la"]);
        assert_eq!(input.submit(true, 8, &mut history), None);
    }

    #[test]
    fn ghost_text_and_recall_come_from_history() {
        let mut history = History::default();
        history.push("cargo build --release");
        history.push("cargo test");
        let mut input = PaneInput::default();
        typed(&mut input, "cargo b", &history);
        assert_eq!(input.view(&history, None).ghost, "uild --release");
        press(&mut input, Mods::NONE, Key::Right, &history);
        assert_eq!(
            input.view(&history, None).lines[0]
                .iter()
                .map(|c| c.0)
                .collect::<String>(),
            "cargo build --release"
        );
        let mut fresh = PaneInput::default();
        press(&mut fresh, Mods::NONE, Key::Up, &history);
        assert_eq!(
            fresh.view(&history, None).lines[0]
                .iter()
                .map(|c| c.0)
                .collect::<String>(),
            "cargo test"
        );
    }

    #[test]
    fn control_keys_on_an_empty_line_go_to_the_shell() {
        let history = History::default();
        let mut input = PaneInput::default();
        assert_eq!(
            press(&mut input, Mods::CTRL, Key::Char('c'), &history),
            Step::Shell(b"\x03")
        );
        typed(&mut input, "x", &history);
        assert_eq!(
            press(&mut input, Mods::CTRL, Key::Char('c'), &history),
            Step::Done
        );
        assert!(input.is_empty());
        assert_eq!(
            press(&mut input, Mods::CTRL, Key::Char('d'), &history),
            Step::Shell(b"\x04")
        );
    }

    #[test]
    fn finished_blocks_teach_the_history_once_and_never_masked_text() {
        let mut history = History::default();
        let mut seen = std::collections::BTreeSet::new();
        let view = json!({"blocks": [
            {"block_id": "a", "command": "make", "complete": true},
            {"block_id": "b", "command": "export TOKEN=********", "complete": true},
            {"block_id": "c", "command": "sleep 9", "complete": false}
        ]});
        learn(&mut history, &mut seen, &view);
        history.push("ls");
        learn(&mut history, &mut seen, &view);
        assert_eq!(history.entries(), ["make", "ls"]);
    }
}
