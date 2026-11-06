//! Shared application configuration and input state.
pub mod config;
pub mod history_cli;
pub mod frame_trace;
pub mod frame_clock;
pub mod themes;
pub mod theme_scene;
pub mod calm;
pub mod theme_store;
pub mod theme_runtime;
pub mod robot;
pub mod robot_gate_clock;
pub mod settings;
pub mod settings_panel;
#[cfg(test)]
mod settings_panel_tests;
pub mod platform;
pub mod look;
pub mod clipboard;
pub mod scrollback_search;
pub mod window_tools;
pub mod compose;
pub mod editor;
pub mod input;
pub mod mcp;
pub mod layout_store;
pub mod notifications;
pub mod session;
