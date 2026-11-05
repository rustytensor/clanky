//! Real theme inputs for the contrast script and the storage round-trip gate.
use clanky::{theme_store::ThemeStore, themes};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn storage() -> Result<(), String> {
    let target =
        PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").ok_or("CARGO_TARGET_DIR required")?);
    let root = target.join(format!("theme-storage-gate-{}", std::process::id()));
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _scratch = Scratch(root.clone());
    let folder = root.join("themes");
    let mut store = ThemeStore::open(&folder)?;
    let mut theme = themes::builtin("stone-dark")?;
    theme.name = "Gate Custom".into();
    theme.colors.background = [1, 2, 3, 255];
    let id = store.save_new("custom", &theme)?;
    require(id == "user:custom", "save returned the wrong ID")?;
    let saved = fs::read(folder.join("custom.json")).map_err(|e| e.to_string())?;
    let document: Value = serde_json::from_slice(&saved).map_err(|e| e.to_string())?;
    require(
        document["name"] == "Gate Custom" && document["colors"]["background"] == "#010203",
        "saved custom values differ",
    )?;
    require(
        themes::builtin("stone-dark")?.colors.background == [26, 24, 22, 255],
        "saving a copy changed Stone",
    )?;

    let export = root.join("export.json");
    store.export(&id, &export)?;
    if std::env::var("CLANKY_THEME_GATE_FAULT").as_deref() == Ok("storage") {
        let mut corrupt = saved.clone();
        corrupt.push(b'\n');
        fs::write(&export, corrupt).map_err(|e| e.to_string())?;
    }
    let exported = fs::read(&export).map_err(|e| e.to_string())?;
    require(saved == exported, "export bytes differ from saved file")?;
    let imported = store.import(&export, "imported")?;
    require(imported == "user:imported", "import returned the wrong ID")?;
    require(
        fs::read(folder.join("imported.json")).map_err(|e| e.to_string())? == saved,
        "import bytes differ from saved file",
    )?;
    let reexport = root.join("reexport.json");
    store.export(&imported, &reexport)?;
    require(
        fs::read(&reexport).map_err(|e| e.to_string())? == saved,
        "re-export bytes differ",
    )?;
    let reopened = ThemeStore::open(&folder)?.get(&imported)?;
    require(
        reopened.name == "Gate Custom" && reopened.colors.background == [1, 2, 3, 255],
        "reopened imported values differ",
    )?;

    let occupied = root.join("occupied.json");
    let sentinel = b"owner's existing file\n";
    fs::write(&occupied, sentinel).map_err(|e| e.to_string())?;
    require(
        store.export(&id, &occupied).is_err(),
        "export overwrote an existing path",
    )?;
    require(
        fs::read(&occupied).map_err(|e| e.to_string())? == sentinel,
        "refused export changed existing bytes",
    )?;
    theme.colors.background = [4, 5, 6, 255];
    require(
        store.save_new("custom", &theme).is_err(),
        "save overwrote an existing theme",
    )?;
    require(
        store.import(&export, "custom").is_err(),
        "import overwrote an existing theme",
    )?;
    require(
        fs::read(folder.join("custom.json")).map_err(|e| e.to_string())? == saved,
        "refused writes changed saved bytes",
    )?;
    println!(
        "PASS: theme storage save/export/import/re-export byte-identical; all overwrites refused"
    );
    Ok(())
}
fn run() -> Result<(), String> {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [mode] if mode == "--palettes" => {
            let mut documents = Vec::new();
            for builtin in themes::BUILTINS {
                let theme: Value =
                    serde_json::from_str(&builtin.load()?.to_json()?).map_err(|e| e.to_string())?;
                documents.push(json!({"id":builtin.id,"theme":theme}));
            }
            println!("{}", Value::Array(documents));
            Ok(())
        }
        [mode] if mode == "--storage" => storage(),
        _ => Err("use --palettes or --storage".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL: theme gate: {error}");
        std::process::exit(1);
    }
}
