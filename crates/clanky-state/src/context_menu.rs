//! The menu a right-click opens over a terminal pane, as in the mockup Marc approved on 2026-11-13: Ptyxis's rows
//! (links, copy and paste, find, reset) plus Clanky's own (splits, the agent, agent access, close pane). This holds
//! which rows it shows for what is under the pointer and which row the keyboard has selected; the window draws it and
//! runs the rows.
//!
//! Keys: the menu opens with no row selected. Down selects the next row and Up the previous one; from no selection,
//! Down selects the first row and Up the last. Greyed rows are skipped, and the selection stops at either end, as
//! every list of the window does. Enter picks the selected row.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use clanky_api::Client;
use serde_json::json;

use crate::{
    keys::Chord,
    list_selection::{ListKey, ListSelection, ListStep},
};

/// What a row does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    OpenLink,
    CopyLink,
    Copy,
    Paste,
    SelectAll,
    CopyCommand,
    CopyOutput,
    RunAgain,
    Find,
    SplitRight,
    SplitBelow,
    StartAgent,
    AgentAccess,
    Reset,
    ResetClear,
    ClosePane,
}

/// A finished command block under the pointer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub command: String,
}

/// What is under the pointer when the menu opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// A web address under the pointer.
    pub link: Option<String>,
    pub block: Option<Block>,
    /// The pane holds a selection with text in it.
    pub selection: bool,
    /// Post-AI mode: no agent rows.
    pub post_ai: bool,
    /// Agents may use this pane.
    pub access: bool,
}

/// One row: what it does, its label, the shortcut shown at its right end (empty for none), and whether it can act.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub item: Item,
    pub label: &'static str,
    pub keys: &'static str,
    pub enabled: bool,
}

/// Rows drawn together, with a line between sections; some have a heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub heading: Option<&'static str>,
    pub rows: Vec<Row>,
}

fn row(item: Item, label: &'static str, keys: &'static str, enabled: bool) -> Row {
    Row {
        item,
        label,
        keys,
        enabled,
    }
}

fn section(rows: Vec<Row>) -> Section {
    Section {
        heading: None,
        rows,
    }
}

/// The menu's sections, top to bottom, for what is under the pointer.
pub fn sections(context: &Context) -> Vec<Section> {
    let mut sections = Vec::new();
    if context.link.is_some() {
        sections.push(section(vec![
            row(Item::OpenLink, "Open link", "Ctrl+click", true),
            row(Item::CopyLink, "Copy link", "", true),
        ]));
    }
    sections.push(section(vec![
        row(Item::Copy, "Copy", "Ctrl+Shift+C", context.selection),
        row(Item::Paste, "Paste", "Ctrl+Shift+V", true),
        row(Item::SelectAll, "Select all", "", true),
    ]));
    if context.block.is_some() {
        sections.push(Section {
            heading: Some("This command"),
            rows: vec![
                row(Item::CopyCommand, "Copy command", "", true),
                row(Item::CopyOutput, "Copy output", "", true),
                row(Item::RunAgain, "Run again", "", true),
            ],
        });
    }
    let mut tools = vec![
        row(Item::Find, "Find…", "Ctrl+Shift+F", true),
        row(Item::SplitRight, "Split right", "Ctrl+Shift+D", true),
        row(Item::SplitBelow, "Split below", "Ctrl+Shift+E", true),
    ];
    if !context.post_ai {
        tools.push(row(
            Item::StartAgent,
            "Start an agent here…",
            "Ctrl+Shift+G",
            true,
        ));
        tools.push(row(
            Item::AgentAccess,
            "Agent access",
            if context.access { "On" } else { "Off" },
            true,
        ));
    }
    sections.push(section(tools));
    sections.push(section(vec![
        row(Item::Reset, "Reset terminal", "", true),
        row(Item::ResetClear, "Reset and clear", "", true),
    ]));
    sections.push(section(vec![row(
        Item::ClosePane,
        "Close pane",
        "Ctrl+Shift+W",
        true,
    )]));
    sections
}

/// An open menu: its sections and the row the keyboard selected, counted over all sections top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub context: Context,
    sections: Vec<Section>,
    selection: ListSelection,
}

impl Menu {
    pub fn new(context: Context) -> Self {
        let sections = sections(&context);
        let len = sections.iter().map(|section| section.rows.len()).sum();
        Menu {
            context,
            sections,
            selection: ListSelection::new(len),
        }
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The row at a place counted over all sections.
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.sections
            .iter()
            .flat_map(|section| &section.rows)
            .nth(index)
    }

    pub fn selected(&self) -> Option<usize> {
        self.selection.selected()
    }

    /// A key while the menu has the keyboard: the arrows, Home and End move the selection over the rows that can
    /// act, and Enter picks the selected row.
    pub fn key(&mut self, chord: Chord) -> ListKey {
        let Some(step) = ListStep::of(chord) else {
            return match self.selection.key(chord) {
                ListKey::Pick(index) if !self.enabled(index) => ListKey::Ignored,
                other => other,
            };
        };
        let onward = match step {
            ListStep::First | ListStep::Down => ListStep::Down,
            ListStep::Last | ListStep::Up => ListStep::Up,
        };
        let mut probe = self.selection;
        probe.step(step);
        // A greyed row is passed over in the same direction; with none past it, the selection stays where it was.
        while let Some(index) = probe.selected() {
            if self.enabled(index) {
                self.selection = probe;
                break;
            }
            probe.step(onward);
            if probe.selected() == Some(index) {
                break;
            }
        }
        ListKey::Moved
    }

    fn enabled(&self, index: usize) -> bool {
        self.row(index).is_some_and(|row| row.enabled)
    }
}

/// The largest output the menu copies at once, as today's window: anything longer is copied by a byte range through the
/// API, so one reply stays well inside the socket's line limit.
const MAX_COPY_BYTES: usize = 64 * 1024;

/// A finished block's output, as the server copies it (with secrets masked): it is selected whole on the server first,
/// as the block's own Copy does, through the owner's palette requests. Large or cut-short output has to be copied by a
/// byte range through the API instead.
pub fn block_output(client: &Client, tab: &str, block: &str) -> Result<Vec<u8>, String> {
    let view = client
        .palette_action("view", &json!({"tab_id": tab, "limit": 64}), true)
        .map_err(|e| format!("reading the command's output: {}", e.message))?;
    let listed = view["blocks"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["block_id"].as_str() == Some(block))
        })
        .ok_or("this command is no longer on screen")?;
    let len = listed["output"]
        .as_str()
        .ok_or("the server sent no output for this command")?
        .len();
    if listed["truncated"].as_bool().unwrap_or(false) || len > MAX_COPY_BYTES {
        return Err(
            "this output is too long to copy from the menu; copy a byte range through the API"
                .into(),
        );
    }
    let args = json!({"tab_id": tab, "block_id": block, "start": 0, "end": len});
    client
        .palette_action("select", &args, true)
        .map_err(|e| format!("selecting the command's output: {}", e.message))?;
    let copied = client
        .palette_action("copy", &json!({"tab_id": tab, "block_id": block}), true)
        .map_err(|e| format!("copying the command's output: {}", e.message))?;
    STANDARD
        .decode(
            copied["data"]
                .as_str()
                .ok_or("the server sent no copied output")?,
        )
        .map_err(|e| format!("reading the copied output: {e}"))
}

/// The bytes that type a block's command again and run it; an error for one that cannot be typed (empty, or with
/// control characters that would do something else in the shell).
pub fn rerun_bytes(command: &str) -> Result<Vec<u8>, String> {
    if command.trim().is_empty() || command.chars().any(char::is_control) {
        return Err("this command cannot be typed again".into());
    }
    Ok(format!("{command}\r").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{Key, Mods};

    fn press(menu: &mut Menu, keys: &[Key]) -> ListKey {
        let mut last = ListKey::Ignored;
        for key in keys {
            last = menu.key(Chord::new(Mods::NONE, *key));
        }
        last
    }

    fn picked(menu: &Menu, result: ListKey) -> Option<Item> {
        match result {
            ListKey::Pick(index) => menu.row(index).map(|row| row.item),
            _ => None,
        }
    }

    #[test]
    fn keys_skip_greyed_rows_and_post_ai_drops_the_agent_rows() {
        use Key::{Down as D, Enter as E, Up as U};
        // Worked out from the approved rows (the same counts as today's menu): with nothing selected, Copy is greyed,
        // so Down x4 is Paste, Select all, Find…, Split right; Up from no selection is Close pane.
        let mut menu = Menu::new(Context::default());
        let result = press(&mut menu, &[D, D, D, D, E]);
        assert_eq!(picked(&menu, result), Some(Item::SplitRight));
        let mut menu = Menu::new(Context::default());
        let result = press(&mut menu, &[U, E]);
        assert_eq!(picked(&menu, result), Some(Item::ClosePane));
        let block = Block {
            id: "b1".into(),
            command: "echo hi".into(),
        };
        let mut menu = Menu::new(Context {
            selection: true,
            block: Some(block),
            ..Context::default()
        });
        let result = press(&mut menu, &[D, D, D, D, D, E]);
        assert_eq!(picked(&menu, result), Some(Item::CopyOutput));
        // Post-AI: Paste, Select all, Find…, Split right, Split below, Reset terminal, Reset and clear.
        let mut menu = Menu::new(Context {
            post_ai: true,
            ..Context::default()
        });
        let result = press(&mut menu, &[D, D, D, D, D, D, D, E]);
        assert_eq!(picked(&menu, result), Some(Item::ResetClear));
    }

    #[test]
    fn the_selection_stops_at_the_ends_and_never_rests_on_a_greyed_row() {
        use Key::{Down, End, Enter, Home, Up};
        let mut menu = Menu::new(Context::default());
        // Copy (row 0) is greyed: Home goes to Paste (1), and Up from there stays on Paste.
        press(&mut menu, &[Home]);
        assert_eq!(menu.selected(), Some(1));
        press(&mut menu, &[Up]);
        assert_eq!(menu.selected(), Some(1));
        // 11 rows without a link or block; End and Down stop on Close pane (10).
        press(&mut menu, &[End, Down]);
        assert_eq!(menu.selected(), Some(10));
        assert_eq!(menu.key(Chord::new(Mods::NONE, Enter)), ListKey::Pick(10));
    }

    #[test]
    fn a_link_and_a_block_add_their_rows_and_access_shows_its_state() {
        let menu = Menu::new(Context {
            link: Some("https://example.invalid/".into()),
            block: Some(Block::default()),
            access: true,
            ..Context::default()
        });
        let labels: Vec<&str> = menu
            .sections()
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.label))
            .collect();
        assert_eq!(
            labels,
            [
                "Open link",
                "Copy link",
                "Copy",
                "Paste",
                "Select all",
                "Copy command",
                "Copy output",
                "Run again",
                "Find…",
                "Split right",
                "Split below",
                "Start an agent here…",
                "Agent access",
                "Reset terminal",
                "Reset and clear",
                "Close pane"
            ]
        );
        assert_eq!(menu.sections()[2].heading, Some("This command"));
        assert_eq!(menu.row(12).map(|row| row.keys), Some("On"));
    }

    #[test]
    fn a_command_with_control_characters_is_not_typed_again() {
        assert_eq!(rerun_bytes("echo ok"), Ok(b"echo ok\r".to_vec()));
        assert!(rerun_bytes("echo a\nrm -rf x").is_err());
        assert!(rerun_bytes("   ").is_err());
    }
}
