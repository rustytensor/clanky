//! The pane's command input: the model from `clanky-state`, fed with this pane's keys, input-method text and
//! pastes while it takes keys, and the bytes it asks to send. The main window draws it from [`InputShown`].

use std::{cell::RefCell, collections::BTreeSet, path::PathBuf, rc::Rc};

use clanky_core::{MODE_ALT_SCREEN, MODE_BRACKETED_PASTE};
use clanky_state::{
    command_input::{Highlighter, History},
    keys::Key,
    pane_input::{self, Aside, PaneInput, Step, View},
};
use gpui::{Context, Keystroke};
use serde_json::json;

use super::TerminalPane;
use crate::main_window::chord_of;

/// What every pane's input in a window shares: the history it recalls and suggests from, and the bash colourer.
#[derive(Clone)]
pub struct SharedInput {
    history: Rc<RefCell<History>>,
    highlighter: Rc<RefCell<Option<Highlighter>>>,
}

impl SharedInput {
    pub fn new() -> Self {
        Self {
            history: Rc::default(),
            highlighter: Rc::new(RefCell::new(Highlighter::new())),
        }
    }
}

/// One pane's input and the blocks it has learnt commands from.
pub struct Input {
    model: PaneInput,
    shared: SharedInput,
    seen: BTreeSet<String>,
}

impl Input {
    pub fn new(shared: SharedInput) -> Self {
        Self {
            model: PaneInput::default(),
            shared,
            seen: BTreeSet::new(),
        }
    }
}

/// A pane's input as the main window draws it.
pub struct InputShown {
    pub view: View,
    /// Why the input steps aside, when it does: keys go to the program.
    pub aside: Option<Aside>,
}

impl TerminalPane {
    fn input_aside(&self) -> Option<Aside> {
        let input = self.input.as_ref()?;
        let alternate = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.modes & MODE_ALT_SCREEN != 0);
        pane_input::aside(&self.blocks["prompt"], alternate, input.model.sent_seq())
    }

    /// True while the pane's command input takes its keys (it is shown and not stepped aside).
    pub(super) fn input_takes_keys(&self) -> bool {
        self.input.is_some() && self.input_aside().is_none()
    }

    /// The input as drawn, or None in a pane without one (classic input).
    pub fn input_shown(&self) -> Option<InputShown> {
        let input = self.input.as_ref()?;
        let history = input.shared.history.borrow();
        let mut highlighter = input.shared.highlighter.borrow_mut();
        Some(InputShown {
            view: input.model.view(&history, highlighter.as_mut()),
            aside: self.input_aside(),
        })
    }

    /// Commands the pane's finished blocks ran join the shared history.
    pub(super) fn learn_commands(&mut self) {
        if let Some(input) = self.input.as_mut() {
            pane_input::learn(
                &mut input.shared.history.borrow_mut(),
                &mut input.seen,
                &self.blocks,
            );
        }
    }

    /// A key while the input takes keys. Typed characters are left to the input method, which delivers them as
    /// text; every other key is the input's, so none reaches the program.
    pub(super) fn input_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(chord) = chord_of(keystroke) else {
            return;
        };
        let typed = !chord.mods.ctrl && !chord.mods.alt && matches!(chord.key, Key::Char(_));
        if typed {
            return;
        }
        cx.stop_propagation();
        let Some(input) = self.input.as_mut() else {
            return;
        };
        let step = input.model.key(chord, None, &input.shared.history.borrow());
        match step {
            Step::Ignored | Step::Done => {}
            Step::Submit => self.submit_input(),
            Step::Complete => self.complete_input(),
            Step::Shell(bytes) => {
                self.send(bytes);
            }
        }
        cx.notify();
    }

    /// Input-method text or a paste, into the input.
    pub(super) fn input_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(input) = self.input.as_mut() {
            input.model.insert_text(text);
            cx.notify();
        }
    }

    fn submit_input(&mut self) {
        let bracketed = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.modes & MODE_BRACKETED_PASTE != 0);
        let seq = self.blocks["prompt"]["seq"].as_u64().unwrap_or(0);
        let Some(input) = self.input.as_mut() else {
            return;
        };
        let bytes = input
            .model
            .submit(bracketed, seq, &mut input.shared.history.borrow_mut());
        if let Some(bytes) = bytes {
            self.trace.borrow_mut().key_event();
            self.send(&bytes);
        }
    }

    /// Tab: completes against the shell's own folder, read from the server now.
    fn complete_input(&mut self) {
        let cwd = self
            .client
            .exchange(3, "terminal_info", &json!({"tab_id": self.tab}))
            .ok()
            .and_then(|info| info["cwd"].as_str().map(PathBuf::from));
        let (Some(cwd), Some(input)) = (cwd, self.input.as_mut()) else {
            return;
        };
        let home = clanky_core::platform::home_dir();
        let path = std::env::var_os("PATH");
        input.model.complete(&cwd, home.as_deref(), path.as_deref());
    }

    /// A click or drag in the input's text: the caret to that line and column.
    pub fn input_click(&mut self, line: usize, col: usize, extend: bool, cx: &mut Context<Self>) {
        if let Some(input) = self.input.as_mut() {
            input.model.click(line, col, extend);
            cx.notify();
        }
    }
}
