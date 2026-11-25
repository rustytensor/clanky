//! Keys: the window's shortcuts, read only.
//!
//! The keys shown come from the window's own key table (`clanky_state::keys`), so a binding changed there shows
//! here without a second list to keep in step.

use clanky_state::keys::{Action, chords_for};

use crate::settings::{PageContext, Row};

/// One shortcut row: its title, what it does, the action whose keys it shows, and whether it belongs to the agent
/// features that Post-AI mode turns off.
struct Shortcut {
    label: &'static str,
    description: &'static str,
    action: Action,
    agent: bool,
}

const fn shortcut(
    label: &'static str,
    description: &'static str,
    action: Action,
    agent: bool,
) -> Shortcut {
    Shortcut {
        label,
        description,
        action,
        agent,
    }
}

/// The shortcuts in the order the page lists them: Settings first, as today, then sessions and panes, text,
/// the window's own panels, and the agent keys last.
const SHORTCUTS: &[Shortcut] = &[
    shortcut(
        "Settings",
        "Open or close this panel.",
        Action::ToggleSettings,
        false,
    ),
    shortcut(
        "Command palette",
        "Find any command by name.",
        Action::ToggleCommandPalette,
        false,
    ),
    shortcut(
        "New tab",
        "Start a shell in a new session.",
        Action::NewTab,
        false,
    ),
    shortcut(
        "Next session",
        "Switch to the next session.",
        Action::NextTab,
        false,
    ),
    shortcut(
        "Split columns",
        "Put a new pane beside the current one.",
        Action::SplitRight,
        false,
    ),
    shortcut(
        "Split rows",
        "Put a new pane below the current one.",
        Action::SplitDown,
        false,
    ),
    shortcut(
        "Close pane",
        "Close the current pane.",
        Action::ClosePane,
        false,
    ),
    shortcut("Copy", "Copy the selected text.", Action::Copy, false),
    shortcut(
        "Paste",
        "Paste into the focused pane.",
        Action::Paste,
        false,
    ),
    shortcut(
        "Find",
        "Search the focused pane's output.",
        Action::FindInScrollback,
        false,
    ),
    shortcut(
        "Open file",
        "Open a file in the editor.",
        Action::OpenFile,
        false,
    ),
    shortcut(
        "Compose",
        "Write a long prompt, then send it to the pane.",
        Action::OpenCompose,
        false,
    ),
    shortcut(
        "Tools panel",
        "Show or hide the tools panel.",
        Action::ToggleToolsPanel,
        false,
    ),
    shortcut(
        "Larger text",
        "Increase text size for this window.",
        Action::ZoomIn,
        false,
    ),
    shortcut(
        "Smaller text",
        "Decrease text size for this window.",
        Action::ZoomOut,
        false,
    ),
    shortcut(
        "Reset zoom",
        "Return to the saved text size.",
        Action::ZoomReset,
        false,
    ),
    shortcut(
        "Block cards",
        "Move the keyboard between the focused pane's commands.",
        Action::CycleCardFocus,
        false,
    ),
    shortcut(
        "Window controls",
        "Move the keyboard from the pane to the window's buttons.",
        Action::EnterControls,
        false,
    ),
    shortcut(
        "Start an agent",
        "Pick an agent to run in a pane.",
        Action::ToggleAgentPicker,
        true,
    ),
    shortcut(
        "Pane agent access",
        "Toggle agent access for the focused pane.",
        Action::ToggleAgentAccess,
        true,
    ),
    shortcut(
        "Stop agents",
        "Revoke agent access and cancel pending waits.",
        Action::StopAgents,
        true,
    ),
];

/// The Keys page's rows: one read-only card per shortcut, the agent ones left out in Post-AI mode, and Escape last
/// because it is Settings' own key rather than one of the window's.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let post_ai = context.config.post_ai;
    SHORTCUTS
        .iter()
        .filter(|shortcut| !(post_ai && shortcut.agent))
        .map(|shortcut| {
            // An action with two chords (Ctrl+= and Ctrl++) shows both, in the table's order.
            let keys = chords_for(shortcut.action)
                .map(|chord| chord.label())
                .collect::<Vec<_>>()
                .join(" or ");
            Row::keys(shortcut.label, shortcut.description, keys)
        })
        .chain(std::iter::once(Row::keys(
            "Close Settings",
            "Cancel an edit first, or close this panel.",
            "Escape",
        )))
        .collect()
}
