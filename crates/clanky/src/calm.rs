//! Calm inset geometry; the M11 fixture entry points remain unchanged.
use crate::look::{self,Layout,Metrics,PixelRect};
use clanky_core::layout::Layout as Tree;

mod scene;
mod recent;
pub mod settings_frame;
pub mod settings_panel;
pub use recent::{human_recent,pane_label,uptime_label};
pub use scene::{presentation,legacy_palette};

fn rect(x:f64,y:f64,w:f64,h:f64)->PixelRect{PixelRect{x:x as f32,y:y as f32,w:w.max(0.0) as f32,h:h.max(0.0) as f32}}
pub fn font_metrics(zoom:&crate::config::Zoom,scale:f64)->Result<crate::config::Metrics,String>{
    let mut metrics=zoom.metrics(scale)?;
    metrics.cell_width=(f64::from(metrics.font_px)*8.0/13.0).ceil().max(1.0) as u32;
    metrics.cell_height=(f64::from(metrics.font_px)*20.0/13.0).ceil().max(1.0) as u32;
    Ok(metrics)
}
pub fn layout(window:[u32;2],scale:f64,rails:bool,post_ai:bool,tree:&Tree,cell:[u32;2])->Layout{
    let logical=Metrics{top_bar:56.0,left_rail:256.0,right_rail:280.0,outer_gap:8.0,pane_gap:12.0,
        pane_padding:12.0,pane_padding_y:10.0,pane_header:40.0,pane_radius:10.0,
        small_font_px:12.0,ui_font_px:14.0,title_font_px:14.0,brand_font_px:14.0,
        ..Metrics::DEFAULT};
    let mut out=look::layout_with_metrics(window,scale,rails,tree,cell,&logical);
    let m=out.metrics;let [w,h]=window.map(f64::from);let margin=m.outer_gap;
    let left=if rails{m.left_rail.min(w)}else{margin.min(w)};
    let inset=rect(left,margin.min(h),(w-left-margin).max(0.0),(h-2.0*margin).max(0.0));
    let top=m.top_bar.min(inset.h as f64);
    let right=if rails && !post_ai{m.right_rail.min(inset.w as f64)}else{0.0};
    out.top_bar=rect(inset.x as f64,inset.y as f64,inset.w as f64,top);
    out.left_rail=rails.then(||rect(0.0,0.0,left,h));
    out.right_rail=(rails && !post_ai).then(||rect(inset.x as f64+inset.w as f64-right,inset.y as f64+top,right,inset.h as f64-top));
    let pad=12.0*out.scale;
    let x=(inset.x as f64+pad).min(inset.x as f64+inset.w as f64);
    let y=(inset.y as f64+top+pad).min(inset.y as f64+inset.h as f64);
    out.content=rect(x,y,inset.x as f64+inset.w as f64-right-pad-x,inset.y as f64+inset.h as f64-pad-y);
    let minimum=look::minimum(tree,&m,cell);
    out.minimum_window_px=[minimum[0]+2.0*pad+(if rails{m.left_rail}else{margin})+(if rails && !post_ai{m.right_rail}else{0.0})+margin,
        minimum[1]+2.0*pad+m.top_bar+2.0*margin].map(|n|n.ceil() as u32);
    out.fits_minimum=out.valid && out.content.w as f64>=minimum[0] && out.content.h as f64>=minimum[1];
    out.panes.clear();out.dividers.clear();
    if out.valid{look::collect(tree,out.content,&mut Vec::new(),&mut out);}
    out
}
pub fn inset(layout:&Layout)->PixelRect{
    PixelRect{x:layout.top_bar.x,y:layout.top_bar.y,w:layout.top_bar.w,
        h:(layout.window_px[1] as f32-2.0*layout.metrics.outer_gap as f32).max(0.0)}
}
pub fn drag_divider(before:&Layout,index:usize,pointer:f64)->Result<look::DragResult,String>{
    let moved=look::drag_divider(before,index,pointer)?;
    let next=layout(before.window_px,before.scale,before.rails_shown,before.right_rail.is_none() && before.rails_shown,
        &moved.workspace_layout,before.cell_px);
    Ok(look::DragResult{ratio:moved.ratio,workspace_layout:moved.workspace_layout,layout:next})
}

#[cfg(test)]
mod tests{
    use super::*;
    use clanky_core::layout::Axis;
    #[test]
    fn calm_geometry(){
        let tree=Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
        let mut shown=layout([1240,760],1.0,true,false,&tree,[8,20]);
        if std::env::var_os("CLANKY_CALM_FAULT").is_some(){shown.panes[0].rect.w+=1.0;}
        assert_eq!(inset(&shown).array(),[256.0,8.0,976.0,744.0]);
        assert_eq!(shown.panes[0].rect.array(),[268.0,76.0,672.0,326.0]);
        assert_eq!(shown.panes[1].rect.array(),[268.0,414.0,672.0,326.0]);
        assert_eq!(shown.panes[0].grid_origin,[280.0,126.0]);
        assert_eq!(shown.panes[1].grid_origin,[280.0,464.0]);
        assert!(shown.panes.iter().all(|p|(p.cols,p.rows)==(81,13)));
        let hidden=layout([1240,760],1.0,false,false,&tree,[8,20]);
        assert_eq!(hidden.panes[0].rect.array(),[20.0,76.0,1200.0,326.0]);
        assert_eq!(hidden.panes[0].cols,147);
        let moved=drag_divider(&shown,0,300.0).unwrap();
        assert_eq!(moved.layout.panes[0].rect.array(),[268.0,76.0,672.0,218.0]);
        assert_eq!(moved.layout.panes[1].rect.array(),[268.0,306.0,672.0,434.0]);
        assert_eq!((moved.layout.panes[0].rows,moved.layout.panes[1].rows),(7,18));
        let restored:Tree=serde_json::from_str(&serde_json::to_string(&moved.workspace_layout).unwrap()).unwrap();
        assert_eq!(layout([1240,760],1.0,true,false,&restored,[8,20]).panes,moved.layout.panes);
        let plain=layout([1240,760],1.0,true,true,&tree,[8,20]);
        assert_eq!(plain.panes[0].rect.array(),[268.0,76.0,952.0,326.0]);
        assert_eq!(plain.panes[0].cols,116);
    }
}
