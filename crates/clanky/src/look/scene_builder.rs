//! Pure Studio presentation: drawing and hit targets share physical rectangles.
use super::{ease_out_cubic, Layout, PixelRect, Theme};
use clanky_core::TerminalSnapshot;
use clanky_render::{Fill, GridLayer, Layer, Scene, Shadow, Shape, TextRun, Underline};


#[derive(Clone, Debug)]
pub struct LookState {
    pub theme: Theme,
    pub opacity: f32,
    pub animations: bool,
    pub elapsed_ms: f32,
    pub focused: Option<String>,
    pub panes: Vec<PaneState>,
    pub sessions: Vec<SessionItem>,
    pub agents: Vec<AgentItem>,
    pub files: Vec<FileItem>,
    pub timeline: Vec<TimelineItem>,
    pub activity: Vec<f32>,
    pub uptime_seconds: u64,
    pub masking_ready: bool,
    pub panels: Vec<PanelState>,
}
impl Default for LookState {
    fn default() -> Self {
        Self { theme: Theme::dark(), opacity: 0.94, animations: true, elapsed_ms: 0.0, focused: None,
            panes: vec![], sessions: vec![], agents: vec![], files: vec![],
            timeline: vec![], activity: vec![], uptime_seconds: 0,
            masking_ready: false, panels: vec![] }
    }
}
#[derive(Clone, Debug)]
pub struct PaneState {
    pub id: String,
    pub title: String,
    pub driver: String,
    pub allow_agent: bool,
    pub agent_running: bool,
    pub snapshot: TerminalSnapshot,
    pub underlines: Vec<Underline>,
    pub blocks: Vec<BlockCard>,
    pub cursor: Option<CursorMotion>,
    pub scroll_px: f32,
}
impl PaneState {
    pub fn new(id: impl Into<String>, snapshot: TerminalSnapshot) -> Self {
        Self { id: id.into(), title: String::new(), driver: String::new(),
            allow_agent: false, agent_running: false, snapshot, underlines: vec![],
            blocks: vec![], cursor: None, scroll_px: 0.0 }
    }
}
#[derive(Clone, Debug)]
pub struct BlockCard {
    pub id: String,
    pub first_row: usize,
    pub last_row: usize,
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub running: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct CursorMotion { pub from: [f32; 2], pub to: [f32; 2], pub elapsed_ms: f32 }
#[derive(Clone, Debug)]
pub struct SessionItem { pub id: String, pub title: String, pub active: bool }
#[derive(Clone, Debug)]
pub struct AgentItem {
    pub connection_id: u64, pub peer_pid: u32, pub tab_id: Option<String>, pub running: bool,
}
#[derive(Clone, Debug)]
pub struct FileItem { pub id: String, pub title: String, pub pending: usize, pub active: bool }
#[derive(Clone, Debug)]
pub struct TimelineItem { pub label: String, pub detail: String }
#[derive(Clone, Debug)]
pub struct PanelState {
    pub id: String, pub rect: PixelRect, pub title: String, pub lines: Vec<PanelLine>,
    pub snapshot: Option<TerminalSnapshot>, pub underlines: Vec<Underline>,
    pub opacity: f32, pub rise_px: f32,
}
#[derive(Clone, Debug)]
pub struct PanelLine { pub text: String, pub selected: bool, pub action: Option<HitAction> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitAction {
    Palette, ToggleRails, Session(String), File(String),
    Agent { connection_id: u64, tab_id: Option<String> }, Access(String),
    Block { tab_id: String, block_id: String, action: String }, PaletteChoice(usize),
}
#[derive(Clone, Debug)]
pub struct Hotspot { pub rect: PixelRect, pub action: HitAction }
#[derive(Clone, Debug)]
pub struct Presentation { pub scene: Scene, pub hits: Vec<Hotspot> }

fn rect(x: f32, y: f32, w: f32, h: f32) -> PixelRect {
    PixelRect { x, y, w: w.max(0.0), h: h.max(0.0) }
}
fn shape(out: &mut Presentation, r: PixelRect, radius: f32, fill: [u8; 4], border: Option<(f32, [u8; 4])>, shadow: Option<Shadow>) {
    if r.w > 0.0 && r.h > 0.0 {
        out.scene.layers.push(Layer::Shape(Shape { rect: r.array(), radius,
            fill: Fill::Solid(fill), border, shadow }));
    }
}
/// Inter's widest glyph fits inside one em, including the ellipsis. Reserve
/// one em per scalar (two for non-ASCII); never rely on renderer text clipping.
fn bounded(text: &str, width: f32, px: f32) -> String {
    let budget = (width / px.max(1.0)).floor().max(0.0) as usize;
    let chars: Vec<char> = text.split(['\n', '\r']).next().unwrap_or("").chars().take(8192).collect();
    let cost = |c: char| if c.is_ascii() { 1 } else { 2 };
    if chars.iter().map(|c| cost(*c)).sum::<usize>() <= budget { return chars.into_iter().collect(); }
    if budget < 2 { return String::new(); }
    let mut result = String::new();
    let mut used = 0;
    for c in chars { if used + cost(c) > budget - 2 { break; } used += cost(c); result.push(c); }
    result.push('…'); result
}
fn text(out: &mut Presentation, r: PixelRect, value: &str, px: f32, color: [u8; 4], weight: u16) {
    if r.h < px * 1.25 { return; }
    let value = bounded(value, r.w, px);
    if !value.is_empty() { out.scene.layers.push(Layer::Text(TextRun {
        origin: [r.x, r.y], text: value, px, color, weight, mono: false, opacity: 1.0,
    })); }
}
fn hit(out: &mut Presentation, r: PixelRect, action: HitAction) {
    if r.w > 0.0 && r.h > 0.0 { out.hits.push(Hotspot { rect: r, action }); }
}
fn chip(out: &mut Presentation, t: &Theme, m: &super::Metrics, r: PixelRect, label: &str, color: [u8; 4], action: Option<HitAction>) {
    let pad = m.pill_padding as f32;
    let inset = m.chip_inset_y as f32;
    shape(out, r, r.h / 2.0, t.chip_neutral, Some((m.border as f32, t.line)), None);
    text(out, rect(r.x+pad, r.y+inset, r.w-2.0*pad, r.h-inset), label, m.small_font_px as f32, color, 600);
    if let Some(action) = action { hit(out, r, action); }
}
fn row(out: &mut Presentation, t: &Theme, m: &super::Metrics, r: PixelRect, label: &str, active: bool, color: [u8;4], action: HitAction) {
    let inset = m.control_gap as f32;
    if active {
        shape(out, r, m.item_radius as f32, t.selection, None, None);
        shape(out, rect(r.x,r.y+inset,m.border as f32*2.0,r.h-2.0*inset),m.border as f32,color,None,None);
    }
    text(out,rect(r.x+inset,r.y+m.chip_inset_y as f32,r.w-2.0*inset,r.h-m.chip_inset_y as f32),label,m.small_font_px as f32,if active { t.ink } else { t.dim },500);
    hit(out,r,action);
}
pub fn scene(layout: &Layout, state: &LookState) -> Scene { presentation(layout,state).scene }

pub fn presentation(layout: &Layout, state: &LookState) -> Presentation {
    let t = &state.theme;
    let m = &layout.metrics;
    let p = m.pane_padding as f32;
    let mut out = Presentation { scene: Scene { clear: Theme::fade(t.window,state.opacity),layers:vec![] },hits:vec![] };
    chrome(&mut out,layout,state);
    for pane in &layout.panes {
        let data=state.panes.iter().find(|p| p.id==pane.id);
        let focused=state.focused.as_deref()==Some(pane.id.as_str());
        let agent=data.is_some_and(|p| p.agent_running);
        let edge=if agent { t.violet } else if focused { t.focus } else { t.line };
        // The saturated near-edge violet is intentional: its red/green token
        // separation is only 28, and the design requires >=20 after blending.
        let shadow=(focused || agent).then_some(Shadow {
            offset:[0.0,0.0],blur:if agent { m.agent_glow_blur } else { m.focus_glow_blur } as f32,
            spread:if agent { m.agent_glow_spread } else { m.focus_glow_spread } as f32,
            color:if agent { t.agent_glow } else { t.focus_glow },
        });
        shape(&mut out,pane.rect,m.pane_radius as f32,t.panel,Some((m.border as f32,edge)),shadow);
        let Some(data)=data else { continue; };
        let access_width=m.small_font_px as f32*"Agent off".len() as f32+2.0*m.pill_padding as f32;
        let access=rect(pane.rect.x+pane.rect.w-p-access_width,pane.rect.y+p,access_width,m.pill_height as f32);
        chip(&mut out,t,m,access,if data.allow_agent { "Agent on" } else { "Agent off" },if data.allow_agent { t.violet } else { t.dim },Some(HitAction::Access(data.id.clone())));
        let title_width=(access.x-pane.rect.x-2.0*p-m.control_gap as f32).max(0.0);
        text(&mut out,rect(pane.rect.x+p,pane.rect.y+p,title_width*0.62,m.pane_header as f32),&data.title,m.title_font_px as f32,t.ink,600);
        text(&mut out,rect(pane.rect.x+p+title_width*0.64,pane.rect.y+p,title_width*0.34,m.pane_header as f32),&data.driver,m.small_font_px as f32,if agent { t.violet } else { t.dim },500);
        let scroll=if data.scroll_px.is_finite() { data.scroll_px } else { 0.0 };
        let origin=[pane.grid_origin[0],pane.grid_origin[1]-scroll];
        for block in &data.blocks { block_card(&mut out,layout,state,pane,data,block,origin); }
        let mut snapshot=data.snapshot.clone(); snapshot.cursor=None;
        for cell in &mut snapshot.cells { cell.fg=t.terminal_color(cell.fg,false); }
        out.scene.layers.push(Layer::Grid(GridLayer { origin,clip:pane.grid_clip.array(),snapshot,underlines:data.underlines.clone(),opacity:1.0 }));
        if focused {
            let position=if let Some(motion)=data.cursor {
                Some([ease_out_cubic(motion.from[0],motion.to[0],motion.elapsed_ms,m.cursor_ms,state.animations),
                    ease_out_cubic(motion.from[1],motion.to[1],motion.elapsed_ms,m.cursor_ms,state.animations)])
            } else { data.snapshot.cursor.map(|(row,col)| [origin[0]+col as f32*layout.cell_px[0] as f32,origin[1]+row as f32*layout.cell_px[1] as f32]) };
            if let Some([x,y])=position {
                let cursor=intersect(rect(x,y+layout.cell_px[1] as f32-m.cursor_inset_y as f32,layout.cell_px[0] as f32,m.cursor_width as f32),pane.grid_clip);
                shape(&mut out,cursor,m.cursor_radius as f32,t.accent,None,None);
            }
        }
    }
    for panel in &state.panels { panel_card(&mut out,layout,state,panel); }
    out
}
fn intersect(a: PixelRect,b: PixelRect) -> PixelRect {
    let x=a.x.max(b.x); let y=a.y.max(b.y);
    rect(x,y,(a.x+a.w).min(b.x+b.w)-x,(a.y+a.h).min(b.y+b.h)-y)
}
fn block_card(out: &mut Presentation,layout: &Layout,state: &LookState,pane: &super::PaneLayout,data: &PaneState,block: &BlockCard,origin:[f32;2]) {
    if block.last_row<block.first_row || block.first_row>=data.snapshot.rows { return; }
    let t=&state.theme; let m=&layout.metrics;
    let last=block.last_row.min(data.snapshot.rows.saturating_sub(1));
    let ch=layout.cell_px[1] as f32; let inset=m.card_inset as f32;
    let y=origin[1]+block.first_row as f32*ch;
    let card=intersect(rect(origin[0]-inset,y-inset,pane.grid_clip.w+2.0*inset,(last-block.first_row+1) as f32*ch+2.0*inset),
        rect(pane.grid_clip.x-inset,pane.grid_clip.y-inset,pane.grid_clip.w+2.0*inset,pane.grid_clip.h+2.0*inset));
    if card.h<=0.0 { return; }
    shape(out,card,m.block_radius as f32,t.card,block.running.then_some((m.border as f32,t.violet)),None);
    if y<pane.grid_clip.y || y+ch>pane.grid_clip.y+pane.grid_clip.h { return; }
    let mut right=card.x+card.w-m.chip_right_gap as f32;
    let dy=m.chip_inset_y as f32;
    if let Some(code)=block.exit_code {
        let label=if code==0 { "✓ 0".into() } else { format!("✗ {code}") };
        let color=if code==0 { t.ok_ink } else { t.err_ink };
        let font=m.chip_font_px as f32;
        let width=label.chars().map(|c| if c.is_ascii() { 1.0 } else { 2.0 }).sum::<f32>()*font+2.0*m.chip_padding_x as f32;
        let pill=rect(right-width,y+dy,width,ch-2.0*dy);
        if pill.x>=card.x {
            shape(out,pill,pill.h/2.0,if code==0 { t.chip_ok } else { t.chip_err },None,None);
            text(out,rect(pill.x+m.chip_padding_x as f32,pill.y,pill.w-2.0*m.chip_padding_x as f32,pill.h),&label,font,color,600);
            right=pill.x-m.control_gap as f32;
        }
    }
    if let Some(ms)=block.duration_ms {
        let width=m.small_font_px as f32*8.0;
        text(out,rect((right-width).max(card.x),y+dy,width,ch-2.0*dy),&format!("{:.2}s",ms as f64/1000.0),m.small_font_px as f32,t.dim,500);
        right-=width+m.control_gap as f32;
    }
    if block.running {
        let pulse=if state.animations { 0.65+0.35*(state.elapsed_ms/m.pulse_ms).sin().abs() } else { 1.0 };
        let diameter=m.chip_right_gap as f32;
        shape(out,rect(right-diameter,y+(ch-diameter)/2.0,diameter,diameter),diameter/2.0,Theme::fade(t.violet,pulse),None,None);
    }
    // Actual command/output are painted only by the original, unmodified grid.
    let actions=[("copy","Copy"),("rerun","Run"),("send","Agent"),("collapse","Fold")];
    let width=m.pill_height as f32*2.0;
    let controls_width=actions.len() as f32*width;
    if right-controls_width>card.x+pane.grid_clip.w/2.0 {
        for (i,(action,label)) in actions.iter().enumerate() {
            let r=rect(right-controls_width+i as f32*width,y+dy,width-m.control_gap as f32,ch-2.0*dy);
            text(out,r,label,m.small_font_px as f32,t.dim,500);
            hit(out,r,HitAction::Block { tab_id:data.id.clone(),block_id:block.id.clone(),action:(*action).into() });
        }
    }
}
fn chrome(out: &mut Presentation,layout: &Layout,state: &LookState) {
    let t=&state.theme; let m=&layout.metrics;
    let p=m.rail_padding as f32; let gap=m.control_gap as f32;
    let small=m.small_font_px as f32; let heading=m.ui_font_px as f32;
    let line=m.rail_header_height as f32; let pill_h=m.pill_height as f32;
    let w=layout.window_px[0] as f32;
    shape(out,layout.top_bar,0.0,t.rail,None,None);
    if let Some(r)=layout.left_rail { shape(out,r,0.0,t.rail,None,None); }
    if let Some(r)=layout.right_rail { shape(out,r,0.0,t.rail,None,None); }
    let bar_y=(layout.top_bar.h-pill_h)/2.0;
    let brand_w=m.brand_font_px as f32*6.0;
    text(out,rect(p,bar_y,brand_w,pill_h),"clanky",m.brand_font_px as f32,t.ink,600);
    let toggle=rect(p+brand_w+gap,bar_y,small*5.0+2.0*m.pill_padding as f32,pill_h);
    chip(out,t,m,toggle,"Rails",t.dim,Some(HitAction::ToggleRails));
    let status_w=small*15.0+2.0*m.pill_padding as f32;
    let status_x=w-p-status_w;
    let uptime_x=status_x-gap-status_w;
    chip(out,t,m,rect(uptime_x,bar_y,status_w,pill_h),&format!("Up {}h {:02}m",state.uptime_seconds/3600,state.uptime_seconds/60%60),t.teal,None);
    chip(out,t,m,rect(status_x,bar_y,status_w,pill_h),if state.masking_ready { "Masking ready" } else { "Masking unknown" },if state.masking_ready { t.ok_ink } else { t.dim },None);
    let search_x=(toggle.x+toggle.w+gap).max(layout.left_rail.map_or(0.0,|r|r.w)+m.outer_gap as f32);
    let search_w=if layout.rails_shown { m.search_width as f32 } else { small*6.0+2.0*m.pill_padding as f32 };
    let search=rect(search_x,(layout.top_bar.h-m.search_height as f32)/2.0,search_w.min(uptime_x-gap-search_x),m.search_height as f32);
    shape(out,search,m.item_radius as f32,t.panel,Some((m.border as f32,t.line)),None);
    text(out,rect(search.x+gap,search.y+m.chip_inset_y as f32,search.w-2.0*gap,search.h-m.chip_inset_y as f32),if layout.rails_shown { "Search commands…" } else { "Search" },small,t.dim,500);
    hit(out,search,HitAction::Palette);
    if let Some(r)=layout.left_rail {
        let bottom=r.y+r.h-p; let mut y=r.y+m.rail_section_gap as f32;
        for section in 0..3 {
            if y+line>bottom { break; }
            text(out,rect(r.x+p,y,r.w-2.0*p,line),["Sessions","Agents","Files"][section],heading,t.ink,600); y+=line+gap;
            match section {
                0=>for item in &state.sessions {
                    if y+m.rail_row_height as f32>bottom { break; }
                    row(out,t,m,rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32),&item.title,item.active,t.accent,HitAction::Session(item.id.clone())); y+=m.rail_row_height as f32;
                },
                1=>for item in &state.agents {
                    if y+m.rail_row_height as f32>bottom { break; }
                    row(out,t,m,rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32),&format!("{} · {}",item.peer_pid,if item.running { "running" } else { "idle" }),item.running,t.violet,HitAction::Agent { connection_id:item.connection_id,tab_id:item.tab_id.clone() }); y+=m.rail_row_height as f32;
                },
                _=>for item in &state.files {
                    if y+m.rail_row_height as f32>bottom { break; }
                    let file_rect=rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32);
                    row(out,t,m,file_rect,&item.title,item.active,t.accent,HitAction::File(item.id.clone()));
                    if item.pending>0 {
                        let count=format!("{} {}",item.pending,if item.pending==1 { "change" } else { "changes" });
                        let offset=m.chip_inset_y as f32+small*1.25+m.border as f32;
                        text(out,rect(file_rect.x+gap,y+offset,file_rect.w-2.0*gap,file_rect.h-offset),&count,small,t.dim,400);
                    }
                    y+=m.rail_row_height as f32;
                },
            }
            y+=m.rail_section_gap as f32;
        }
    } else {
        let mut x=search.x+search.w+gap;
        for item in &state.sessions {
            let width=small*8.0+2.0*m.pill_padding as f32;
            let r=rect(x,bar_y,width,pill_h);
            if r.x+r.w>uptime_x-gap { break; }
            chip(out,t,m,r,&item.title,if item.active { t.accent } else { t.dim },Some(HitAction::Session(item.id.clone()))); x+=width+gap;
        }
    }
    if let Some(r)=layout.right_rail {
        let x=r.x+p; let width=r.w-2.0*p; let bottom=r.y+r.h-p;
        let title_y=r.y+m.rail_section_gap as f32;
        // Agent commands per minute · last 30 min; wrap deliberately, never clip the label.
        let heading_line=m.panel_line_height as f32;
        text(out,rect(x,title_y,width,heading_line),"Agent commands per",small,t.ink,600);
        text(out,rect(x,title_y+heading_line,width,heading_line),"minute · last 30 min",small,t.ink,600);
        let chart=rect(x,title_y+2.0*heading_line+gap,width,m.sparkline_height as f32);
        shape(out,chart,m.item_radius as f32,t.panel,None,None);
        let samples: Vec<f32>=state.activity.iter().rev().take(30).copied().collect();
        let max=samples.iter().copied().filter(|v|v.is_finite()).fold(1.0_f32,f32::max);
        let step=(chart.w-2.0*gap)/samples.len().saturating_sub(1).max(1) as f32;
        let stroke=m.sparkline_bar_width as f32; let mut previous:Option<[f32;2]>=None;
        for (i,value) in samples.iter().rev().enumerate() {
            let h=if value.is_finite() { (value/max).clamp(0.0,1.0)*(chart.h-2.0*gap) } else { 0.0 };
            let point=[chart.x+gap+i as f32*step,chart.y+chart.h-gap-h];
            if let Some(from)=previous {
                let steps=((point[0]-from[0]).abs().max((point[1]-from[1]).abs())/(stroke/2.0).max(m.border as f32)).ceil().max(1.0) as usize;
                for j in 0..=steps {
                    let f=j as f32/steps as f32;
                    shape(out,rect(from[0]+(point[0]-from[0])*f-stroke/2.0,from[1]+(point[1]-from[1])*f-stroke/2.0,stroke,stroke),stroke/2.0,t.violet,None,None);
                }
            } else { shape(out,rect(point[0]-stroke/2.0,point[1]-stroke/2.0,stroke,stroke),stroke/2.0,t.violet,None,None); }
            previous=Some(point);
        }
        let access_height=state.panes.len() as f32*m.rail_row_height as f32+line+gap;
        let access_y=(bottom-access_height).max(chart.y+chart.h+line+2.0*gap);
        let timeline_y=chart.y+chart.h+m.rail_section_gap as f32;
        text(out,rect(x,timeline_y,width,line),"Timeline",heading,t.ink,600);
        let mut y=timeline_y+line+gap;
        let timeline_height=(m.timeline_row_height as f32).min(m.panel_line_height as f32);
        for item in state.timeline.iter().rev().take(20) {
            if y+timeline_height>access_y-gap { break; }
            let diameter=m.card_inset as f32;
            shape(out,rect(x,y+(timeline_height-diameter)/2.0,diameter,diameter),diameter/2.0,t.teal,None,None);
            text(out,rect(x+gap,y,width-gap,timeline_height),&format!("{} · {}",item.label,item.detail),small,t.dim,500); y+=timeline_height;
        }
        if access_y+line<bottom {
            text(out,rect(x,access_y,width,line),"Agent access",heading,t.ink,600); y=access_y+line+gap;
            for item in &state.panes {
                if y+m.access_switch_height as f32>bottom { break; }
                let sw=rect(x+width-m.access_switch_width as f32,y,m.access_switch_width as f32,m.access_switch_height as f32);
                text(out,rect(x,y,width-sw.w-gap,line),&item.title,small,t.dim,500);
                shape(out,sw,sw.h/2.0,if item.allow_agent { t.violet } else { t.line },None,None);
                let inset=m.access_knob_inset as f32; let diameter=(sw.h-2.0*inset).max(0.0);
                let knob_x=if item.allow_agent { sw.x+sw.w-inset-diameter } else { sw.x+inset };
                shape(out,rect(knob_x,sw.y+inset,diameter,diameter),diameter/2.0,t.ink,None,None);
                hit(out,sw,HitAction::Access(item.id.clone())); y+=m.rail_row_height as f32;
            }
        }
    }
}
fn panel_card(out: &mut Presentation,layout: &Layout,state: &LookState,panel: &PanelState) {
    let t=&state.theme; let m=&layout.metrics; let p=m.panel_padding as f32;
    let opacity=if state.animations { panel.opacity.clamp(0.0,1.0) } else { 1.0 };
    let rise=if state.animations { panel.rise_px } else { 0.0 };
    let r=intersect(rect(panel.rect.x,panel.rect.y+rise,panel.rect.w,panel.rect.h),rect(0.0,0.0,layout.window_px[0] as f32,layout.window_px[1] as f32));
    if opacity<=0.0 { return; }
    shape(out,r,m.panel_radius as f32,Theme::fade(t.panel,opacity),Some((m.border as f32,Theme::fade(t.line,opacity))),Some(Shadow { offset:[0.0,m.panel_shadow_offset_y as f32],blur:m.panel_shadow_blur as f32,spread:0.0,color:Theme::fade(t.shadow,opacity) }));
    text(out,rect(r.x+p,r.y+p,r.w-2.0*p,m.panel_header as f32),&panel.title,m.title_font_px as f32,Theme::fade(t.ink,opacity),600);
    let content=rect(r.x+p,r.y+p+m.panel_header as f32,r.w-2.0*p,r.h-2.0*p-m.panel_header as f32);
    if let Some(snapshot)=&panel.snapshot {
        let mut snapshot=snapshot.clone(); snapshot.cursor=None;
        out.scene.layers.push(Layer::Grid(GridLayer { origin:[content.x,content.y],clip:content.array(),snapshot,underlines:panel.underlines.clone(),opacity }));
    }
    for (index,line) in panel.lines.iter().enumerate() {
        let y=content.y+index as f32*m.panel_line_height as f32;
        if y+m.panel_line_height as f32>content.y+content.h { break; }
        let r=rect(content.x,y,content.w,m.panel_line_height as f32);
        if line.selected { shape(out,r,m.item_radius as f32,Theme::fade(t.selection,opacity),None,None); }
        text(out,rect(r.x+m.card_inset as f32,y,r.w-2.0*m.card_inset as f32,r.h),&line.text,m.small_font_px as f32,Theme::fade(if line.selected { t.ink } else { t.dim },opacity),500);
        if let Some(action)=&line.action { hit(out,r,action.clone()); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clanky_core::{layout::{Axis, Layout as Tree}, snapshot_from_chunks};
    use clanky_render::offscreen_scene;
    #[test]
    fn studio_scene_app() {
        // Literal independent design cases 4/5, not samples derived from drawing.
        let tree=Tree::branch(Axis::Rows,Tree::leaf("A"),Tree::leaf("B"));
        let layout=super::super::layout([1240,760],1.0,true,&tree,[10,20]);
        let a=PaneState::new("A",snapshot_from_chunks(14,71,&[]).unwrap());
        let mut b=PaneState::new("B",snapshot_from_chunks(14,71,&[b"cargo build"]).unwrap());
        b.blocks.push(BlockCard { id:"build".into(),first_row:0,last_row:0,command:"cargo build".into(),exit_code:Some(0),duration_ms:None,running:false });
        let mut state=LookState { focused:Some("B".into()),panes:vec![a,b],..LookState::default() };
        let pixels=offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
        let pixel=|pixels: &[u8],x:usize,y:usize| -> [u8;4] { pixels[(y*1240+x)*4..(y*1240+x)*4+4].try_into().unwrap() };
        for (x,y,expected) in [(590,44,[14,20,32,240]),(950,380,[19,27,43,255]),
            (221,51,[14,20,32,240]),(226,56,[19,27,43,255]),(100,700,[11,17,28,255]),
            (1100,700,[11,17,28,255]),(230,451,[26,34,49,255]),(936,451,[38,66,66,255])] {
            let got=pixel(&pixels,x,y);
            assert!(got.iter().zip(expected).all(|(a,b)| (*a as i16-b as i16).abs()<=3),"({x},{y}) {got:?} expected {expected:?}");
        }
        let glow=pixel(&pixels,215,577);
        assert!(glow[2]>=40 && glow[2]>glow[0],"focus glow {glow:?}");
        state.panes[0].allow_agent=true; state.panes[0].agent_running=true;
        let pixels=offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
        let glow=pixel(&pixels,215,222);
        assert!(glow[0] as i16-glow[1] as i16>=20 && glow[2] as i16-glow[1] as i16>=20,"agent glow {glow:?}");
    }
}
