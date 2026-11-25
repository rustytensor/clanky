//! History: the commands and output kept on this computer.

use crate::settings::{PageContext, Row, whole_number};

/// The History page's rows, in the order today's Settings shows them.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    vec![
        Row::switch(
            "history",
            "Save history",
            "Keep commands and their output on this computer.",
            config.history,
        )
        .in_group("History"),
        Row::field(
            "history_days",
            "Keep for",
            "Days of saved history. Zero keeps everything.",
            config.history_days.to_string(),
            whole_number,
        ),
        Row::field(
            "history_output_kib",
            "Output per command",
            "The output saved with each command, in KiB.",
            config.history_output_kib.to_string(),
            whole_number,
        ),
    ]
}
