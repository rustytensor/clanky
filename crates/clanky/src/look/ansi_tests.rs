use super::*;
use clanky_core::{Cell, ColorSource, snapshot_from_chunks};
use clanky_render::{GridLayer, Layer, Scene, offscreen_scene};

#[test]
fn light_ansi_contrast() {
    // WCAG 2 relative luminance, against the fixed light panel #ffffff.
    // Independently calculated before this run: minimum 4.860:1 (cyan 6).
    let mut theme=Theme::light();
    if std::env::var_os("CLANKY_ANSI_FAULT").is_some(){theme.ansi[6]=[255;3];}
    let linear=|value:u8| {let s=value as f64/255.0;if s<=0.04045{s/12.92}else{((s+0.055)/1.055).powf(2.4)}};
    for index in (1..16).filter(|i| ![7,15].contains(i)) {
        let [r,g,b]=theme.ansi[index];
        let luminance=0.2126*linear(r)+0.7152*linear(g)+0.0722*linear(b);
        let contrast=1.05/(luminance+0.05);
        println!("ANSI {index}: {contrast:.3}:1");
        assert!(contrast>=4.5,"ANSI {index}: {contrast:.3}:1 is below 4.5:1");
    }
    // Literal palette from the pre-change core indexed() table, not from Theme.
    assert_eq!(Theme::dark().ansi,[
        [0,0,0],[255,0,0],[0,205,0],[205,205,0],[0,0,238],[205,0,205],[0,205,205],[229,229,229],
        [127,127,127],[255,0,0],[0,255,0],[255,255,0],[92,92,255],[255,0,255],[0,255,255],[255,255,255],
    ]);
}

#[test]
fn light_ansi_color_sources() {
    // Input and expectations fixed before the run:
    // truecolour 229 stays 229; ANSI cyan becomes (11,127,120); default becomes
    // (26,34,51). Explicit black background stays black; inverse default is white.
    let bytes="\x1b[38;2;229;229;229m█\x1b[36m█\x1b[39m█\x1b[48;2;0;0;0m \x1b[0;7m█";
    let raw=snapshot_from_chunks(1,5,&[bytes.as_bytes()]).unwrap();
    assert_eq!([raw.cells[0].fg,raw.cells[1].fg,raw.cells[2].fg],[[229;3],[0,205,205],[229;3]]);
    assert_eq!([raw.cells[0].color_source.unwrap().fg,raw.cells[1].color_source.unwrap().fg,raw.cells[2].color_source.unwrap().fg],
        [ColorSource::Truecolor,ColorSource::Indexed(6),ColorSource::Default]);
    assert_eq!(raw.cells[0].bg,[0;3]);
    let mut shown=raw.clone();shown.cursor=None;
    let light=Theme::light();
    for cell in &mut shown.cells{light.terminal_cell(cell);}
    assert_eq!([shown.cells[0].fg,shown.cells[1].fg,shown.cells[2].fg],[[229;3],[11,127,120],[26,34,51]]);
    assert_eq!(shown.cells[2].bg,[255;3]);
    // The scene renderer now uses the M12 8×20 cells; sample their centres.
    // The five independently fixed colour expectations remain unchanged.
    let scene=Scene{clear:[255;4],layers:vec![Layer::Grid(GridLayer{
        origin:[0.0,0.0],clip:[0.0,0.0,40.0,20.0],snapshot:shown.clone(),underlines:vec![],opacity:1.0,
    })]};
    let pixels=offscreen_scene(&scene,40,20).unwrap();
    for (x,expected) in [(4,[229,229,229,255]),(12,[11,127,120,255]),(20,[26,34,51,255]),(28,[0,0,0,255]),(36,[255;4])] {
        let start=(10*40+x)*4;
        assert_eq!(&pixels[start..start+4],&expected,"cell centre {x},10");
    }
    // Recolour the same cells without feeding the terminal again.
    for cell in &mut shown.cells{Theme::dark().terminal_cell(cell);}
    assert_eq!([shown.cells[0].fg,shown.cells[1].fg,shown.cells[2].fg],[[229;3],[0,205,205],[219,228,243]]);
    assert_eq!(shown.cells[4].bg,[229;3]);
    for cell in &mut shown.cells{light.terminal_cell(cell);}
    assert_eq!(shown.cells[1].fg,[11,127,120]);
    assert_eq!(raw.cells[1].fg,[0,205,205]);
    // Old RGB-only cells round-trip without gaining an optional field or being guessed at.
    let old=serde_json::json!({"ch":"A","fg":[229,229,229],"bg":[0,0,0]});
    let mut legacy:Cell=serde_json::from_value(old.clone()).unwrap();
    light.terminal_cell(&mut legacy);
    assert_eq!(serde_json::to_value(legacy).unwrap(),old);
}
