//! The agent picker's list ("Start an agent"): the configured agent tools, each marked installed or not (a tool that
//! is not installed is listed greyed, never hidden), narrowed by what is typed, and where the new pane opens. The
//! window draws it and starts the picked tool; this holds what is listed and what the keys do to it.

/// One configured agent tool as the picker lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentChoice {
    pub name: String,
    pub command: String,
    /// Whether the tool's program was found when the picker opened.
    pub installed: bool,
}

/// The picker's state while it is open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentPicker {
    choices: Vec<AgentChoice>,
    query: String,
    /// Open the agent in a new tab instead of a split to the right of the focused pane.
    new_tab: bool,
}

impl AgentPicker {
    /// The picker as it opens: every choice listed, nothing typed, the agent opening in a split.
    pub fn new(choices: Vec<AgentChoice>) -> Self {
        Self {
            choices,
            query: String::new(),
            new_tab: false,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn new_tab(&self) -> bool {
        self.new_tab
    }

    /// Switches between opening the agent in a split and in a new tab.
    pub fn toggle_new_tab(&mut self) {
        self.new_tab = !self.new_tab;
    }

    /// Adds typed text to the query. Control characters are not part of a name and are left out.
    pub fn type_text(&mut self, text: &str) {
        self.query.extend(text.chars().filter(|c| !c.is_control()));
    }

    /// Removes the query's last character; false when there was none.
    pub fn backspace(&mut self) -> bool {
        self.query.pop().is_some()
    }

    /// The choices whose name holds the query, case folded, in the config's order.
    pub fn shown(&self) -> Vec<&AgentChoice> {
        let query = self.query.to_lowercase();
        self.choices
            .iter()
            .filter(|choice| choice.name.to_lowercase().contains(&query))
            .collect()
    }

    /// The shown row the list starts on: the first installed one, else the first.
    pub fn first_row(&self) -> Option<usize> {
        let shown = self.shown();
        shown
            .iter()
            .position(|choice| choice.installed)
            .or_else(|| (!shown.is_empty()).then_some(0))
    }

    /// The tool a pick of shown row `row` starts: None for a row past the end or a tool that is not installed.
    pub fn pick(&self, row: usize) -> Option<&AgentChoice> {
        self.shown()
            .get(row)
            .copied()
            .filter(|choice| choice.installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(name: &str, installed: bool) -> AgentChoice {
        AgentChoice {
            name: name.into(),
            command: name.to_lowercase(),
            installed,
        }
    }

    fn picker() -> AgentPicker {
        AgentPicker::new(vec![
            choice("Aider", false),
            choice("Codex", true),
            choice("Claude Code", true),
        ])
    }

    fn names(picker: &AgentPicker) -> Vec<&str> {
        picker
            .shown()
            .iter()
            .map(|choice| choice.name.as_str())
            .collect()
    }

    #[test]
    fn it_opens_on_the_first_installed_tool_and_never_starts_a_missing_one() {
        let picker = picker();
        // Aider (row 0) is not installed, so the list starts on Codex (row 1).
        assert_eq!(picker.first_row(), Some(1));
        assert_eq!(picker.pick(0), None);
        assert_eq!(picker.pick(1).map(|c| c.command.as_str()), Some("codex"));
        assert_eq!(picker.pick(3), None);
    }

    #[test]
    fn typing_narrows_the_list_case_folded_and_backspace_widens_it() {
        let mut picker = picker();
        picker.type_text("C");
        // "c" is in Codex and Claude Code; Aider has none.
        assert_eq!(names(&picker), ["Codex", "Claude Code"]);
        assert_eq!(picker.first_row(), Some(0));
        picker.type_text("LA\u{8}");
        assert_eq!(picker.query(), "CLA");
        assert_eq!(names(&picker), ["Claude Code"]);
        picker.type_text("x");
        assert!(names(&picker).is_empty());
        assert_eq!(picker.first_row(), None);
        assert!(picker.backspace());
        assert!(picker.backspace());
        assert!(picker.backspace());
        assert!(picker.backspace());
        assert!(!picker.backspace(), "nothing left to remove");
        assert_eq!(names(&picker), ["Aider", "Codex", "Claude Code"]);
    }

    #[test]
    fn a_list_of_only_missing_tools_starts_on_its_first_row() {
        let picker = AgentPicker::new(vec![choice("Aider", false), choice("Pi", false)]);
        assert_eq!(picker.first_row(), Some(0));
        assert_eq!(picker.pick(0), None);
    }

    #[test]
    fn the_new_pane_opens_in_a_split_until_switched() {
        let mut picker = picker();
        assert!(!picker.new_tab());
        picker.toggle_new_tab();
        assert!(picker.new_tab());
    }
}
