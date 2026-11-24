//! Syntax colours for editor panes, from tree-sitter grammars.
//!
//! A [`Syntax`] owns one parse tree for one document. Text changes are replayed
//! as [`TextEdit`]s so the tree is re-parsed incrementally, and [`Syntax::spans`]
//! queries only the byte range on screen. Colouring is a pure function of the text:
//! an edited document colours exactly like a fresh parse of the same bytes.
use clanky_edit::TextEdit;
use std::{
    ops::{ControlFlow, Range},
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

/// Files larger than this stay plain text.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
/// A parse that takes longer than this gives up and the file stays plain.
const PARSE_BUDGET: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language { Rust, Python, TypeScript, Tsx, JavaScript, Go, C, Bash, Json, Toml, Yaml, Markdown }

impl Language {
    pub const ALL: [Language; 12] = [Language::Rust, Language::Python, Language::TypeScript, Language::Tsx,
        Language::JavaScript, Language::Go, Language::C, Language::Bash, Language::Json, Language::Toml,
        Language::Yaml, Language::Markdown];

    /// The language of a file from its name, else from a `#!` first line.
    pub fn detect(path: &Path, text: &str) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if let Some(lang) = Self::from_name(name) { return Some(lang); }
        let first = text.lines().next()?.strip_prefix("#!")?;
        let mut words = first.split_whitespace();
        let program = words.next()?;
        let program = if program.ends_with("/env") { words.find(|w| !w.starts_with('-'))? } else { program };
        match program.rsplit('/').next()? {
            "bash" | "sh" | "zsh" | "dash" => Some(Self::Bash),
            p if p.starts_with("python") => Some(Self::Python),
            "node" | "deno" | "bun" => Some(Self::JavaScript),
            _ => None,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            ".bashrc" | ".bash_profile" | ".profile" | ".bash_aliases" | ".bash_logout" => return Some(Self::Bash),
            "Cargo.lock" => return Some(Self::Toml),
            _ => {}
        }
        let (_, ext) = name.rsplit_once('.')?;
        Some(match ext.to_ascii_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" | "pyi" => Self::Python,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "go" => Self::Go,
            "c" | "h" => Self::C,
            "sh" | "bash" => Self::Bash,
            "json" => Self::Json,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "md" | "markdown" => Self::Markdown,
            _ => return None,
        })
    }
}

/// A colour class. The window theme gives each one its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Token { Keyword, Function, Type, String, Number, Constant, Comment, Plain }

/// What one capture name paints: `None` paints nothing (the capture is ignored).
fn token(name: &str) -> Option<Token> {
    let first = name.split('.').next().unwrap_or(name);
    Some(match name {
        "none" => return None,
        "text.title" => Token::Keyword,
        "text.uri" | "string.special.key" | "property.key" => Token::Function,
        "text.reference" => Token::Type,
        "text.literal" => Token::String,
        "function.macro" | "function.special" => Token::Constant,
        _ => match first {
            "comment" => Token::Comment,
            "keyword" => Token::Keyword,
            "function" => Token::Function,
            "type" | "constructor" => Token::Type,
            "string" | "escape" => Token::String,
            "number" => Token::Number,
            "constant" | "boolean" | "attribute" | "embedded" => Token::Constant,
            _ => Token::Plain,
        },
    })
}

// Clanky's own patterns, appended after the upstream queries. Among colours on one
// node the later pattern wins, so these decide where the approved design differs.
const RUST_EXTRA: &str = r#"
((identifier) @constant (#match? @constant "^[A-Z][A-Z0-9_]+$"))
(self) @keyword
(boolean_literal) @keyword
(integer_literal) @number
(float_literal) @number
(attribute_item) @whole.constant
(inner_attribute_item) @whole.constant
(token_tree (identifier) @function . (token_tree))
"#;
const JSON_EXTRA: &str = "(pair key: (string) @property.key)";
const TOML_EXTRA: &str = r#"
(pair (bare_key) @property.key)
(pair (quoted_key) @property.key)
(pair (dotted_key (bare_key) @property.key))
"#;
const YAML_EXTRA: &str = r#"
(block_mapping_pair key: (flow_node (plain_scalar (string_scalar) @property.key)))
(block_mapping_pair key: (flow_node [(double_quote_scalar) (single_quote_scalar)] @property.key))
"#;

struct Grammar {
    language: tree_sitter::Language,
    query: Query,
    /// Per capture index: the token it paints and whether it covers everything inside it.
    paints: Vec<Option<(Token, bool)>>,
}

impl Grammar {
    fn new(language: tree_sitter::Language, sources: &[&str]) -> Self {
        let source = sources.join("\n");
        let query = Query::new(&language, &source).unwrap_or_else(|e| panic!("highlight query: {e}"));
        let paints = query.capture_names().iter().map(|name| match name.strip_prefix("whole.") {
            Some(rest) => token(rest).map(|t| (t, true)),
            None => token(name).map(|t| (t, false)),
        }).collect();
        Self { language, query, paints }
    }
}

fn grammar(lang: Language) -> &'static Grammar {
    static CELLS: [OnceLock<Grammar>; 12] = [const { OnceLock::new() }; 12];
    let index = Language::ALL.iter().position(|&l| l == lang).unwrap_or(0);
    CELLS[index].get_or_init(|| {
        let js = tree_sitter_javascript::HIGHLIGHT_QUERY;
        let jsx = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
        let ts = tree_sitter_typescript::HIGHLIGHTS_QUERY;
        match lang {
            Language::Rust => Grammar::new(tree_sitter_rust::LANGUAGE.into(), &[tree_sitter_rust::HIGHLIGHTS_QUERY, RUST_EXTRA]),
            Language::Python => Grammar::new(tree_sitter_python::LANGUAGE.into(), &[tree_sitter_python::HIGHLIGHTS_QUERY]),
            Language::TypeScript => Grammar::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), &[js, ts]),
            Language::Tsx => Grammar::new(tree_sitter_typescript::LANGUAGE_TSX.into(), &[js, jsx, ts]),
            Language::JavaScript => Grammar::new(tree_sitter_javascript::LANGUAGE.into(), &[js, jsx]),
            Language::Go => Grammar::new(tree_sitter_go::LANGUAGE.into(), &[tree_sitter_go::HIGHLIGHTS_QUERY]),
            Language::C => Grammar::new(tree_sitter_c::LANGUAGE.into(), &[tree_sitter_c::HIGHLIGHT_QUERY]),
            Language::Bash => Grammar::new(tree_sitter_bash::LANGUAGE.into(), &[tree_sitter_bash::HIGHLIGHT_QUERY]),
            Language::Json => Grammar::new(tree_sitter_json::LANGUAGE.into(), &[tree_sitter_json::HIGHLIGHTS_QUERY, JSON_EXTRA]),
            Language::Toml => Grammar::new(tree_sitter_toml_ng::LANGUAGE.into(), &[tree_sitter_toml_ng::HIGHLIGHTS_QUERY, TOML_EXTRA]),
            Language::Yaml => Grammar::new(tree_sitter_yaml::LANGUAGE.into(), &[tree_sitter_yaml::HIGHLIGHTS_QUERY, YAML_EXTRA]),
            Language::Markdown => Grammar::new(tree_sitter_md::LANGUAGE.into(), &[tree_sitter_md::HIGHLIGHT_QUERY_BLOCK]),
        }
    })
}

/// Markdown's inline grammar, run over the visible `inline` nodes of the block tree.
fn markdown_inline() -> &'static (Grammar, Query) {
    static INLINE: OnceLock<(Grammar, Query)> = OnceLock::new();
    INLINE.get_or_init(|| {
        let grammar = Grammar::new(tree_sitter_md::INLINE_LANGUAGE.into(), &[tree_sitter_md::HIGHLIGHT_QUERY_INLINE]);
        let block: tree_sitter::Language = tree_sitter_md::LANGUAGE.into();
        let inlines = Query::new(&block, "(inline) @inline").unwrap_or_else(|e| panic!("inline query: {e}"));
        (grammar, inlines)
    })
}

fn point(p: (usize, usize)) -> Point { Point { row: p.0, column: p.1 } }

fn parse(parser: &mut Parser, text: &str, old: Option<&Tree>) -> Option<Tree> {
    let started = Instant::now();
    let bytes = text.as_bytes();
    let mut read = |offset: usize, _: Point| bytes.get(offset..).unwrap_or(&[]);
    let mut progress = |_: &tree_sitter::ParseState| {
        if started.elapsed() > PARSE_BUDGET { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    };
    parser.parse_with_options(&mut read, old, Some(tree_sitter::ParseOptions::new().progress_callback(&mut progress)))
}

/// One painted byte range.
pub type Span = (Range<usize>, Token);

/// One document's parse tree.
pub struct Syntax {
    lang: Language,
    parser: Parser,
    tree: Option<Tree>,
    inline: Option<Parser>,
    revision: u64,
}

impl Syntax {
    /// Parse the whole text. Files over [`MAX_BYTES`] or slower than the budget stay plain.
    pub fn open(lang: Language, text: &str) -> Self {
        let mut parser = Parser::new();
        let ok = parser.set_language(&grammar(lang).language).is_ok();
        let tree = if ok && text.len() <= MAX_BYTES { parse(&mut parser, text, None) } else { None };
        Self { lang, parser, tree, inline: None, revision: 0 }
    }
    pub fn language(&self) -> Language { self.lang }
    /// False when the file is too large or too slow to colour.
    pub fn coloured(&self) -> bool { self.tree.is_some() }
    /// Changes whenever the colours may change.
    pub fn revision(&self) -> u64 { self.revision }

    /// Replay the document's edits, oldest first, then re-parse incrementally.
    pub fn edit(&mut self, edits: &[TextEdit], text: &str) {
        if edits.is_empty() { return; }
        self.revision += 1;
        let Some(tree) = &mut self.tree else { return; };
        for e in edits {
            tree.edit(&InputEdit { start_byte: e.start, old_end_byte: e.old_end, new_end_byte: e.new_end,
                start_position: point(e.start_point), old_end_position: point(e.old_end_point),
                new_end_position: point(e.new_end_point) });
        }
        let old = self.tree.take();
        self.tree = if text.len() <= MAX_BYTES { parse(&mut self.parser, text, old.as_ref()) } else { None };
    }

    /// The tree as an S-expression, for comparing an incremental parse with a fresh one.
    pub fn sexp(&self) -> Option<String> { self.tree.as_ref().map(|t| t.root_node().to_sexp()) }

    /// Coloured runs inside `range`, sorted and not overlapping. Bytes not covered are plain.
    pub fn spans(&mut self, text: &str, range: Range<usize>) -> Vec<Span> {
        let Some(tree) = &self.tree else { return Vec::new(); };
        let range = range.start.min(text.len())..range.end.min(text.len());
        if range.is_empty() { return Vec::new(); }
        let mut paint: Vec<Option<Token>> = vec![None; range.len()];
        let mut layer = collect(grammar(self.lang), tree, text, range.clone());
        if self.lang == Language::Markdown {
            let (inline_grammar, inlines) = markdown_inline();
            let mut cursor = QueryCursor::new();
            cursor.set_byte_range(range.clone());
            let mut ranges = Vec::new();
            let mut matches = cursor.matches(inlines, tree.root_node(), text.as_bytes());
            while let Some(m) = matches.next() {
                for c in m.captures() { ranges.push(c.node.range()); }
            }
            ranges.sort_by_key(|r| r.start_byte);
            ranges.dedup_by_key(|r| r.start_byte);
            if !ranges.is_empty() {
                let parser = self.inline.get_or_insert_with(|| {
                    let mut p = Parser::new();
                    let _ = p.set_language(&inline_grammar.language);
                    p
                });
                if parser.set_included_ranges(&ranges).is_ok()
                    && let Some(inline_tree) = parse(parser, text, None)
                {
                    // Inline captures sit inside block ones: they paint after all of them.
                    layer.extend(collect(inline_grammar, &inline_tree, text, range.clone()).into_iter()
                        .map(|mut p| { p.order.0 = 1; p }));
                }
            }
        }
        layer.sort_by_key(|p| p.order);
        for p in &layer {
            let a = p.bytes.start.max(range.start) - range.start;
            let b = p.bytes.end.min(range.end).saturating_sub(range.start);
            for slot in paint.get_mut(a..b.max(a)).into_iter().flatten() {
                *slot = Some(p.token);
            }
        }
        let mut out: Vec<Span> = Vec::new();
        for (i, token) in paint.into_iter().enumerate() {
            let Some(token) = token.filter(|t| *t != Token::Plain) else { continue; };
            let at = range.start + i;
            match out.last_mut() {
                Some((r, t)) if r.end == at && *t == token => r.end = at + 1,
                _ => out.push((at..at + 1, token)),
            }
        }
        out
    }
}

struct Painted {
    bytes: Range<usize>,
    token: Token,
    /// Painted in ascending order, so the later paint wins: covering captures last,
    /// inner (shorter) nodes after outer ones, a colour after plain on the same node,
    /// and a later pattern after an earlier one.
    /// The first field is the layer: Markdown's inline grammar paints over its block grammar.
    order: (u8, usize, usize, bool, usize),
}

fn collect(grammar: &Grammar, tree: &Tree, text: &str, range: Range<usize>) -> Vec<Painted> {
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(range);
    let mut out = Vec::new();
    let mut matches = cursor.matches(&grammar.query, tree.root_node(), text.as_bytes());
    while let Some(m) = matches.next() {
        for c in m.captures() {
            let Some((token, whole)) = grammar.paints[c.index as usize] else { continue; };
            let bytes = c.node.byte_range();
            if bytes.is_empty() { continue; }
            let inner = usize::MAX - bytes.len();
            out.push(Painted { order: (0, usize::from(whole), inner, token != Token::Plain, m.pattern_index), bytes, token });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grammar_and_query_loads() {
        for lang in Language::ALL {
            let mut s = Syntax::open(lang, "x");
            assert!(s.coloured(), "{lang:?}");
            let _ = s.spans("x", 0..1);
        }
    }
}
