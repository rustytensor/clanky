//! Today's window and its tools. The window-side state lives in `clanky-state` and is re-exported here under the
//! same paths, so `clanky::config` and the like keep working.
pub use clanky_state::config;
pub use clanky_state::config_xml;
pub mod history_cli;
pub mod agent_launch;
pub mod agent_cli;
pub use clanky_state::cursor_blink;
pub mod frame_trace;
pub mod frame_clock;
pub use clanky_state::themes;
pub use clanky_state::terminal_schemes;
pub mod theme_scene;
pub mod calm;
pub use clanky_state::theme_store;
pub use clanky_state::theme_runtime;
pub mod robot;
pub mod robot_gate_clock;
pub use clanky_state::settings;
pub mod settings_panel;
#[cfg(test)]
mod settings_panel_tests;
pub mod platform;
pub mod look;
pub use clanky_state::clipboard;
pub mod scrollback_search;
pub mod window_tools;
pub use clanky_state::mouse;
pub mod compose;
pub mod editor;
pub mod input;
pub use clanky_state::command_input;
pub mod mcp;
pub use clanky_state::layout_store;
pub use clanky_state::notifications;
pub mod session;
