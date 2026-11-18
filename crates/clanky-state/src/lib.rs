//! Clanky's window-side state, kept apart from any drawing library so every window (today's and the gpui-ce one)
//! shares it: the config and its file, themes and terminal colour schemes, the command-input model, mouse selection,
//! notifications, cursor blink, the clipboard bridge, the saved layout and the key bindings. Tabs, panes and their
//! split tree live in `clanky-core`. The selection of a list the arrow keys move through is here too, with the find
//! bar's model and the command palette's list and its matching rule.
pub mod blocks;
pub mod clipboard;
pub mod command_palette;
pub mod command_input;
pub mod config;
pub mod config_xml;
pub mod files_tree;
pub mod cursor_blink;
pub mod layout_store;
pub mod list_selection;
pub mod look;
pub mod mouse;
pub mod keys;
pub mod notifications;
pub mod pane_find;
pub mod pane_input;
pub mod platform;
pub mod settings;
pub mod terminal_schemes;
pub mod theme_runtime;
pub mod theme_store;
pub mod themes;
