use super::*;
use crate::config::{Config, StartupOverrides};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn recreated_theme_clears_old_error() {
    let root = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap())
        .join(format!("theme-recreated-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let path = root.join("repaired.json");
    fs::write(&path, b"{}").unwrap();
    let mut store = ThemeStore::open(&root).unwrap();
    assert_eq!(store.issues().len(), 1);
    fs::remove_file(&path).unwrap();
    store.save_new("repaired", &crate::themes::builtin("stone-dark").unwrap()).unwrap();
    assert!(store.issues().is_empty());
    assert_eq!(store.get("user:repaired").unwrap().colors.background, [26, 24, 22, 255]);
    assert!(!store.reload().unwrap());
}

#[test]
fn theme_store_round_trip() {
    let root = PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR").expect("use the shared build directory"),
    )
    .join(format!("theme-store-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let folder = root.join("themes");
    let mut store = ThemeStore::open(&folder).unwrap();
    assert!(!folder.exists());
    assert_eq!(store.picker(None).unwrap().len(), 13);
    assert!(store.issues().is_empty());

    // Input is the literal approved Stone document, with three explicit edits.
    // Expected custom background is (1,2,3,255); exports must preserve these bytes.
    let source = include_str!("../../../assets/themes/stone-dark.json")
        .replace("\"Stone\"", "\"Workshop\"")
        .replace("\"background\": \"#1a1816\"", "\"background\": \"#010203\"");
    let incoming = root.join("incoming.json");
    fs::write(&incoming, &source).unwrap();
    let id = store.import(&incoming, "workshop").unwrap();
    assert_eq!(id, "user:workshop");
    assert_eq!(store.get(&id).unwrap().colors.background, [1, 2, 3, 255]);
    assert_eq!(
        fs::read(folder.join("workshop.json")).unwrap(),
        source.as_bytes()
    );
    let picker = store.picker(Some(Appearance::Dark)).unwrap();
    let custom = picker.iter().find(|entry| entry.id == id).unwrap();
    assert_eq!(custom.name, "Workshop");
    assert_eq!(custom.swatches.background, [1, 2, 3, 255]);
    assert!(!custom.builtin);
    assert_eq!(custom.swatches.agent, [210, 164, 118, 255]);

    let export = root.join("export.json");
    store.export(&id, &export).unwrap();
    if std::env::var_os("CLANKY_THEME_STORE_FAULT").is_some() {
        fs::write(&export, b"corrupted export").unwrap();
    }
    assert_eq!(fs::read(&export).unwrap(), source.as_bytes());
    let imported_id = store.import(&export, "imported").unwrap();
    assert_eq!(
        fs::read(folder.join("imported.json")).unwrap(),
        source.as_bytes()
    );
    assert_eq!(store.get(&imported_id).unwrap().name, "Workshop");
    assert!(store.export(&id, &export).is_err());
    assert!(store.import(&incoming, "workshop").is_err());
    assert!(store.import(&incoming, "../escape").is_err());
    assert!(!root.join("escape.json").exists());
    assert_eq!(fs::read(&export).unwrap(), source.as_bytes());

    let mut copy = store.get("stone-dark").unwrap();
    copy.name = "Personal Stone".into();
    copy.colors.agent = [10, 20, 30, 255];
    let saved = store.save_new("personal", &copy).unwrap();
    assert_eq!(store.get(&saved).unwrap().colors.agent, [10, 20, 30, 255]);
    assert_eq!(
        store.get("stone-dark").unwrap().colors.agent,
        [210, 164, 118, 255]
    );

    let path = folder.join("workshop.json");
    fs::write(&path, source.replace("#010203", "#040506")).unwrap();
    assert!(store.reload().unwrap());
    assert_eq!(store.get(&id).unwrap().colors.background, [4, 5, 6, 255]);
    assert!(!store.reload().unwrap());
    fs::write(&path, b"{\"name\":\"broken\"}").unwrap();
    assert!(store.reload().unwrap());
    assert_eq!(store.issues()[0].path, path);
    assert_eq!(
        store.issues()[0].message,
        "theme missing required field: appearance"
    );
    assert_eq!(store.get(&id).unwrap().colors.background, [4, 5, 6, 255]);
    assert!(!store.reload().unwrap());
    let retained = root.join("retained.json");
    store.export(&id, &retained).unwrap();
    assert_eq!(
        fs::read(&retained).unwrap(),
        source.replace("#010203", "#040506").as_bytes()
    );
    assert!(store.import(&path, "broken").is_err());
    assert!(!folder.join("broken.json").exists());
    fs::write(&path, &source).unwrap();
    assert!(store.reload().unwrap());
    assert!(store.issues().is_empty());

    let config = Config::from_json(r##"{"theme":"dark","theme_dark":"user:workshop","theme_light":"paper-light","post_ai":true,"colors":{"agent":"#10203080"}}"##).unwrap();
    assert!(config.post_ai);
    assert_eq!(config.selected_theme_id(true), "user:workshop");
    let resolved = config.resolved_document(&store, true).unwrap();
    assert_eq!(resolved.colors.background, [1, 2, 3, 255]);
    assert_eq!(resolved.colors.agent, [16, 32, 48, 128]);
    assert_eq!(store.get(&id).unwrap().colors.agent, [210, 164, 118, 255]);
    assert_eq!(
        Config::from_json(r#"{"post_ai":"true"}"#).unwrap_err(),
        "post_ai must be a boolean"
    );
    assert!(Config::from_json(r#"{"theme_dark":"paper-light"}"#).is_err());
    assert!(Config::from_json(r#"{"theme_dark":"user:../bad"}"#).is_err());
    let mut flags = StartupOverrides::default();
    assert!(!flags.parse_flag("--post-aix"));
    assert!(flags.parse_flag("--post-ai"));
    let mut reloaded = Config::from_json(r#"{"post_ai":false}"#).unwrap();
    flags.apply(&mut reloaded);
    assert!(reloaded.post_ai);
    assert!(!Config::default().post_ai);

    // The existing Settings API accepts the new keys and semantic RGBA overrides.
    let mut settings = crate::settings::Settings::open(root.join("config.json")).unwrap();
    settings
        .set("theme_dark", serde_json::json!("user:workshop"))
        .unwrap();
    settings.set("post_ai", serde_json::json!(true)).unwrap();
    settings.set_color("agent", Some("#10203080")).unwrap();
    assert_eq!(
        settings
            .config()
            .resolved_document(&store, false)
            .unwrap()
            .colors
            .agent,
        [16, 32, 48, 128]
    );
    assert!(settings.config().post_ai);

    fs::remove_file(&path).unwrap();
    assert!(store.reload().unwrap());
    assert!(store.get(&id).is_err());
    assert!(config.resolved_document(&store, false).is_err());
    assert!(
        !store
            .picker(None)
            .unwrap()
            .iter()
            .any(|entry| entry.id == id)
    );
    println!(
        "PASS: custom save, exact export/import, picker swatches, valid/invalid/deleted reloads, semantic overrides and Post-AI parsing"
    );
}
