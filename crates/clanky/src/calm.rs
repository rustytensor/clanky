//! Calm inset geometry; the M11 fixture entry points remain unchanged.
use crate::look::{self,Layout,Metrics,PixelRect};
use clanky_core::layout::Layout as Tree;

mod scene;
mod chrome;
pub mod palette;
pub mod agent_picker;
pub mod context_menu;
mod recent;
pub mod settings_frame;
pub mod settings_panel;
mod settings_appearance;
pub mod settings_layout;
mod settings_access;
pub mod command_bar;
pub mod breath;
pub use recent::{human_recent,pane_label,uptime_label};
pub use scene::{presentation,legacy_palette,notice,review_buttons};

/// The mockup's faint text colour: muted text 42 % of the way to the background.
pub(crate) fn scene_faint(c:&crate::themes::Colors)->crate::themes::Color{
    std::array::from_fn(|i|if i==3{255}else{(c.muted_foreground[i] as f32*0.58+c.background[i] as f32*0.42).round() as u8})
}
fn rect(x:f64,y:f64,w:f64,h:f64)->PixelRect{PixelRect{x:x as f32,y:y as f32,w:w.max(0.0) as f32,h:h.max(0.0) as f32}}
/// Terminal cells sized from the font as VTE does: width is the widest ASCII advance
/// rounded up, height the ascent and descent each rounded up; each is then multiplied
/// by its scale and truncated. The cell is worked out at the logical font size and then
/// multiplied by the display scale (rounded to whole pixels), so every scale shows the
/// same cell as scale 1, only larger; the glyphs are still drawn at the physical size.
/// `face` is [advance, ascent, descent] in em; without it (before the window's fonts load)
/// the calm 8:13 and 20:13 ratios stand in.
pub fn font_metrics(zoom:&crate::config::Zoom,scale:f64,face:Option<[f32;3]>,scales:[f32;2])->Result<crate::config::Metrics,String>{
    let mut metrics=zoom.metrics(scale)?;
    let px=f64::from(metrics.font_px)/scale;
    let (width,height)=match face{
        Some([advance,ascent,descent])=>((px*f64::from(advance)).ceil(),(px*f64::from(ascent)).ceil()+(px*f64::from(descent)).ceil()),
        None=>((px*8.0/13.0).ceil(),(px*20.0/13.0).ceil()),
    };
    let width=(width*f64::from(scales[0])).floor().max(1.0);
    let height=(height*f64::from(scales[1])).floor().max(2.0);
    metrics.cell_width=(width*scale).round().max(1.0) as u32;
    metrics.cell_height=(height*scale).round().max(2.0) as u32;
    Ok(metrics)
}

#[cfg(test)]
#[test]
fn cells_follow_vte(){
    // IBM Plex Mono: hhea 1025/-275, advance 600, 1000 units/em; 11 pt is 14.667 px at 96 dpi.
    // VTE 0.84 (Ptyxis 50.1) draws it at cell-height-scale 1.25 as 9 x 26 px cells.
    let zoom=crate::config::Zoom::new(11.0);
    let m=font_metrics(&zoom,1.0,Some([0.6,1.025,0.275]),[1.0,1.25]).unwrap();
    assert_eq!((m.cell_width,m.cell_height),(9,26));
    // Geist Mono (hhea 1005/-295, advance 600) at 11 pt: 15 + 5 = 20 px, x 1.25 = 25 px; at
    // scale 1.6 the same cell x 1.6 = 40 px (not 38 from rounding the 23.47 px font on its own).
    let m=font_metrics(&zoom,1.6,Some([0.6,1.005,0.295]),[1.0,1.25]).unwrap();
    assert_eq!((m.cell_width,m.cell_height),(14,40));
}
/// What the window shows around the panes (Settings › Appearance › Layout).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Prefs{pub tools:bool,pub vertical:bool,pub status:bool}
/// The quiet layout (mockup section 1, sizes from Marc's R1 of 2026-11-14 and Ptyxis 50.1): a
/// 46 px title bar, a 236 px tools panel and a 236 px vertical tab list on the left, a 26 px
/// status line, and panes with 14 px of padding on every side and 8 px of window colour
/// between them (and between the tools panel and the panes).
pub const TITLE_BAR:f64=46.0;
pub const SIDE_COLUMN:f64=236.0;
pub const STATUS_LINE:f64=26.0;
pub const PANE_GAP:f64=8.0;
pub const PANE_PADDING:f64=14.0;
pub fn layout(window:[u32;2],scale:f64,rails:bool,post_ai:bool,tree:&Tree,cell:[u32;2])->Layout{
    window_layout(window,scale,Prefs{tools:rails,vertical:false,status:true},post_ai,tree,cell)
}
pub fn window_layout(window:[u32;2],scale:f64,prefs:Prefs,_post_ai:bool,tree:&Tree,cell:[u32;2])->Layout{
    let logical=Metrics{top_bar:TITLE_BAR,left_rail:SIDE_COLUMN,right_rail:0.0,outer_gap:0.0,pane_gap:PANE_GAP,
        pane_padding:PANE_PADDING,pane_padding_y:PANE_PADDING,pane_header:0.0,pane_radius:0.0,
        small_font_px:12.0,ui_font_px:14.0,title_font_px:14.0,brand_font_px:14.0,
        ..Metrics::DEFAULT};
    let mut out=look::layout_with_metrics(window,scale,prefs.tools,tree,cell,&logical);
    let m=out.metrics;let [w,h]=window.map(f64::from);
    // Whole pixels at every scale: the edges between the bars, columns and panes stay crisp.
    let top=m.top_bar.round().min(h);
    let status=if prefs.status{(STATUS_LINE*out.scale).round().min((h-top).max(0.0))}else{0.0};
    let column=m.left_rail.round();
    let mut left=0.0;
    out.tabs_column=prefs.vertical.then(||{let r=rect(left,top,column.min(w),h-top-status);left+=r.w as f64;r});
    out.left_rail=prefs.tools.then(||{let r=rect(left,top,column.min((w-left).max(0.0)),h-top-status);left+=r.w as f64;r});
    // The panes keep the gap between panes from the columns on their left.
    if left>0.0{left=(left+(PANE_GAP*out.scale).round()).min(w);}
    out.right_rail=None;
    out.top_bar=rect(0.0,0.0,w,top);
    out.status_line=prefs.status.then(||rect(0.0,h-status,w,status));
    out.content=rect(left,top,(w-left).max(0.0),(h-top-status).max(0.0));
    let minimum=look::minimum(tree,&m,cell);
    out.minimum_window_px=[minimum[0]+left,minimum[1]+top+status].map(|n|n.ceil() as u32);
    out.fits_minimum=out.valid && out.content.w as f64>=minimum[0] && out.content.h as f64>=minimum[1];
    out.panes.clear();out.dividers.clear();
    if out.valid{look::collect(tree,out.content,&mut Vec::new(),&mut out);}
    out
}
/// The area the panes share (between the left columns, the title bar and the status line).
pub fn inset(layout:&Layout)->PixelRect{layout.content}
pub fn prefs(layout:&Layout)->Prefs{Prefs{tools:layout.rails_shown,vertical:layout.tabs_column.is_some(),status:layout.status_line.is_some()}}
pub fn drag_divider(before:&Layout,index:usize,pointer:f64)->Result<look::DragResult,String>{
    let moved=look::drag_divider(before,index,pointer)?;
    let next=window_layout(before.window_px,before.scale,prefs(before),false,&moved.workspace_layout,before.cell_px);
    Ok(look::DragResult{ratio:moved.ratio,workspace_layout:moved.workspace_layout,layout:next})
}

#[cfg(test)]
mod tests{
    use super::*;
    use clanky_core::layout::Axis;
    #[test]
    fn calm_geometry(){
        // Title bar 46, tools panel 236 then an 8 px gap, status line 26, pane padding 14, 8 px between panes (R1).
        let tree=Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
        let mut shown=layout([1240,760],1.0,true,false,&tree,[8,20]);
        if std::env::var_os("CLANKY_CALM_FAULT").is_some(){shown.panes[0].rect.w+=1.0;}
        assert_eq!(inset(&shown).array(),[244.0,46.0,996.0,688.0]);
        assert_eq!(shown.panes[0].rect.array(),[244.0,46.0,996.0,340.0]);
        assert_eq!(shown.panes[1].rect.array(),[244.0,394.0,996.0,340.0]);
        assert_eq!(shown.panes[0].grid_origin,[258.0,60.0]);
        assert_eq!(shown.panes[1].grid_origin,[258.0,408.0]);
        assert!(shown.panes.iter().all(|p|(p.cols,p.rows)==(121,15)));
        let hidden=layout([1240,760],1.0,false,false,&tree,[8,20]);
        assert_eq!(hidden.panes[0].rect.array(),[0.0,46.0,1240.0,340.0]);
        assert_eq!(hidden.panes[0].cols,151);
        let restored:Tree=serde_json::from_str(&serde_json::to_string(&drag_divider(&shown,0,300.0).unwrap().workspace_layout).unwrap()).unwrap();
        assert_eq!(layout([1240,760],1.0,true,false,&restored,[8,20]).panes,drag_divider(&shown,0,300.0).unwrap().layout.panes);
        let both=window_layout([1240,760],1.0,Prefs{tools:true,vertical:true,status:false},false,&Tree::leaf("A"),[8,20]);
        assert_eq!(both.panes[0].rect.array(),[480.0,46.0,760.0,714.0]);
    }
}
