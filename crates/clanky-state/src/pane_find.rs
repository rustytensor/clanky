//! The pane's find bar, without its drawing: the text being looked for, how many times the pane holds it, which match
//! is the current one, and the cells to mark. The session server does the looking (`window_search`: a literal,
//! case-sensitive search over the pane's history and screen that joins only soft-wrapped rows, and scrolls the view to
//! the current match); this keeps what it answered, checked before it is used, and asks again as the text changes or
//! Enter and Shift+Enter move to the next or previous match.

use clanky_api::Client;
use serde_json::{Value, json};

/// The longest text the server looks for, in bytes.
pub const QUERY_LIMIT: usize = 256;
/// The largest view the server may answer with, either way and in cells: a bigger one is a broken answer.
const MAX_SIDE: u64 = 1000;
const MAX_CELLS: u64 = 250_000;

/// A run of marked cells on one row of the view, `start..end` in columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    /// Part of the current match.
    pub current: bool,
}

/// What the server found: the number of matches, where the current one starts (a line counted from the oldest line
/// of the history, and a column), and the cells of the view the matches cover.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub total: usize,
    pub selected: Option<(usize, usize)>,
    pub runs: Vec<Run>,
}

impl Found {
    /// The server's answer, checked: its view's size, the number of cells, every position inside the view, and a
    /// current match exactly when there is a match. Anything else is refused whole.
    pub fn parse(value: &Value) -> Result<Self, String> {
        let snapshot = &value["snapshot"];
        let rows = snapshot["rows"]
            .as_u64()
            .ok_or("the search answer has no view rows")?;
        let cols = snapshot["cols"]
            .as_u64()
            .ok_or("the search answer has no view columns")?;
        if rows == 0 || cols == 0 || rows > MAX_SIDE || cols > MAX_SIDE || rows * cols > MAX_CELLS {
            return Err(format!(
                "the search answer's view is {rows} x {cols}, outside 1..={MAX_SIDE}"
            ));
        }
        let (rows, cols) = (as_index(rows)?, as_index(cols)?);
        if snapshot["cells"].as_array().map(Vec::len) != Some(rows * cols) {
            return Err(
                "the search answer's view does not hold one cell per row and column".into(),
            );
        }
        let total = value["total"]
            .as_u64()
            .ok_or("the search answer has no match count")
            .and_then(|total| as_index(total).map_err(|_| "the match count is too large"))?;
        let selected = match &value["selected"] {
            Value::Null => None,
            place => Some(position(place)?),
        };
        if (total == 0) != selected.is_none() {
            return Err(format!(
                "the search answer counts {total} matches but its current match is {selected:?}"
            ));
        }
        let highlights = value["highlights"]
            .as_array()
            .ok_or("the search answer has no marked cells")?;
        if highlights.len() > rows * cols {
            return Err("the search answer marks more cells than its view holds".into());
        }
        let mut cells = Vec::with_capacity(highlights.len());
        for mark in highlights {
            let (row, col) = position(mark)?;
            if row >= rows || col >= cols {
                return Err(format!(
                    "the search answer marks the cell {row}, {col} outside its {rows} x {cols} view"
                ));
            }
            let current = mark["selected"]
                .as_bool()
                .ok_or("a marked cell does not say whether it is current")?;
            cells.push((row, col, current));
        }
        Ok(Found {
            total,
            selected,
            runs: runs_of(cells),
        })
    }
}

fn as_index(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{value} is too large for this machine"))
}

/// A `{"row", "col"}` object of the server's answer.
fn position(value: &Value) -> Result<(usize, usize), String> {
    let number = |name: &str| {
        value[name]
            .as_u64()
            .ok_or_else(|| format!("a position in the search answer has no {name}"))
            .and_then(as_index)
    };
    Ok((number("row")?, number("col")?))
}

/// Marked cells joined into runs along their rows: neighbouring cells of the same kind make one run.
fn runs_of(mut cells: Vec<(usize, usize, bool)>) -> Vec<Run> {
    cells.sort_unstable();
    let mut runs: Vec<Run> = Vec::new();
    for (row, col, current) in cells {
        match runs.last_mut() {
            Some(run) if run.row == row && run.end == col && run.current == current => run.end += 1,
            _ => runs.push(Run {
                row,
                start: col,
                end: col + 1,
                current,
            }),
        }
    }
    runs
}

/// What the bar says about the matches: nothing before any text is typed, then "No matches", "1 match" or
/// "N matches".
pub fn count_label(query: &str, total: usize) -> String {
    match (query.is_empty(), total) {
        (true, _) => String::new(),
        (false, 0) => "No matches".into(),
        (false, 1) => "1 match".into(),
        (false, total) => format!("{total} matches"),
    }
}

/// The text with more typed after it, refused past the server's limit.
pub fn extended(query: &str, typed: &str) -> Result<String, String> {
    if query.len().saturating_add(typed.len()) > QUERY_LIMIT {
        return Err(format!(
            "the text to find is limited to {QUERY_LIMIT} bytes"
        ));
    }
    Ok(format!("{query}{typed}"))
}

/// One pane's find bar.
#[derive(Clone, Debug)]
pub struct PaneFind {
    pane: String,
    query: String,
    found: Found,
}

impl PaneFind {
    /// The bar opened on a pane, with nothing typed.
    pub fn new(pane: String) -> Self {
        PaneFind {
            pane,
            query: String::new(),
            found: Found::default(),
        }
    }

    pub fn pane(&self) -> &str {
        &self.pane
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn found(&self) -> &Found {
        &self.found
    }

    pub fn count_label(&self) -> String {
        count_label(&self.query, self.found.total)
    }

    /// Text typed into the bar: the matches of the longer text, the first one current.
    pub fn type_text(&mut self, client: &Client, typed: &str) -> Result<(), String> {
        if typed.is_empty() {
            return Ok(());
        }
        let query = extended(&self.query, typed)?;
        self.search(client, query, false, false)
    }

    /// Backspace: the last character goes, and the matches of what is left are found.
    pub fn backspace(&mut self, client: &Client) -> Result<(), String> {
        let mut query = self.query.clone();
        if query.pop().is_none() {
            return Ok(());
        }
        self.search(client, query, false, false)
    }

    /// Enter (forward) or Shift+Enter (backwards): the next or previous match becomes current, wrapping at the ends.
    pub fn step(&mut self, client: &Client, backwards: bool) -> Result<(), String> {
        if self.query.is_empty() {
            return Ok(());
        }
        self.search(client, self.query.clone(), true, backwards)
    }

    /// Asks the server; a failed call leaves the text and the matches as they were.
    fn search(
        &mut self,
        client: &Client,
        query: String,
        navigate: bool,
        backwards: bool,
    ) -> Result<(), String> {
        if query.is_empty() {
            self.query = query;
            self.found = Found::default();
            return Ok(());
        }
        let after = self
            .found
            .selected
            .filter(|_| navigate)
            .map(|(row, col)| json!({"row": row, "col": col}));
        let answer = client
            .exchange(
                3,
                "window_search",
                &json!({"tab_id": self.pane, "query": query, "after": after, "backwards": backwards}),
            )
            .map_err(|error| format!("finding in the pane: {}", error.message))?;
        self.found =
            Found::parse(&answer).map_err(|problem| format!("finding in the pane: {problem}"))?;
        self.query = query;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An answer for a 2 x 4 view; the cells' contents are not read, only their number.
    fn answer(total: u64, selected: Value, highlights: Value) -> Value {
        json!({
            "total": total, "selected": selected, "highlights": highlights,
            "snapshot": {"rows": 2, "cols": 4, "cells": vec![Value::Null; 8]},
        })
    }

    #[test]
    fn the_count_reads_as_the_bar_says_it_today() {
        assert_eq!(count_label("", 0), "");
        assert_eq!(count_label("g1", 0), "No matches");
        assert_eq!(count_label("g1", 1), "1 match");
        assert_eq!(count_label("g1", 6), "6 matches");
    }

    #[test]
    fn marked_cells_become_runs_with_the_current_match_apart() {
        // "g1" at row 0 columns 1-2 and, current, at row 1 columns 0-1; given out of order.
        let found = Found::parse(&answer(
            2,
            json!({"row": 41, "col": 0}),
            json!([
                {"row": 1, "col": 1, "selected": true}, {"row": 0, "col": 1, "selected": false},
                {"row": 0, "col": 2, "selected": false}, {"row": 1, "col": 0, "selected": true},
            ]),
        ))
        .expect("a well-formed answer");
        assert_eq!(found.total, 2);
        assert_eq!(found.selected, Some((41, 0)));
        assert_eq!(
            found.runs,
            [
                Run {
                    row: 0,
                    start: 1,
                    end: 3,
                    current: false
                },
                Run {
                    row: 1,
                    start: 0,
                    end: 2,
                    current: true
                },
            ]
        );
    }

    #[test]
    fn two_matches_side_by_side_stay_two_runs_when_one_is_current() {
        let found = Found::parse(&answer(
            2,
            json!({"row": 0, "col": 0}),
            json!([
                {"row": 0, "col": 0, "selected": true}, {"row": 0, "col": 1, "selected": true},
                {"row": 0, "col": 2, "selected": false}, {"row": 0, "col": 3, "selected": false},
            ]),
        ))
        .expect("a well-formed answer");
        assert_eq!(
            found.runs,
            [
                Run {
                    row: 0,
                    start: 0,
                    end: 2,
                    current: true
                },
                Run {
                    row: 0,
                    start: 2,
                    end: 4,
                    current: false
                },
            ]
        );
    }

    #[test]
    fn broken_answers_are_refused() {
        let cell_outside = answer(
            1,
            json!({"row": 0, "col": 0}),
            json!([{"row": 0, "col": 4, "selected": true}]),
        );
        assert!(
            Found::parse(&cell_outside)
                .unwrap_err()
                .contains("outside its 2 x 4 view")
        );
        let current_without_match = answer(0, json!({"row": 0, "col": 0}), json!([]));
        assert!(
            Found::parse(&current_without_match)
                .unwrap_err()
                .contains("counts 0 matches")
        );
        let match_without_current = answer(1, Value::Null, json!([]));
        assert!(Found::parse(&match_without_current).is_err());
        let mut short = answer(0, Value::Null, json!([]));
        short["snapshot"]["cells"] = json!([null, null]);
        assert!(
            Found::parse(&short)
                .unwrap_err()
                .contains("one cell per row and column")
        );
        let mut huge = answer(0, Value::Null, json!([]));
        huge["snapshot"]["rows"] = json!(600);
        huge["snapshot"]["cols"] = json!(600);
        assert!(Found::parse(&huge).unwrap_err().contains("600 x 600"));
        let mut uncounted = answer(0, Value::Null, json!([]));
        uncounted["total"] = json!("none");
        assert!(
            Found::parse(&uncounted)
                .unwrap_err()
                .contains("no match count")
        );
    }

    #[test]
    fn the_text_stops_at_the_servers_limit() {
        let almost = "a".repeat(QUERY_LIMIT - 1);
        assert_eq!(
            extended(&almost, "b").map(|query| query.len()),
            Ok(QUERY_LIMIT)
        );
        assert!(extended(&almost, "bc").is_err());
    }
}
