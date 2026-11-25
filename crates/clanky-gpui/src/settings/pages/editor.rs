//! Editor: spelling and sending in the compose editor.

use crate::settings::{PageContext, Row, text};

/// The Editor page's rows, in the order today's Settings shows them.
pub fn rows(context: &PageContext<'_>) -> Vec<Row> {
    let config = context.config;
    vec![
        Row::field(
            "compose_dictionary",
            "Dictionary",
            "The spelling language of the compose editor.",
            config.compose_dictionary.clone(),
            text,
        )
        .in_group("Compose"),
        // The settings model rejects a folder that is not absolute, so the plain text parser is enough here.
        Row::field(
            "compose_dictionary_dir",
            "Dictionary folder",
            "Where the spelling dictionaries are.",
            config.compose_dictionary_dir.to_string_lossy().into_owned(),
            text,
        ),
        Row::switch(
            "compose_send_enter",
            "Enter after send",
            "Press Enter in the pane after a draft is sent.",
            config.compose_send_enter,
        ),
    ]
}
