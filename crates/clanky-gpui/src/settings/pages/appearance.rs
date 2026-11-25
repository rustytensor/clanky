//! Appearance: the window's layout, theme, text and opacity.

use clanky_state::config::{BlockStyle, CommandInput, TabGroups, TabStyle, ThemePreference};
use serde_json::{Value, json};

use crate::settings::{Changes, PageContext, Row, number, number_text, text};

/// The Appearance page's rows.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    let style = match config.tab_style {
        TabStyle::Horizontal => "horizontal",
        TabStyle::Vertical => "vertical",
    };
    let groups = match config.tab_groups {
        TabGroups::Project => "project",
        TabGroups::Flat => "none",
    };
    let input = match config.command_input {
        CommandInput::Editor => "editor",
        CommandInput::Classic => "classic",
    };
    let blocks = match config.block_style {
        BlockStyle::Quiet => "quiet",
        BlockStyle::Cards => "cards",
    };
    let follow = config.theme == ThemePreference::System;
    vec![
        Row::segments(
            "tab_style",
            "Tabs",
            "Along the top of the window, or stacked in a column on the left with each session's program, state \
             and branch.",
            &[("Top", json!("horizontal")), ("Side", json!("vertical"))],
            &json!(style),
        )
        .in_group("Layout"),
        Row::segments(
            "tab_groups",
            "Group side tabs by project",
            "Sessions in the same repository sit together under its name.",
            &[("On", json!("project")), ("Off", json!("none"))],
            &json!(groups),
        ),
        Row::segments(
            "tools_panel",
            "Tools panel",
            "A panel beside the panes. Ctrl+Shift+B shows it or puts it away.",
            // The file spells this setting "on" and "off", not true and false.
            &[("On", json!("on")), ("Off", json!("off"))],
            &json!(if config.tools_panel { "on" } else { "off" }),
        ),
        Row::segments(
            "command_input",
            "Command input",
            "An editor under each pane, or the shell's own prompt.",
            &[("Editor", json!("editor")), ("Classic", json!("classic"))],
            &json!(input),
        ),
        Row::segments(
            "block_style",
            "Blocks",
            "Quiet hairlines between commands, or framed blocks.",
            &[("Quiet", json!("quiet")), ("Cards", json!("cards"))],
            &json!(blocks),
        ),
        Row::segments(
            "status_line",
            "Status line",
            "Session, server and masking, in one row at the bottom.",
            // Spelled "show" and "hide" in the file.
            &[("Show", json!("show")), ("Hide", json!("hide"))],
            &json!(if config.status_line { "show" } else { "hide" }),
        ),
        // The options stand for following the system or not; what the file stores is a theme preference, so the
        // row writes `theme` itself. Off keeps the mode the window shows now, which is dark: the window does not
        // follow the desktop's mode yet and draws a "system" preference with the dark theme.
        Row::segments(
            "theme",
            "Follow the system",
            "Use one theme in dark mode and another in light mode.",
            &[("On", json!(true)), ("Off", json!(false))],
            &json!(follow),
        )
        .writes(follow_system)
        .in_group("Theme"),
        Row::field(
            "font_size",
            "Font size",
            "Ctrl + and Ctrl - change it for one window.",
            // The default comes from points (11 pt is 14.666... px); two places are as fine as a size is set.
            number_text((config.font_size * 100.0).round() / 100.0),
            number,
        )
        .in_group("Text"),
        Row::field(
            "font_family",
            "Terminal font",
            "Used in every terminal pane.",
            config.font_family.clone(),
            text,
        ),
        Row::field(
            "ui_font_family",
            "Interface font",
            "Used for menus, tabs and Settings.",
            config.ui_font_family.clone(),
            text,
        ),
        Row::field(
            "cell_width_scale",
            "Cell width",
            "Wider cells space the letters out, as in Ptyxis.",
            number_text(config.cell_width_scale),
            number,
        ),
        Row::field(
            "cell_height_scale",
            "Line height",
            "Taller rows space the lines out, as in Ptyxis.",
            number_text(config.cell_height_scale),
            number,
        ),
        Row::slider(
            "opacity",
            "Opacity",
            "How much of the desktop shows through.",
            // The file holds an f32; read back at two places so 0.8 shows as 0.8, not 0.800000011920929.
            (f64::from(config.opacity) * 100.0).round() / 100.0,
            (0.5, 1.0),
            0.01,
        )
        .in_group("Window"),
    ]
}

/// What the "Follow the system" row writes: `theme` "system" when it is on, "dark" when it is off.
fn follow_system(on: &Value) -> Changes {
    let theme = if on.as_bool() == Some(true) {
        "system"
    } else {
        "dark"
    };
    vec![("theme", Some(json!(theme)))]
}
