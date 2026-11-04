//! Hunspell-backed prose checking. Draft text is never persisted here.
use spellbook::Dictionary;
use std::{collections::hash_map::RandomState, fs, io::Write, ops::Range, path::{Path, PathBuf}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misspelling {
    pub range: Range<usize>,
    pub word: String,
}

pub struct Spellchecker {
    dictionary: Dictionary<RandomState>,
    personal: Option<PathBuf>,
}

fn apostrophe(ch: char) -> bool { ch == '\'' || ch == '’' }
fn valid_word(word: &str) -> bool {
    let chars: Vec<_> = word.chars().collect();
    !chars.is_empty() && chars.iter().enumerate().all(|(i, ch)| {
        ch.is_alphabetic() || (apostrophe(*ch) && i > 0 && i + 1 < chars.len()
            && chars[i - 1].is_alphabetic() && chars[i + 1].is_alphabetic())
    })
}

impl Spellchecker {
    pub fn load(dir: &Path, language: &str, personal: Option<PathBuf>) -> Result<Self, String> {
        if language.is_empty() || !language.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err("invalid dictionary language".into());
        }
        let aff = fs::read_to_string(dir.join(format!("{language}.aff"))).map_err(|e| format!("dictionary aff: {e}"))?;
        let dic = fs::read_to_string(dir.join(format!("{language}.dic"))).map_err(|e| format!("dictionary dic: {e}"))?;
        let mut dictionary = Dictionary::new_with_hasher(&aff, &dic, RandomState::new()).map_err(|e| format!("dictionary: {e}"))?;
        if let Some(path) = &personal {
            match fs::read_to_string(path) {
                Ok(words) => for word in words.lines() {
                    if valid_word(word) { dictionary.add(word).map_err(|e| format!("personal word: {e}"))?; }
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                Err(e) => return Err(format!("personal word list: {e}")),
            }
        }
        Ok(Self { dictionary, personal })
    }

    pub fn flagged(&self, text: &str) -> Vec<Misspelling> {
        prose_words(text).into_iter().filter_map(|range| {
            let word = &text[range.clone()];
            (!self.dictionary.check(word)).then(|| Misspelling { range, word: word.to_owned() })
        }).collect()
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut words = Vec::new();
        self.dictionary.suggest(word, &mut words);
        words
    }

    pub fn add_word(&mut self, word: &str) -> Result<(), String> {
        if !valid_word(word) { return Err("personal words must contain letters and internal apostrophes only".into()); }
        let path = self.personal.as_ref().ok_or("Add disabled: no personal word-list path")?;
        // Read the latest list, not a cached copy: preserve entries added elsewhere.
        let existing = match fs::read_to_string(path) {
            Ok(words) => words,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("personal word list: {e}")),
        };
        if !existing.lines().any(|line| line == word) {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| format!("personal word directory: {e}"))?;
            }
            let mut file = match clanky_core::platform::create_private_file(path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists =>
                    fs::OpenOptions::new().append(true).open(path)
                        .map_err(|e| format!("personal word list: {e}"))?,
                Err(e) => return Err(format!("personal word list: {e}")),
            };
            let entry = format!("{}{word}\n", if existing.is_empty() || existing.ends_with('\n') { "" } else { "\n" });
            file.write_all(entry.as_bytes()).and_then(|_| file.flush()).map_err(|e| format!("personal word list: {e}"))?;
        }
        self.dictionary.add(word).map_err(|e| format!("personal word: {e}"))
    }
}

// Work in byte ranges so Unicode spelling spans map directly to editor cells.
fn prose_words(text: &str) -> Vec<Range<usize>> {
    let mut masked = vec![false; text.len()];
    let mut in_ticks = false;
    let mut i = 0;
    while i < text.len() {
        if text.as_bytes()[i] == b'`' {
            let start = i;
            while i < text.len() && text.as_bytes()[i] == b'`' { i += 1; }
            masked[start..i].fill(true);
            in_ticks = !in_ticks;
        } else {
            masked[i] = in_ticks;
            i += 1;
        }
    }
    let mut words = Vec::new();
    let mut offset = 0;
    for token in text.split_whitespace() {
        offset += text[offset..].find(token).unwrap();
        let end = offset + token.len();
        // Prose punctuation around a flag/path does not turn it into prose.
        let shape = token.trim_matches(['(', ')', '[', ']', '{', '}', '\'', '"', '“', '”', '‘', '’', ',', ';', ':', '!', '?', '.']);
        let code = shape.starts_with('-') || shape.contains(['/', '\\', '_']) || shape.contains("::")
            || shape.chars().any(|c| c.is_numeric())
            || shape.char_indices().any(|(i, c)| c == '.' && i > 0 && i + 1 < shape.len());
        if !code {
            let mut start = None;
            let chars: Vec<_> = token.char_indices().collect();
            for (n, &(byte, ch)) in chars.iter().enumerate() {
                let absolute = offset + byte;
                let internal = apostrophe(ch) && n > 0 && n + 1 < chars.len()
                    && chars[n - 1].1.is_alphabetic() && chars[n + 1].1.is_alphabetic();
                if !masked[absolute] && (ch.is_alphabetic() || internal) {
                    start.get_or_insert(absolute);
                } else if let Some(begin) = start.take() { words.push(begin..absolute); }
            }
            if let Some(begin) = start { words.push(begin..end); }
        }
        offset = end;
    }
    words
}
