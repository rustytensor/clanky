use clanky_edit::{Document, HunkState, Location, Selection};
use std::{
    env,
    error::Error,
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ORIGINAL: &str = "alpha\r\nβeta\nlast";
const EDITED: &str = "alpha\r\nBeta\nlast";
const LINES: &str = "one\ntwo\nthree\n";

// The directory owns every fixture, including during assertion unwinding.
struct Fixtures(PathBuf);
impl Fixtures {
    fn new() -> Result<Self> {
        let root =
            PathBuf::from(env::var_os("CARGO_TARGET_DIR").ok_or("CARGO_TARGET_DIR is required")?);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for n in 0..100 {
            let path = root.join(format!(
                "editor-core-gate-{}-{stamp}-{n}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err("cannot allocate private fixture directory".into())
    }
    fn file(&self, name: &str, bytes: impl AsRef<[u8]>) -> Result<PathBuf> {
        let path = self.0.join(name);
        fs::write(&path, bytes)?;
        Ok(path)
    }
    fn names(&self) -> Result<Vec<std::ffi::OsString>> {
        let mut names = fs::read_dir(&self.0)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }
}
impl Drop for Fixtures {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.0) {
            eprintln!("FAIL: fixture cleanup: {e}");
        }
    }
}
fn bytes(path: &Path, expected: &str) -> Result {
    assert_eq!(fs::read(path)?, expected.as_bytes(), "disk bytes");
    Ok(())
}
fn selection(doc: &Document, anchor: usize, cursor: usize) {
    let got = doc.selection();
    assert_eq!((got.anchor, got.cursor), (anchor, cursor), "selection");
}
fn state(doc: &Document, id: u64, expected: HunkState) {
    assert_eq!(
        doc.hunks()
            .iter()
            .find(|h| h.id == id)
            .expect("hunk exists")
            .state,
        expected
    );
}

fn item1(f: &Fixtures) -> Result {
    let path = f.file("mixed", ORIGINAL)?;
    let mut doc = Document::open(&path)?;
    assert_eq!(doc.text(), ORIGINAL);
    doc.select(Selection {
        anchor: 7,
        cursor: 9,
    })?;
    assert!(doc.edit(8..8, "x").is_err(), "split UTF-8 must fail");
    assert_eq!(doc.text(), ORIGINAL);
    selection(&doc, 7, 9);
    doc.edit(7..9, "B")?;
    assert_eq!(doc.text(), EDITED);
    selection(&doc, 8, 8);
    assert!(doc.undo()?);
    assert_eq!(doc.text(), ORIGINAL);
    selection(&doc, 7, 9);
    assert!(doc.redo()?);
    assert_eq!(doc.text(), EDITED);
    selection(&doc, 8, 8);
    doc.save()?;
    bytes(&path, EDITED)?;
    assert_eq!(Document::open(&path)?.text(), EDITED);
    assert!(doc.undo()?, "save must retain undo");
    assert_eq!(doc.text(), ORIGINAL);
    selection(&doc, 7, 9);
    Ok(())
}
fn item2(f: &Fixtures, fault: bool) -> Result {
    let path = f.file("review", LINES)?;
    let mut doc = Document::open(&path)?;
    let first = doc.propose(0..3, "ONE")?;
    let second = doc.propose(8..13, "THREE")?;
    assert_ne!(first, second);
    state(&doc, first, HunkState::Pending);
    state(&doc, second, HunkState::Pending);
    assert_eq!(doc.text(), LINES);
    bytes(&path, LINES)?;
    doc.accept(second)?;
    doc.reject(first)?;
    let observed = if fault { "WRONG" } else { doc.text() };
    assert_eq!(observed, "one\ntwo\nTHREE\n", "item 2 accepted text");
    bytes(&path, LINES)?;
    assert!(doc.undo()?);
    assert_eq!(doc.text(), LINES);
    state(&doc, first, HunkState::Rejected);
    state(&doc, second, HunkState::Accepted);
    assert!(doc.redo()?);
    assert_eq!(doc.text(), "one\ntwo\nTHREE\n");
    state(&doc, first, HunkState::Rejected);
    state(&doc, second, HunkState::Accepted);
    assert!(doc.accept(first).is_err());
    assert!(doc.reject(second).is_err());
    bytes(&path, LINES)?;
    doc.save()?;
    bytes(&path, "one\ntwo\nTHREE\n")
}
fn item3(f: &Fixtures) -> Result {
    let path = f.file("shift", LINES)?;
    let mut doc = Document::open(&path)?;
    let first = doc.propose(0..3, "FIRST")?;
    let second = doc.propose(8..13, "THIRD")?;
    doc.accept(first)?;
    let shifted = doc.hunks().iter().find(|h| h.id == second).unwrap();
    assert_eq!(shifted.range, 10..15);
    assert_eq!(shifted.replacement, "THIRD");
    doc.accept(second)?;
    assert_eq!(doc.text(), "FIRST\ntwo\nTHIRD\n");
    let path = f.file("overlap", LINES)?;
    let mut doc = Document::open(path)?;
    let first = doc.propose(0..3, "FIRST")?;
    let overlap = doc.propose(1..3, "x")?;
    doc.accept(first)?;
    state(&doc, overlap, HunkState::Stale);
    assert!(doc.accept(overlap).is_err());
    assert_eq!(doc.text(), "FIRST\ntwo\nthree\n");
    assert!(doc.undo()?);
    assert_eq!(doc.text(), LINES);
    state(&doc, overlap, HunkState::Stale);
    assert!(doc.accept(overlap).is_err());
    Ok(())
}
fn item4(f: &Fixtures) -> Result {
    // Distinct real external changes: bytes, same-length bytes, deletion, inode.
    for (name, replacement) in [
        ("external", Some("external\n")),
        ("same-length", Some("ONE\ntwo\n")),
        ("deleted", None),
        ("replaced", Some("one\ntwo\n")),
    ] {
        let path = f.file(name, "one\ntwo\n")?;
        let mut doc = Document::open(&path)?;
        let id = doc.propose(0..3, "ONE")?;
        if name == "replaced" {
            let other = f.file("replacement-inode", "one\ntwo\n")?;
            fs::rename(other, &path)?;
        } else if let Some(text) = replacement {
            fs::write(&path, text)?;
        } else {
            fs::remove_file(&path)?;
        }
        let names = f.names()?;
        assert!(
            !matches!(doc.refresh(), Ok(true)),
            "external change must fail closed"
        );
        state(&doc, id, HunkState::Stale);
        assert!(doc.accept(id).is_err());
        assert!(doc.save().is_err());
        assert_eq!(doc.text(), "one\ntwo\n");
        if let Some(text) = replacement {
            bytes(&path, text)?;
        } else {
            assert!(!path.exists());
        }
        assert_eq!(
            f.names()?,
            names,
            "conflict must leave no temporary siblings"
        );
        // Restoring bytes does not clear a latched conflict.
        fs::write(&path, "one\ntwo\n")?;
        assert!(doc.save().is_err());
    }
    // Accept itself must refresh, even without an explicit refresh call.
    let path = f.file("accept-refresh", "one\ntwo\n")?;
    let mut doc = Document::open(&path)?;
    let id = doc.propose(0..3, "ONE")?;
    fs::write(&path, "external\n")?;
    assert!(doc.accept(id).is_err());
    state(&doc, id, HunkState::Stale);
    assert_eq!(doc.text(), "one\ntwo\n");
    bytes(&path, "external\n")
}
fn item5(f: &Fixtures) -> Result {
    let loc = Location::parse("src/a:b.rs:2:2")?;
    assert_eq!(loc.path, PathBuf::from("src/a:b.rs"));
    assert_eq!((loc.line, loc.column), (2, 2));
    let short = Location::parse("a.rs:3")?;
    assert_eq!(short.path, PathBuf::from("a.rs"));
    assert_eq!((short.line, short.column), (3, 1));
    for bad in [
        "a.rs:0",
        "a.rs:1:0",
        "a.rs:184467440737095516160",
        "a.rs",
        "a.rs:no",
    ] {
        assert!(Location::parse(bad).is_err(), "invalid location: {bad}");
    }
    let doc = Document::open(f.file("positions", ORIGINAL)?)?;
    assert_eq!(doc.offset(loc.line, loc.column)?, 9);
    assert_eq!(doc.offset(1, 6)?, 5);
    assert_eq!(doc.offset(2, 1)?, 7);
    assert_eq!(doc.offset(2, 5)?, 12);
    assert_eq!(doc.offset(3, 5)?, 17);
    for (line, col) in [
        (0, 1),
        (1, 0),
        (1, 7),
        (2, 6),
        (4, 1),
        (usize::MAX, 1),
        (1, usize::MAX),
    ] {
        assert!(
            doc.offset(line, col).is_err(),
            "invalid position {line}:{col}"
        );
    }
    let doc = Document::open(f.file("final-empty-line", "a\n")?)?;
    assert_eq!(doc.offset(2, 1)?, 2);
    assert!(doc.offset(2, 2).is_err());
    let doc = Document::open(f.file("standalone-cr", "a\rb")?)?;
    assert_eq!(doc.offset(1, 3)?, 2);
    assert_eq!(doc.offset(1, 4)?, 3);
    assert!(doc.offset(2, 1).is_err());
    Ok(())
}
fn item6(f: &Fixtures) -> Result {
    let original = "let x = 1;\n".repeat(1_525_202);
    assert_eq!(original.len(), 16_777_222);
    // Construct expected bytes independently, not from Document output.
    let expected = format!("{}let x = 2;\n", "let x = 1;\n".repeat(1_525_201));
    let path = f.file("large", &original)?;
    let mut doc = Document::open(&path)?;
    let last_one = 16_777_219;
    doc.edit(last_one..last_one + 1, "2")?;
    assert_eq!(doc.text(), expected);
    assert!(doc.undo()?);
    assert_eq!(doc.text(), original);
    assert!(doc.redo()?);
    assert_eq!(doc.text(), expected);
    doc.save()?;
    bytes(&path, &expected)?;
    assert_eq!(Document::open(&path)?.text(), expected);
    Ok(())
}
fn item7(f: &Fixtures) -> Result {
    assert!(Document::open(f.file("invalid-utf8", [0xff, 0xfe])?).is_err());
    let dangling = f.0.join("dangling");
    symlink("nonexistent-target", &dangling)?;
    assert!(Document::open(&dangling).is_err());
    let target = f.file("symlink-target", ORIGINAL)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640))?;
    let old = f.0.join("old-inode");
    fs::hard_link(&target, &old)?;
    let link = f.0.join("symlink");
    symlink("symlink-target", &link)?;
    let old_inode = fs::metadata(&old)?.ino();
    let names = f.names()?;
    let mut doc = Document::open(&link)?;
    doc.edit(7..9, "B")?;
    doc.save()?;
    assert!(fs::symlink_metadata(&link)?.file_type().is_symlink());
    assert_eq!(fs::read_link(&link)?, PathBuf::from("symlink-target"));
    bytes(&target, EDITED)?;
    bytes(&link, EDITED)?;
    bytes(&old, ORIGINAL)?;
    assert_eq!(fs::metadata(&old)?.ino(), old_inode);
    assert_ne!(
        fs::metadata(&target)?.ino(),
        old_inode,
        "save must replace target inode"
    );
    assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o7777, 0o640);
    assert_eq!(f.names()?, names, "save must leave no temporary siblings");
    // Retargeting the original link must not redirect an already open document.
    let other = f.file("other-target", "untouched\n")?;
    fs::remove_file(&link)?;
    symlink("other-target", &link)?;
    doc.edit(7..8, "C")?;
    doc.save()?;
    bytes(&target, "alpha\r\nCeta\nlast")?;
    bytes(&other, "untouched\n")?;
    bytes(&old, ORIGINAL)?;
    Ok(())
}
fn run() -> Result {
    let fault = match env::var("CLANKY_GATE_FAULT") {
        Err(env::VarError::NotPresent) => false,
        Ok(value) if value.is_empty() => false,
        Ok(value) if value == "accepted" => true,
        _ => return Err("unknown CLANKY_GATE_FAULT".into()),
    };
    let fixtures = Fixtures::new()?;
    item1(&fixtures)?;
    item2(&fixtures, fault)?;
    item3(&fixtures)?;
    item4(&fixtures)?;
    item5(&fixtures)?;
    item6(&fixtures)?;
    item7(&fixtures)?;
    // Report cleanup errors as gate failures, not merely stderr warnings.
    fs::remove_dir_all(&fixtures.0)?;
    std::mem::forget(fixtures);
    Ok(())
}
fn main() {
    match std::panic::catch_unwind(run) {
        Ok(Ok(())) => println!("PASS: M7 editor core"),
        Ok(Err(e)) => {
            eprintln!("FAIL: M7 editor core: {e}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("FAIL: M7 editor core assertion");
            std::process::exit(1);
        }
    }
}
