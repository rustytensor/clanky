//! Terminal: the cursor, the clipboard and the terminal's colours.

use clanky_state::config::CursorBlink;
use serde_json::json;

use crate::settings::{PageContext, Row, whole_number};

/// The Terminal page's rows, in the order today's Settings shows them.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    // The config file spells the blink modes in lower case.
    let blink = match config.cursor_blink {
        CursorBlink::System => "system",
        CursorBlink::On => "on",
        CursorBlink::Off => "off",
    };
    vec![
        Row::segments(
            "cursor_blink",
            "Cursor blink",
            "System follows the GNOME setting.",
            &[
                ("System", json!("system")),
                ("On", json!("on")),
                ("Off", json!("off")),
            ],
            &json!(blink),
        )
        .in_group("Cursor"),
        Row::field(
            "cursor_blink_time_ms",
            "Blink cycle",
            "One full blink, shown and hidden, in milliseconds.",
            config.cursor_blink_time_ms.to_string(),
            whole_number,
        ),
        Row::field(
            "cursor_blink_timeout_s",
            "Stop blinking after",
            "Seconds after the last key press.",
            config.cursor_blink_timeout_s.to_string(),
            whole_number,
        ),
        Row::switch(
            "osc52_write",
            "Clipboard writes",
            "Programs in the terminal may copy text (OSC 52).",
            config.osc52_write,
        )
        .in_group("Clipboard"),
        Row::switch(
            "osc52_read",
            "Clipboard reads",
            "Programs in the terminal may read what you copied.",
            config.osc52_read,
        ),
    ]
}
