//! General: how Clanky starts and what a new window holds.

use serde_json::json;

use crate::settings::{PageContext, Row, whole_number};

/// The General page's rows, in the order today's Settings shows them.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    vec![
        Row::switch(
            "post_ai",
            "Post-AI mode",
            "A plain terminal with tabs and splits. No agents, no AI features.",
            config.post_ai,
        )
        .in_group("Startup"),
        Row::switch(
            "restore_layout",
            "Restore layout",
            "Reopen the last tabs and splits when Clanky starts.",
            config.restore_layout,
        ),
        Row::segments(
            "new_window_panes",
            "New windows open with",
            "Applies to the next new window. A restored layout keeps its own panes.",
            &[("One pane", json!(1)), ("Two panes", json!(2))],
            &json!(config.new_window_panes),
        ),
        Row::switch(
            "startup_animation",
            "Robot greeting",
            "The robot says hello when a window opens.",
            config.startup_animation,
        ),
        Row::switch(
            "animations",
            "Animations",
            "Motion in the window, the robot's greeting included.",
            config.animations,
        ),
        Row::field(
            "notify_after_seconds",
            "Notify after",
            "A notice when a command runs this long, in seconds.",
            config.notify_after_seconds.to_string(),
            whole_number,
        ),
        Row::field(
            "columns",
            "Columns",
            "Width of a new window in characters. Applies to new windows.",
            config.columns.to_string(),
            whole_number,
        ),
        Row::field(
            "rows",
            "Rows",
            "Height of a new window in lines. Applies to new windows.",
            config.rows.to_string(),
            whole_number,
        ),
    ]
}
