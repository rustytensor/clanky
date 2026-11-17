//! The Files tool's model: the rows of a folder's tree as the tools panel lists them, and the folder's short name.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

/// One row of the tree: a file or a folder, how deep it sits, and for a folder whether it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub depth: usize,
    pub open: bool,
}

/// The most rows listed, so a huge folder cannot stall the window.
const MOST_ROWS: usize = 400;
/// The most entries read from one folder.
const MOST_ENTRIES: usize = 2000;
/// How deep open folders are followed.
const MOST_DEPTH: usize = 8;

/// The rows under `root`: folders first, then files, each sorted by name ignoring case; an open folder (its path in
/// `open`) lists its own rows beneath it. Hidden names are left out; at most 400 rows.
pub fn rows(root: &Path, open: &BTreeSet<String>) -> Vec<TreeRow> {
    let mut out = Vec::new();
    walk(root, 0, open, &mut out);
    out
}

fn walk(dir: &Path, depth: usize, open: &BTreeSet<String>, out: &mut Vec<TreeRow>) {
    // An unreadable folder lists nothing: the tree shows what can be read.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<(bool, String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let path = entry.path();
            Some((path.is_dir(), name, path))
        })
        .take(MOST_ENTRIES)
        .collect();
    items.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
            .then_with(|| a.1.cmp(&b.1))
    });
    for (dir, name, path) in items {
        if out.len() >= MOST_ROWS {
            return;
        }
        let key = path.to_string_lossy().into_owned();
        let is_open = dir && open.contains(&key);
        out.push(TreeRow {
            path: key,
            name,
            dir,
            depth,
            open: is_open,
        });
        if is_open && depth < MOST_DEPTH {
            walk(&path, depth + 1, open, out);
        }
    }
}

/// A folder's name for the panel's head: "~" for the home folder, else its last part.
pub fn folder_name(cwd: &str, home: Option<&str>) -> String {
    if home == Some(cwd) {
        return "~".into();
    }
    Path::new(cwd).file_name().map_or_else(
        || cwd.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_first_by_name_ignoring_case_hidden_left_out_open_folders_listed() {
        let root = std::env::temp_dir().join(format!("files-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["src", "Docs", ".git", "src/inner"] {
            std::fs::create_dir_all(root.join(dir)).expect("making a test folder");
        }
        for file in ["b.txt", "A.md", ".hidden", "src/main.rs"] {
            std::fs::write(root.join(file), "").expect("writing a test file");
        }
        let open: BTreeSet<String> = [root.join("src").to_string_lossy().into_owned()].into();
        let listed: Vec<(String, bool, usize, bool)> = rows(&root, &open)
            .into_iter()
            .map(|row| (row.name, row.dir, row.depth, row.open))
            .collect();
        std::fs::remove_dir_all(&root).expect("removing the test folder");
        let expected: Vec<(String, bool, usize, bool)> = [
            ("Docs", true, 0, false),
            ("src", true, 0, true),
            ("inner", true, 1, false),
            ("main.rs", false, 1, false),
            ("A.md", false, 0, false),
            ("b.txt", false, 0, false),
        ]
        .into_iter()
        .map(|(name, dir, depth, open)| (name.to_owned(), dir, depth, open))
        .collect();
        assert_eq!(listed, expected);
    }

    #[test]
    fn a_folder_is_named_by_its_last_part_and_home_is_a_tilde() {
        assert_eq!(folder_name("/home/someone", Some("/home/someone")), "~");
        assert_eq!(
            folder_name("/home/someone/git/clanky", Some("/home/someone")),
            "clanky"
        );
        assert_eq!(folder_name("/", None), "/");
    }
}
