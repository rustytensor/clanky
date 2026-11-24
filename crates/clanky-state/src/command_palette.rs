//! The command palette's model: the commands it lists, which of them a typed query finds and in what order, and the
//! query itself. The window draws the list and runs the command picked.
//!
//! The matching rule, case ignored and the query trimmed:
//! - an empty query lists every command, in the list's own order;
//! - otherwise a command is listed when its label starts with the query (best), when one of its words starts with it
//!   (next), or when the query appears anywhere inside it (last); commands of one rank keep the list's order;
//! - a command whose label holds the query's letters only scattered (in order, but apart) is not listed: "spl" finds
//!   "Split right", never "Show or hide the tools panel";
//! - a query that is not empty and has no control characters also offers, last, to run it as a command in the pane
//!   that had the keyboard when the palette opened, when one had it.

use crate::keys::{Action, chords_for};

/// What a palette entry does when it is picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    NewTab,
    SplitRight,
    SplitDown,
    ClosePane,
    ToggleToolsPanel,
    /// Switch between top tabs and side tabs, and save the choice (`tab_style`).
    ToggleSideTabs,
    /// Show the tab at this place in the tab list.
    GoToTab(usize),
    /// Type this text and Enter into the pane the palette opened over.
    Run(String),
}

/// One row of the palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    /// The shortcut that runs the same command, as the key table spells it.
    pub keys: Option<String>,
    pub command: Command,
}

/// What the window knows when the palette lists its commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// The tabs in tab order: each one's title, and whether it is the active one.
    pub tabs: Vec<(String, bool)>,
    /// The name of the pane that had the keyboard when the palette opened, if a pane had it.
    pub pane: Option<String>,
    /// Whether the window shows side tabs now, which names the switch's direction.
    pub side_tabs: bool,
}

/// The longest query the palette takes, in bytes; more typing is dropped.
pub const QUERY_LIMIT: usize = 256;

/// The commands the window can run, in the list's order: the window's own commands, then a "Go to" entry for every
/// tab but the active one.
fn commands(context: &Context) -> Vec<Entry> {
    let entry = |label: &str, action, command| Entry {
        label: label.to_owned(),
        keys: chords_for(action).next().map(|chord| chord.label()),
        command,
    };
    let mut out = vec![
        entry("New tab", Action::NewTab, Command::NewTab),
        entry("Split right", Action::SplitRight, Command::SplitRight),
        entry("Split down", Action::SplitDown, Command::SplitDown),
        entry("Close pane", Action::ClosePane, Command::ClosePane),
        entry(
            "Show or hide the tools panel",
            Action::ToggleToolsPanel,
            Command::ToggleToolsPanel,
        ),
        Entry {
            label: if context.side_tabs { "Switch to top tabs" } else { "Switch to side tabs" }.to_owned(),
            keys: None,
            command: Command::ToggleSideTabs,
        },
    ];
    for (index, (title, active)) in context.tabs.iter().enumerate() {
        if !active {
            out.push(Entry {
                label: format!("Go to {title}"),
                keys: None,
                command: Command::GoToTab(index),
            });
        }
    }
    out
}

/// How well a label matches a query: 0 the label starts with it, 1 one of its words does, 2 it appears inside; None
/// when it does not match. The query is trimmed and case is ignored; an empty query matches everything at 0.
pub fn rank(label: &str, query: &str) -> Option<u8> {
    let label = label.to_lowercase();
    let query = query.trim().to_lowercase();
    if query.is_empty() || label.starts_with(&query) {
        return Some(0);
    }
    if label
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(&query))
    {
        return Some(1);
    }
    label.contains(&query).then_some(2)
}

/// The palette's rows for a query, best match first and in the list's order within a rank, then the offer to run the
/// query in the pane.
pub fn entries(context: &Context, query: &str) -> Vec<Entry> {
    let mut ranked: Vec<(u8, Entry)> = commands(context)
        .into_iter()
        .filter_map(|entry| rank(&entry.label, query).map(|rank| (rank, entry)))
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    let mut out: Vec<Entry> = ranked.into_iter().map(|(_, entry)| entry).collect();
    let typed = query.trim();
    if let Some(pane) = &context.pane
        && !typed.is_empty()
        && !typed.chars().any(char::is_control)
    {
        out.push(Entry {
            label: format!("Run \u{201c}{typed}\u{201d} in {pane}"),
            keys: None,
            command: Command::Run(typed.to_owned()),
        });
    }
    out
}

/// What has been typed into the palette.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    text: String,
}

impl Query {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Adds typed text. Text with a control character, or that would pass `QUERY_LIMIT`, is dropped; false then.
    pub fn insert(&mut self, typed: &str) -> bool {
        if typed.is_empty()
            || typed.chars().any(char::is_control)
            || self.text.len() + typed.len() > QUERY_LIMIT
        {
            return false;
        }
        self.text.push_str(typed);
        true
    }

    /// Removes the last character; false when there was none.
    pub fn backspace(&mut self) -> bool {
        self.text.pop().is_some()
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }
}

/// The first row shown when at most `room` rows fit: the list scrolls by whole rows just far enough to keep the
/// selected row in view, starting from where it was.
pub fn scroll_to_show(first: usize, selected: Option<usize>, len: usize, room: usize) -> usize {
    let room = room.max(1);
    let mut first = first.min(len.saturating_sub(room));
    if let Some(row) = selected {
        if row < first {
            first = row;
        } else if row >= first + room {
            first = row + 1 - room;
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(context: &Context, query: &str) -> Vec<String> {
        entries(context, query)
            .into_iter()
            .map(|entry| entry.label)
            .collect()
    }

    fn two_tabs() -> Context {
        Context {
            tabs: vec![("clanky".into(), true), ("larra".into(), false)],
            pane: Some("clanky \u{b7} pane 1".into()),
            side_tabs: false,
        }
    }

    // Every expected list below was worked out by hand from the rule in this file's comment and the command list.

    #[test]
    fn an_empty_query_lists_every_command_but_the_active_tab() {
        assert_eq!(
            labels(&two_tabs(), "  "),
            [
                "New tab",
                "Split right",
                "Split down",
                "Close pane",
                "Show or hide the tools panel",
                "Switch to side tabs",
                "Go to larra"
            ]
        );
    }

    #[test]
    fn typing_finds_commands_and_offers_to_run_the_rest() {
        let context = two_tabs();
        // Scattered letters do not match: "Show or hide the tools panel" holds s, p, l in order but is not listed.
        assert_eq!(
            labels(&context, "spl"),
            [
                "Split right",
                "Split down",
                "Run \u{201c}spl\u{201d} in clanky \u{b7} pane 1"
            ]
        );
        assert_eq!(
            labels(&context, "larra"),
            [
                "Go to larra",
                "Run \u{201c}larra\u{201d} in clanky \u{b7} pane 1"
            ]
        );
        assert_eq!(
            labels(&context, "git log"),
            ["Run \u{201c}git log\u{201d} in clanky \u{b7} pane 1"]
        );
    }

    #[test]
    fn word_starts_come_before_letters_inside_and_ties_keep_the_list_order() {
        // "t": no label starts with it; "tab", "the"/"tools", "to"/"tabs" and "to" start words; "split" only holds one.
        assert_eq!(
            labels(&two_tabs(), "t"),
            [
                "New tab",
                "Show or hide the tools panel",
                "Switch to side tabs",
                "Go to larra",
                "Split right",
                "Split down",
                "Run \u{201c}t\u{201d} in clanky \u{b7} pane 1"
            ]
        );
        // "ne": "New tab" starts with it; "pane" and "panel" only hold it.
        assert_eq!(
            labels(&two_tabs(), "NE"),
            [
                "New tab",
                "Close pane",
                "Show or hide the tools panel",
                "Run \u{201c}NE\u{201d} in clanky \u{b7} pane 1"
            ]
        );
    }

    #[test]
    fn the_tab_switch_names_the_layout_it_switches_to() {
        // Without a pane, so no "Run" entry follows.
        let top = Context {
            pane: None,
            ..two_tabs()
        };
        assert_eq!(labels(&top, "side"), ["Switch to side tabs"]);
        let side = Context {
            side_tabs: true,
            ..top
        };
        assert_eq!(labels(&side, "switch"), ["Switch to top tabs"]);
        assert_eq!(labels(&side, "side"), Vec::<String>::new());
    }

    #[test]
    fn without_a_pane_nothing_is_offered_to_run() {
        let context = Context {
            pane: None,
            ..two_tabs()
        };
        assert_eq!(labels(&context, "tab"), ["New tab", "Switch to side tabs"]);
        assert_eq!(labels(&context, "agent"), Vec::<String>::new());
    }

    #[test]
    fn commands_show_the_shortcut_that_runs_them() {
        let found = entries(&two_tabs(), "");
        let keys: Vec<Option<&str>> = found.iter().map(|entry| entry.keys.as_deref()).collect();
        assert_eq!(
            keys,
            [
                Some("Ctrl+Shift+T"),
                Some("Ctrl+Shift+D"),
                Some("Ctrl+Shift+E"),
                Some("Ctrl+Shift+W"),
                Some("Ctrl+Shift+B"),
                None,
                None
            ]
        );
    }

    #[test]
    fn picking_a_tab_names_its_place_and_the_run_entry_its_trimmed_text() {
        let found = entries(&two_tabs(), " larra ");
        assert_eq!(found[0].command, Command::GoToTab(1));
        assert_eq!(found[1].command, Command::Run("larra".into()));
    }

    #[test]
    fn the_query_drops_control_characters_and_text_past_its_limit() {
        let mut query = Query::default();
        assert!(query.insert("ls"));
        assert!(!query.insert("\u{1b}[A"), "an escape sequence is not text");
        assert!(!query.insert(&"x".repeat(QUERY_LIMIT - 1)));
        assert!(query.insert(&"x".repeat(QUERY_LIMIT - 2)));
        assert_eq!(query.text().len(), QUERY_LIMIT);
        assert!(query.backspace());
        assert_eq!(query.text().len(), QUERY_LIMIT - 1);
        query.clear();
        assert!(!query.backspace());
    }

    #[test]
    fn the_list_scrolls_just_far_enough_to_show_the_selected_row() {
        // Twelve rows, ten fit: row 11 selected shows rows 2 to 11; back up to row 1 shows rows 1 to 10.
        assert_eq!(scroll_to_show(0, Some(11), 12, 10), 2);
        assert_eq!(scroll_to_show(2, Some(1), 12, 10), 1);
        assert_eq!(scroll_to_show(2, Some(5), 12, 10), 2);
        // The list shrank under the first row shown.
        assert_eq!(scroll_to_show(5, Some(0), 3, 10), 0);
    }
}
