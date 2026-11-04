use crate::{config::Config,look,settings::Settings,settings_panel::{Effect,SettingsPanel}};
use clanky_core::layout::Layout;
use std::{fs,path::PathBuf};
use winit::keyboard::{Key,NamedKey};

#[test]
fn settings_mouse_keyboard_and_pixels() {
    let root=PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("shared build directory"))
        .join(format!("settings-ui-{}",std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path=root.join("config.json");
    fs::write(&path,b"{\"animations\":false}\n").unwrap();
    let mut panel=SettingsPanel::new(Settings::open(&path).unwrap());panel.toggle();
    let layout=look::layout([1240,760],1.0,true,&Layout::leaf("A"),[10,20]);
    let theme=panel.store.config().resolved_theme(false);
    panel.draw(&layout,&theme,&[]);
    // Before execution: panel=(320,140,600,480), content x=332, width=576.
    // Opacity is row 2 at y=266; its track starts 585.44, width 311.04.
    // 60% of that track gives 0.5 + 0.6*(1.0-0.5) = 0.8.
    assert!(matches!(panel.click(772.064,282.0),Effect::Changed));panel.release();
    if std::env::var_os("CLANKY_SETTINGS_UI_FAULT").is_some(){panel.store.set("opacity",serde_json::json!(1.0)).unwrap();}
    let config=Config::load(Some(&path)).unwrap();
    assert_eq!(config.opacity_alpha(),204);
    let original=fs::read(&path).unwrap();
    panel.draw(&layout,&theme,&[]);
    // Row 3 opens the font editor. Delete all 14 characters of JetBrains Mono.
    panel.click(450.0,315.0);
    for _ in 0..14{panel.key(&Key::Named(NamedKey::Backspace),None,false);}
    panel.key(&Key::Named(NamedKey::Enter),None,false);
    assert_eq!(panel.error.as_deref(),Some("font_family must be a nonempty font family of at most 256 bytes without controls"));
    assert_eq!(fs::read(&path).unwrap(),original);
    panel.key(&Key::Named(NamedKey::Escape),None,false);
    let scene=clanky_render::Scene{clear:[15,21,34,204],layers:panel.draw(&layout,&theme,&[])};
    let pixels=clanky_render::offscreen_scene(&scene,1240,760).unwrap();
    let pixel=|x:usize,y:usize|-> [u8;4]{pixels[(y*1240+x)*4..(y*1240+x)*4+4].try_into().unwrap()};
    assert_eq!(pixel(590,44),[12,17,27,204]);
    assert_eq!(pixel(330,610),[19,27,43,255]);

    let export=root.join("chosen.json");
    panel.draw(&layout,&theme,&[]);panel.click(600.0,565.0);
    panel.insert(export.to_str().unwrap());panel.key(&Key::Named(NamedKey::Enter),None,false);
    assert_eq!(fs::read(&export).unwrap(),original);
    fs::write(&export,b"do not replace without confirmation").unwrap();
    panel.draw(&layout,&theme,&[]);panel.click(600.0,565.0);
    panel.insert(export.to_str().unwrap());panel.key(&Key::Named(NamedKey::Enter),None,false);
    assert_eq!(fs::read(&export).unwrap(),b"do not replace without confirmation");
    panel.draw(&layout,&theme,&[]);panel.click(400.0,280.0);
    assert_eq!(fs::read(&export).unwrap(),original);

    let imported=b"{\"opacity\":1,\"animations\":false,\"theme\":\"light\"}\n";
    fs::write(&export,imported).unwrap();
    panel.draw(&layout,&theme,&[]);panel.click(420.0,565.0);
    panel.insert(export.to_str().unwrap());
    assert!(matches!(panel.key(&Key::Named(NamedKey::Enter),None,false),Effect::Changed));
    assert_eq!(fs::read(&path).unwrap(),imported);
    assert_eq!(panel.store.config().resolved_theme(false).panel,[255;4]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn settings_tiny_window() {
    let path=PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").expect("shared build directory"))
        .join(format!("settings-small-{}.json",std::process::id()));
    fs::write(&path,b"{\"animations\":false}").unwrap();
    let mut panel=SettingsPanel::new(Settings::open(&path).unwrap());panel.toggle();
    let layout=look::layout([80,80],1.0,true,&Layout::leaf("A"),[10,20]);
    let scene=clanky_render::Scene{clear:[15,21,34,255],layers:panel.draw(&layout,&look::Theme::dark(),&[])};
    let pixels=clanky_render::offscreen_scene(&scene,80,80).unwrap();
    // At 80x80 only the close button fits. Its inner background at (50,50)
    // is the design's dark card colour, not a negative-width tab or glyph.
    assert_eq!(&pixels[(50*80+50)*4..(50*80+50)*4+4],&[26,34,49,255]);
    fs::remove_file(path).unwrap();
}
