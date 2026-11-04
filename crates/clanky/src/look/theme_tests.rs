use super::*;
use crate::config::{Config, Zoom};
use clanky_core::{layout::{Axis, Layout as Tree}, snapshot_from_chunks};
use clanky_render::offscreen_scene;

#[test]
fn light_theme_pixels_and_config() {
    let tree = Tree::branch(Axis::Rows, Tree::leaf("A"), Tree::leaf("B"));
    let layout = layout([1240,760],1.0,true,&tree,[10,20]);
    let a = PaneState::new("A",snapshot_from_chunks(14,71,&[]).unwrap());
    let mut b = PaneState::new("B",snapshot_from_chunks(14,71,&[b"cargo build"]).unwrap());
    b.blocks.push(BlockCard { id:"build".into(),first_row:0,last_row:0,command:"cargo build".into(),exit_code:Some(0),duration_ms:None,running:false });
    let config = Config::from_json(r##"{"theme":"light","font_size":16}"##).unwrap();
    let mut state = LookState { focused:Some("B".into()),panes:vec![a,b],theme:config.resolved_theme(false),..LookState::default() };
    if std::env::var_os("CLANKY_THEME_FAULT").is_some() { state.theme = Theme::dark(); }
    let pixel = |pixels: &[u8], x: usize, y: usize| -> [u8;4] {
        pixels[(y*1240+x)*4..(y*1240+x)*4+4].try_into().unwrap()
    };
    let pixels = offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
    // Literal design section 7b: these values predate this implementation.
    for (x,y,expected) in [(590,44,[219,224,232,240]),(950,380,[255,255,255,255]),
        (221,51,[219,224,232,240]),(226,56,[255,255,255,255]),
        (100,700,[243,246,251,255]),(1100,700,[243,246,251,255]),
        (230,451,[246,246,247,255]),(936,451,[224,234,229,255])] {
        let got = pixel(&pixels,x,y);
        assert!(got.iter().zip(expected).all(|(a,b)| (*a as i16-b as i16).abs()<=3),"({x},{y}) {got:?} expected {expected:?}");
    }
    let glow = pixel(&pixels,215,577);
    assert!(glow[2] as i16-glow[0] as i16>=21,"focus glow {glow:?}");
    state.panes[0].allow_agent = true; state.panes[0].agent_running = true;
    let pixels = offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
    let glow = pixel(&pixels,215,222);
    assert!(glow[1]<=214 && glow[2] as i16-glow[1] as i16>=18,"agent glow {glow:?}");

    let system = Config::default();
    assert_eq!(system.resolved_theme(false).panel,[19,27,43,255]);
    assert_eq!(system.resolved_theme(true).panel,[255;4]);
    assert_eq!(Config::from_json(r#"{"theme":"dark"}"#).unwrap().resolved_theme(true).panel,[19,27,43,255]);
    assert!(!crate::platform::preference_is_light(None));
    let colors: serde_json::Map<String,serde_json::Value> = COLOR_TOKENS.iter()
        .map(|key| ((*key).into(),serde_json::json!("#202020"))).collect();
    let overrides = Config::from_json(&serde_json::json!({"colors":colors}).to_string()).unwrap();
    assert_eq!(overrides.colors.len(),14);
    let overridden = overrides.resolved_theme(false);
    assert_eq!([overridden.window,overridden.rail,overridden.panel,overridden.line,
        overridden.ink,overridden.dim,overridden.accent,overridden.violet,overridden.teal,
        overridden.ok,overridden.err,overridden.focus,overridden.ok_ink,overridden.err_ink],
        [[32,32,32,255];14]);
    state.theme = Config::from_json(r##"{"theme":"light","colors":{"panel":"#202020"}}"##).unwrap().resolved_theme(false);
    let pixels = offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
    assert_eq!(pixel(&pixels,950,380),[32,32,32,255]);
    for invalid in [r#"{"theme":"blue"}"#,r##"{"colors":{"unknown":"#202020"}}"##,
        r#"{"colors":{"panel":"202020"}}"#,r#"{"font_size":7}"#,r#"{"font_size":33}"#] {
        assert!(Config::from_json(invalid).is_err());
    }
    let config = Config::from_json(r#"{"font_size":20,"font_size_pt":18}"#).unwrap();
    assert_eq!(config.font_size_pt,15.0);
    assert_eq!(Zoom::new(config.font_size_pt).metrics(1.0).unwrap().font_px,20.0);
    assert_eq!(Config::from_json(r#"{"font_size_pt":18}"#).unwrap().font_size,24.0);
}
