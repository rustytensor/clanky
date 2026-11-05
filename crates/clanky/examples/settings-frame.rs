//! Render the rebuilt Settings frame at 2× for comparison with the HTML reference.
use std::{io::Write,path::PathBuf};
fn main()->Result<(),String>{
    let mut args=std::env::args().skip(1);
    let id=args.next().ok_or("usage: settings-frame THEME OUTPUT.rgba")?;
    let output=PathBuf::from(args.next().ok_or("missing output path")?);
    if args.next().is_some(){return Err("too many arguments".into());}
    let store=clanky::theme_store::ThemeStore::open(output.parent().ok_or("output needs a parent")?.join("settings-frame-themes"))?;
    let selected=store.get(&id)?;
    let mut config=clanky::config::Config::window_defaults();
    config.theme=match selected.appearance{clanky::themes::Appearance::Dark=>clanky::config::ThemePreference::Dark,clanky::themes::Appearance::Light=>clanky::config::ThemePreference::Light};
    match selected.appearance{clanky::themes::Appearance::Dark=>config.theme_dark=id,clanky::themes::Appearance::Light=>config.theme_light=id};
    let theme=config.resolved_document(&store,false)?;
    let mut frame=clanky::calm::settings_frame::draw([1800,1440],2.0,clanky::calm::settings_frame::Section::Terminal,&theme,&config,(-1.0,-1.0),false,&mut clanky_render::measure_text);
    frame.scene.clear=theme.colors.sidebar;
    let pixels=clanky_render::offscreen_scene(&frame.scene,1800,1440)?;
    std::fs::OpenOptions::new().write(true).create_new(true).open(output).and_then(|mut file|file.write_all(&pixels)).map_err(|error|error.to_string())?;
    println!("1800×1440 RGBA; panel crop is (120,120)–(1680,1320). Frame preview only; live Settings remains hidden.");
    Ok(())
}
