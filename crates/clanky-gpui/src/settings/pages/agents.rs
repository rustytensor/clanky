//! Agents: what an agent started in a new pane may do.

use serde_json::json;

use crate::settings::{PageContext, Row};

/// The Agents page's rows, in the order today's Settings shows them.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    // The config holds the access as a flag; the file spells it "read+run" or "off".
    let access = if config.agent_pane_access {
        "read+run"
    } else {
        "off"
    };
    vec![
        Row::segments(
            "agent_pane_access",
            "New agent panes",
            "What an agent started in a new pane may do.",
            &[("Read + run", json!("read+run")), ("Off", json!("off"))],
            &json!(access),
        )
        .in_group("New panes"),
    ]
}
