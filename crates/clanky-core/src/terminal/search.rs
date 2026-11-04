//! Literal search over retained physical cells, joining only soft-wrapped rows.
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
struct Span {
    row: usize,
    col: usize,
    width: usize,
    leading: bool,
}

impl Terminal {
    // KMP keeps only query-sized byte-to-cell mapping, never a string per hit
    // or a copy of the complete scrollback. A second pass marks the final view.
    fn search_hits(&self, query: &[u8], mut hit: impl FnMut((usize, usize), &VecDeque<Span>)) {
        let mut prefix = vec![0; query.len()];
        let mut matched = 0;
        for i in 1..query.len() {
            while matched > 0 && query[i] != query[matched] {
                matched = prefix[matched - 1];
            }
            if query[i] == query[matched] { matched += 1; }
            prefix[i] = matched;
        }
        matched = 0;
        let mut recent = VecDeque::with_capacity(query.len());
        let grid = self.term.grid();
        let history = grid.history_size();
        for row in 0..history + self.size.0 {
            let line = Line(row as i32 - history as i32);
            for col in 0..self.size.1 {
                let cell = &grid[Point::new(line, Column(col))];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let leading = col == 0 && row > 0 && grid[Point::new(
                    Line(line.0 - 1), Column(self.size.1 - 1)
                )].flags.contains(Flags::LEADING_WIDE_CHAR_SPACER);
                let span = Span { row, col, leading,
                    width: if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 } };
                for ch in std::iter::once(cell.c).chain(cell.zerowidth().into_iter().flatten().copied()) {
                    let mut buffer = [0; 4];
                    for &byte in ch.encode_utf8(&mut buffer).as_bytes() {
                        if recent.len() == query.len() { recent.pop_front(); }
                        recent.push_back(span);
                        while matched > 0 && byte != query[matched] {
                            matched = prefix[matched - 1];
                        }
                        if byte == query[matched] { matched += 1; }
                        if matched == query.len() {
                            let start = recent.front().unwrap();
                            hit((start.row, start.col), &recent);
                            matched = prefix[matched - 1];
                        }
                    }
                }
            }
            if !grid[Point::new(line, Column(self.size.1 - 1))].flags.contains(Flags::WRAPLINE) {
                matched = 0;
                recent.clear();
            }
        }
    }

    pub fn window_search(&mut self, query: &str, after: Option<(usize, usize)>, backwards: bool)
        -> Result<Value, String>
    {
        if query.len() > 256 { return Err("search query exceeds 256 bytes".into()); }
        let mut total = 0usize;
        let mut first = None;
        let mut last = None;
        let mut candidate = None;
        if !query.is_empty() {
            self.search_hits(query.as_bytes(), |start, _| {
                total += 1;
                first.get_or_insert(start);
                last = Some(start);
                if let Some(after) = after {
                    if backwards && start < after { candidate = Some(start); }
                    if !backwards && start > after && candidate.is_none() { candidate = Some(start); }
                }
            });
        }
        let selected = if after.is_some() { candidate } else { None }
            .or(if backwards { last } else { first });
        let history = self.term.grid().history_size();
        if let Some((row, _)) = selected {
            // Align the start even when it was on the last visible row, so a
            // wrapped continuation is visible too wherever history permits it.
            let offset = history.saturating_sub(row);
            self.scroll(offset as i32 - self.term.grid().display_offset() as i32);
        }
        let top = history - self.term.grid().display_offset();
        let mut marks = vec![0u8; self.size.0 * self.size.1];
        if !query.is_empty() {
            self.search_hits(query.as_bytes(), |start, spans| {
                let mark = if Some(start) == selected { 2 } else { 1 };
                let mut paint = |row: usize, col: usize| {
                    if row >= top && row - top < self.size.0 && col < self.size.1 {
                        let index = (row - top) * self.size.1 + col;
                        marks[index] = marks[index].max(mark);
                    }
                };
                for span in spans {
                    for col in span.col..(span.col + span.width).min(self.size.1) { paint(span.row, col); }
                    if span.leading { paint(span.row - 1, self.size.1 - 1); }
                }
            });
        }
        let highlights: Vec<_> = marks.iter().enumerate().filter(|(_, mark)| **mark != 0)
            .map(|(i, mark)| json!({"row":i / self.size.1,"col":i % self.size.1,"selected":*mark == 2}))
            .collect();
        let mut snapshot = self.snapshot();
        snapshot.images.clear();
        Ok(json!({"total":total,"selected":selected.map(|(row,col)| json!({"row":row,"col":col})),
            "snapshot":snapshot,"highlights":highlights}))
    }
}
