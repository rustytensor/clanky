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
    pub post_ai: bool,
    /// The command palette is open: its top-bar field shows the focus ring.
    pub palette_open: bool,
    /// Open find bar: pane id, query, match count.
    pub find: Option<(String,String,usize)>,
    /// The person at the window, for the sidebar's user block.
    pub user_name: String,
    /// Exit codes known for Recent entries, parallel to timeline: Some(code) when finished.
    pub timeline_exits: Vec<Option<i32>>,
    pub pane_labels: std::collections::BTreeMap<String,String>,
    pub sessions: Vec<SessionItem>,
    pub session_counts: std::collections::BTreeMap<String, usize>,
    /// Optional age labels (or labelled Eastern times), parallel to timeline.
    pub timeline_times: Vec<String>,
    pub agents: Vec<AgentItem>,
    pub agent_labels: std::collections::BTreeMap<u64,String>,
    /// Action opacity by (terminal, block), absent for resting cards.
    pub block_actions: std::collections::BTreeMap<(String,String),f32>,
    pub block_action_focus: Option<(String,String,usize)>,
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
            panes: vec![], post_ai:false, palette_open:false, find: None, user_name: String::new(), timeline_exits: vec![], pane_labels: Default::default(), sessions: vec![], session_counts: Default::default(), timeline_times: vec![], agents: vec![], agent_labels: Default::default(), block_actions: Default::default(), block_action_focus: None, files: vec![],
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
#[derive(Clone, Debug, PartialEq)]
pub struct SessionItem { pub id: String, pub title: String, pub active: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct AgentItem {
    pub connection_id: u64, pub peer_pid: u32, pub tab_id: Option<String>, pub running: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FileItem { pub id: String, pub title: String, pub pending: usize, pub active: bool }
#[derive(Clone, Debug, PartialEq)]
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
    Palette, Settings, ToggleRails, NewTab, Split, Session(String), File(String),
    Agent { connection_id: u64, tab_id: Option<String> }, Access(String),
    Block { tab_id: String, block_id: String, action: String }, PaletteChoice(usize),
    /// Find bar: -1 previous match, 1 next match, 0 close.
    Find(i8),
    /// A change-review button in an editor pane.
    Review { pane: String, action: String },
}
#[derive(Clone, Debug)]
pub struct Hotspot { pub rect: PixelRect, pub action: HitAction }
#[derive(Clone, Debug)]
pub struct Presentation { pub scene: Scene, pub hits: Vec<Hotspot> }
#[derive(Clone,PartialEq)]
struct ChromeKey {
    theme:Theme,metrics:super::Metrics,window:[u32;2],top:PixelRect,rails:[Option<PixelRect>;2],shown:bool,
    sessions:Vec<SessionItem>,counts:std::collections::BTreeMap<String,usize>,agents:Vec<AgentItem>,
    labels:std::collections::BTreeMap<u64,String>,files:Vec<FileItem>,timeline:Vec<TimelineItem>,times:Vec<String>,
    activity:Vec<f32>,panes:Vec<(String,String,bool)>,uptime:Option<u64>,masking:bool,
}
impl ChromeKey {
    fn new(layout:&Layout,state:&LookState)->Self{Self{
        theme:state.theme.clone(),metrics:layout.metrics,window:layout.window_px,top:layout.top_bar,
        rails:[layout.left_rail,layout.right_rail],shown:layout.rails_shown,
        sessions:state.sessions.clone(),counts:state.session_counts.clone(),agents:state.agents.clone(),labels:state.agent_labels.clone(),
        files:state.files.clone(),timeline:state.timeline.clone(),times:state.timeline_times.clone(),activity:state.activity.clone(),
        panes:state.panes.iter().map(|pane|(pane.id.clone(),pane.title.clone(),pane.allow_agent)).collect(),
        uptime:(state.masking_ready || state.uptime_seconds>0).then_some(state.uptime_seconds/3600),masking:state.masking_ready,
    }}
}
/// Reuses static chrome until its inputs change. Clear after changing font families.
#[derive(Default)]
pub struct SceneCache {chrome:Option<(ChromeKey,Vec<Layer>,Vec<Hotspot>)>}
impl SceneCache {
    pub fn clear(&mut self){self.chrome=None;}
    pub fn presentation(&mut self,layout:&Layout,state:&LookState,measure:&mut dyn FnMut(&TextRun)->clanky_render::TextMetrics)->Presentation{
        build_presentation(layout,state,measure,Some(self))
    }
}
struct Builder<'a> {
    scene: Scene,
    hits: Vec<Hotspot>,
    measure: &'a mut dyn FnMut(&TextRun) -> clanky_render::TextMetrics,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PixelRect {
    PixelRect { x, y, w: w.max(0.0), h: h.max(0.0) }
}
fn shape(out: &mut Builder<'_>, r: PixelRect, radius: f32, fill: [u8; 4], border: Option<(f32, [u8; 4])>, shadow: Option<Shadow>) {
    if r.w > 0.0 && r.h > 0.0 {
        out.scene.layers.push(Layer::Shape(Shape { rect: r.array(), radius,
            fill: Fill::Solid(fill), border, shadow }));
    }
}
fn run(value: &str, px: f32, weight: u16, mono: bool, letter_spacing: f32) -> TextRun {
    TextRun { text:value.split(['\n','\r']).next().unwrap_or("").chars().take(4096).collect(),
        px,weight,mono,letter_spacing,..TextRun::default() }
}
fn text_width(out: &mut Builder<'_>, value: &str, px: f32, weight: u16, mono: bool, spacing: f32) -> f32 {
    (out.measure)(&run(value,px,weight,mono,spacing)).width
}
fn styled_text(out: &mut Builder<'_>, r: PixelRect, value: &str, px: f32, color: [u8;4], weight: u16, mono: bool, spacing: f32) {
    if r.w<=0.0 || r.h<=0.0 {return;}
    let mut run=run(value,px,weight,mono,spacing);
    let mut metrics=(out.measure)(&run);
    if metrics.width>r.w {
        let chars:Vec<char>=run.text.chars().collect();
        let (mut low,mut high)=(0,chars.len());
        while low<high {
            let mid=(low+high).div_ceil(2);
            run.text=chars[..mid].iter().collect::<String>()+"…";
            if (out.measure)(&run).width<=r.w {low=mid;}else{high=mid-1;}
        }
        run.text=chars[..low].iter().collect::<String>()+"…";
        metrics=(out.measure)(&run);
    }
    if metrics.width>r.w || metrics.ascent+metrics.descent>r.h || run.text.is_empty(){return;}
    let baseline=r.y+(r.h-metrics.ascent-metrics.descent)/2.0+metrics.ascent;
    // cosmic-text's baseline includes half the line-box leading.
    let baseline_in_box=(metrics.line_height-metrics.ascent-metrics.descent)/2.0+metrics.ascent;
    run.origin=[r.x,baseline-baseline_in_box];run.color=color;
    out.scene.layers.push(Layer::Text(run));
}
fn text(out: &mut Builder<'_>, r: PixelRect, value: &str, px: f32, color: [u8;4], weight: u16) {
    styled_text(out,r,value,px,color,weight,false,0.0);
}
fn heading_text(out: &mut Builder<'_>, r: PixelRect, value: &str, px: f32, color: [u8;4]) {
    styled_text(out,r,value,px,color,600,false,px*0.1);
}
fn hit(out: &mut Builder<'_>, r: PixelRect, action: HitAction) {
    if r.w > 0.0 && r.h > 0.0 { out.hits.push(Hotspot { rect: r, action }); }
}
fn chip(out: &mut Builder<'_>, t: &Theme, m: &super::Metrics, r: PixelRect, label: &str, color: [u8; 4], action: Option<HitAction>) {
    let pad = m.pill_padding as f32;
    shape(out, r, r.h / 2.0, t.panel, Some((m.border as f32, Theme::fade(color,0.35))), None);
    text(out, rect(r.x+pad, r.y, r.w-2.0*pad, r.h), label, m.small_font_px as f32, color, 600);
    if let Some(action) = action { hit(out, r, action); }
}
fn row(out: &mut Builder<'_>, t: &Theme, m: &super::Metrics, r: PixelRect, label: &str, count: &str, active: bool, color: [u8;4], action: HitAction) {
    let inset = m.control_gap as f32;
    if active {
        shape(out, r, m.item_radius as f32, t.panel, None, None);
        shape(out, rect(r.x,r.y+inset,m.border as f32*2.0,r.h-2.0*inset),m.border as f32,t.accent,None,None);
    }
    let dot=matches!(action,HitAction::Session(_) | HitAction::Agent{..});
    let left=if dot {3.0*inset} else {inset};
    if dot {shape(out,rect(r.x+inset,r.y+(r.h-inset)/2.0,inset,inset),inset/2.0,color,None,None);}
    let count_width=text_width(out,count,m.small_font_px as f32,400,true,0.0);
    text(out,rect(r.x+left,r.y,(r.w-left-2.0*inset-count_width).max(0.0),r.h),label,m.small_font_px as f32,if active { t.ink } else { t.dim },500);
    if !count.is_empty() {
        styled_text(out,rect(r.x+r.w-inset-count_width,r.y,count_width,r.h),count,m.small_font_px as f32,t.dim,400,true,0.0);
    }
    hit(out,r,action);
}
pub fn scene(layout: &Layout, state: &LookState) -> Scene { presentation(layout,state).scene }

pub fn panel_presentation(layout:&Layout,state:&LookState,measure:&mut dyn FnMut(&TextRun)->clanky_render::TextMetrics)->Presentation {
    let mut out=Builder{scene:Scene{clear:[0;4],layers:vec![]},hits:vec![],measure};
    for panel in &state.panels{panel_card(&mut out,layout,state,panel);}
    Presentation{scene:out.scene,hits:out.hits}
}
pub fn presentation(layout: &Layout, state: &LookState) -> Presentation {
    presentation_with_measure(layout,state,&mut clanky_render::measure_text)
}
pub fn presentation_with_measure(layout: &Layout, state: &LookState,
    measure: &mut dyn FnMut(&TextRun) -> clanky_render::TextMetrics) -> Presentation {
    build_presentation(layout,state,measure,None)
}
fn build_presentation(layout:&Layout,state:&LookState,measure:&mut dyn FnMut(&TextRun)->clanky_render::TextMetrics,cache:Option<&mut SceneCache>)->Presentation {
    let t = &state.theme;
    let m = &layout.metrics;
    let p = m.pane_padding as f32;
    let mut out = Builder { scene: Scene { clear: Theme::fade(t.window,state.opacity),layers:vec![] },hits:vec![],measure };
    if let Some(cache)=cache {
        let key=ChromeKey::new(layout,state);
        if let Some((old,layers,hits))=&cache.chrome {
            if *old==key {out.scene.layers=layers.clone();out.hits=hits.clone();}
            else {chrome(&mut out,layout,state);cache.chrome=Some((key,out.scene.layers.clone(),out.hits.clone()));}
        } else {chrome(&mut out,layout,state);cache.chrome=Some((key,out.scene.layers.clone(),out.hits.clone()));}
    } else {chrome(&mut out,layout,state);}
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
        let access_label=if data.allow_agent { "read+run" } else { "agents off" };
        let access_width=text_width(&mut out,access_label,m.small_font_px as f32,600,false,0.0)+2.0*m.pill_padding as f32;
        let access=rect(pane.rect.x+pane.rect.w-p-access_width,pane.rect.y+p,access_width,m.pane_header as f32);
        chip(&mut out,t,m,access,access_label,if data.allow_agent { t.violet } else { t.dim },Some(HitAction::Access(data.id.clone())));
        let title_width=(access.x-pane.rect.x-2.0*p-m.control_gap as f32).max(0.0);
        let driver_width=text_width(&mut out,&data.driver,m.small_font_px as f32,500,false,0.0).min(title_width*0.4);
        let title_width=text_width(&mut out,&data.title,m.title_font_px as f32,600,false,0.0)
            .min((title_width-driver_width-m.control_gap as f32).max(0.0));
        text(&mut out,rect(pane.rect.x+p,pane.rect.y+p,title_width,m.pane_header as f32),&data.title,m.title_font_px as f32,t.ink,600);
        text(&mut out,rect(pane.rect.x+p+title_width+m.control_gap as f32,pane.rect.y+p,driver_width,m.pane_header as f32),&data.driver,m.small_font_px as f32,if agent { t.violet } else { t.dim },500);
        let scroll=if data.scroll_px.is_finite() { data.scroll_px } else { 0.0 };
        let origin=[pane.grid_origin[0],pane.grid_origin[1]-scroll];
        for block in &data.blocks { block_card(&mut out,layout,state,pane,data,block,origin); }
        let mut snapshot=data.snapshot.clone(); snapshot.cursor=None;
        for cell in &mut snapshot.cells { t.terminal_cell(cell); }
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
    Presentation {scene:out.scene,hits:out.hits}
}
fn intersect(a: PixelRect,b: PixelRect) -> PixelRect {
    let x=a.x.max(b.x); let y=a.y.max(b.y);
    rect(x,y,(a.x+a.w).min(b.x+b.w)-x,(a.y+a.h).min(b.y+b.h)-y)
}
pub fn block_card_rect(layout: &Layout,pane: &super::PaneLayout,block: &BlockCard,rows: usize,origin: [f32;2]) -> Option<PixelRect> {
    if block.last_row<block.first_row || block.first_row>=rows {return None;}
    let last=block.last_row.min(rows.saturating_sub(1));
    let ch=layout.cell_px[1] as f32;let inset=layout.metrics.card_inset as f32;
    let y=origin[1]+block.first_row as f32*ch;
    let card=intersect(rect(origin[0]-inset,y-inset,pane.grid_clip.w+2.0*inset,(last-block.first_row+1) as f32*ch+2.0*inset),
        rect(pane.grid_clip.x-inset,pane.grid_clip.y-inset,pane.grid_clip.w+2.0*inset,pane.grid_clip.h+2.0*inset));
    (card.h>0.0).then_some(card)
}
fn block_card(out: &mut Builder<'_>,layout: &Layout,state: &LookState,pane: &super::PaneLayout,data: &PaneState,block: &BlockCard,origin:[f32;2]) {
    let Some(card)=block_card_rect(layout,pane,block,data.snapshot.rows,origin) else{return;};
    let t=&state.theme; let m=&layout.metrics;
    let ch=layout.cell_px[1] as f32;
    let y=origin[1]+block.first_row as f32*ch;
    shape(out,card,m.block_radius as f32,t.card,block.running.then_some((m.border as f32,t.violet)),None);
    if y<pane.grid_clip.y || y+ch>pane.grid_clip.y+pane.grid_clip.h { return; }
    let prompt_px=ch/1.25;
    let prompt_width=text_width(out,"❯",prompt_px,400,false,0.0);
    let prompt_left=pane.rect.x+m.border as f32;
    let prompt_space=origin[0]-prompt_left;
    if prompt_width<=prompt_space {
        text(out,rect(prompt_left,y,prompt_space,ch),"❯",prompt_px,t.accent,400);
    }
    let mut right=card.x+card.w-m.chip_right_gap as f32;
    let dy=m.chip_inset_y as f32;
    if let Some(code)=block.exit_code {
        let label=if code==0 { "✓ 0".into() } else { format!("✗ {code}") };
        let color=if code==0 { t.ok_ink } else { t.err_ink };
        let font=m.chip_font_px as f32;
        let width=text_width(out,&label,font,600,false,0.0)+2.0*m.chip_padding_x as f32;
        let pill=rect(right-width,y+dy,width,ch-2.0*dy);
        if pill.x>=card.x {
            shape(out,pill,pill.h/2.0,if code==0 { t.chip_ok } else { t.chip_err },None,None);
            text(out,rect(pill.x+m.chip_padding_x as f32,pill.y,pill.w-2.0*m.chip_padding_x as f32,pill.h),&label,font,color,600);
            right=pill.x-m.control_gap as f32;
        }
    }
    if let Some(ms)=block.duration_ms {
        let duration=format!("{:.1}s",ms as f64/1000.0);
        let width=text_width(out,&duration,m.small_font_px as f32,500,false,0.0);
        text(out,rect((right-width).max(card.x),y+dy,width,ch-2.0*dy),&duration,m.small_font_px as f32,t.dim,500);
        right-=width+m.control_gap as f32;
    }
    if block.running {
        let pulse=if state.animations { 0.65+0.35*(state.elapsed_ms/m.pulse_ms).sin().abs() } else { 1.0 };
        let diameter=m.chip_right_gap as f32;
        shape(out,rect(right-diameter,y+(ch-diameter)/2.0,diameter,diameter),diameter/2.0,Theme::fade(t.violet,pulse),None,None);
    }
    // Actual command/output are painted only by the original, unmodified grid.
    let opacity=state.block_actions.get(&(data.id.clone(),block.id.clone())).copied().unwrap_or(0.0).clamp(0.0,1.0);
    if opacity<=0.0 {return;}
    let actions=[("copy","Copy"),("rerun","Run"),("send","Agent"),("collapse","Fold")];
    let width=m.pill_height as f32*2.0;
    let controls_width=actions.len() as f32*width;
    if right-controls_width>card.x+pane.grid_clip.w/2.0 {
        for (i,(action,label)) in actions.iter().enumerate() {
            let r=rect(right-controls_width+i as f32*width,y+dy,width-m.control_gap as f32,ch-2.0*dy);
            if state.block_action_focus.as_ref().is_some_and(|(tab,id,index)|tab==&data.id && id==&block.id && *index==i) {
                shape(out,r,m.item_radius as f32,Theme::fade(t.selection,opacity),None,None);
            }
            text(out,r,label,m.small_font_px as f32,Theme::fade(t.dim,opacity),500);
            hit(out,r,HitAction::Block { tab_id:data.id.clone(),block_id:block.id.clone(),action:(*action).into() });
        }
    }
}
fn chrome(out: &mut Builder<'_>,layout: &Layout,state: &LookState) {
    let t=&state.theme; let m=&layout.metrics;
    let p=m.rail_padding as f32; let gap=m.control_gap as f32;
    let small=m.small_font_px as f32; let heading=10.5*(m.ui_font_px/14.0) as f32;
    let line=m.rail_header_height as f32; let pill_h=m.pill_height as f32;
    let w=layout.window_px[0] as f32;
    shape(out,layout.top_bar,0.0,t.rail,None,None);
    shape(out,rect(0.0,layout.top_bar.h-m.border as f32,w,m.border as f32),0.0,t.line,None,None);
    if let Some(r)=layout.left_rail { shape(out,r,0.0,t.rail,None,None); shape(out,rect(r.x+r.w-m.border as f32,r.y,m.border as f32,r.h),0.0,t.line,None,None); }
    if let Some(r)=layout.right_rail { shape(out,r,0.0,t.rail,None,None); shape(out,rect(r.x,r.y,m.border as f32,r.h),0.0,t.line,None,None); }
    let bar_y=(layout.top_bar.h-pill_h)/2.0;
    let brand_w=text_width(out,"Clanky",m.brand_font_px as f32,600,false,0.0);
    let mark=m.ui_font_px as f32;
    // Three-stop vertical gradient, clipped analytically to the 4 px rounded mark.
    let radius=m.card_inset as f32;
    let strips=(mark*2.0).ceil().max(1.0) as usize;
    out.scene.layers.push(Layer::Shape(Shape{rect:[p,(layout.top_bar.h-mark)/2.0,mark,mark],radius,
        fill:Fill::Vertical(t.accent,t.teal),border:None,shadow:None}));
    for i in 0..strips {
        let y=mark*(i as f32+0.5)/strips as f32;
        let edge=(radius-y).max(y-(mark-radius)).max(0.0);
        let inset=radius-(radius*radius-edge*edge).max(0.0).sqrt();
        let f=y/mark; let (a,b,q)=if f<0.5 {(t.accent,t.violet,f*2.0)}else{(t.violet,t.teal,(f-0.5)*2.0)};
        let color=std::array::from_fn(|c|(a[c] as f32*(1.0-q)+b[c] as f32*q).round() as u8);
        shape(out,rect(p+inset,(layout.top_bar.h-mark)/2.0+i as f32*mark/strips as f32,mark-2.0*inset,mark/strips as f32),0.0,color,None,None);
    }
    text(out,rect(p+mark+gap,bar_y,brand_w,pill_h),"Clanky",m.brand_font_px as f32,t.ink,600);
    let mut uptime_x=w-p;
    if state.masking_ready {
        let label="masking on";let width=text_width(out,label,small,600,false,0.0)+2.0*m.pill_padding as f32;
        uptime_x-=width;
        chip(out,t,m,rect(uptime_x,bar_y,width,pill_h),label,t.dim,None);uptime_x-=gap;
    }
    if state.masking_ready || state.uptime_seconds>0 {
        let label=format!("● server up · {} h",state.uptime_seconds/3600);
        let width=text_width(out,&label,small,600,false,0.0)+2.0*m.pill_padding as f32;
        uptime_x-=width;
        chip(out,t,m,rect(uptime_x,bar_y,width,pill_h),&label,t.ok,None);uptime_x-=gap;
    }
    let settings_width=text_width(out,"Settings",small,600,false,0.0)+2.0*m.pill_padding as f32;
    uptime_x-=settings_width+gap;
    chip(out,t,m,rect(uptime_x,bar_y,settings_width,pill_h),"Settings",t.dim,Some(HitAction::Settings));
    let search_x=p+mark+gap+brand_w+3.0*gap;
    let search_w=if layout.rails_shown { m.search_width as f32 } else { small*6.0+2.0*m.pill_padding as f32 };
    let search=rect(search_x,(layout.top_bar.h-m.search_height as f32)/2.0,search_w.min(uptime_x-gap-search_x),m.search_height as f32);
    shape(out,search,m.control_gap as f32,t.panel,Some((m.border as f32,t.line)),None);
    let shortcut="Ctrl+Shift+P";let shortcut_width=text_width(out,shortcut,small,400,false,0.0);
    text(out,rect(search.x+gap,search.y,(search.w-3.0*gap-shortcut_width).max(0.0),search.h),if layout.rails_shown { "Search commands, blocks, files…" } else { "Search" },small,t.dim,500);
    if layout.rails_shown && search.w>shortcut_width+4.0*gap { text(out,rect(search.x+search.w-gap-shortcut_width,search.y,shortcut_width,search.h),shortcut,small,t.dim,400); }
    hit(out,search,HitAction::Palette);
    if let Some(r)=layout.left_rail {
        let bottom=r.y+r.h-p; let mut y=r.y+m.rail_section_gap as f32;
        for section in 0..3 {
            if y+line>bottom { break; }
            heading_text(out,rect(r.x+p,y,r.w-2.0*p,line),["SESSIONS","AGENTS","FILES"][section],heading,t.dim); y+=line+gap;
            match section {
                0=>for item in &state.sessions {
                    if y+m.rail_row_height as f32>bottom { break; }
                    row(out,t,m,rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32),&item.title,&state.session_counts.get(&item.id).map(usize::to_string).unwrap_or_default(),item.active,t.accent,HitAction::Session(item.id.clone())); y+=m.rail_row_height as f32;
                },
                1=>for item in &state.agents {
                    if y+m.rail_row_height as f32>bottom { break; }
                    row(out,t,m,rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32),&state.agent_labels.get(&item.connection_id).cloned().unwrap_or_else(||format!("agent {}",item.peer_pid)),if item.running {"run"} else {"idle"},item.running,if item.running {t.violet} else {t.dim},HitAction::Agent { connection_id:item.connection_id,tab_id:item.tab_id.clone() }); y+=m.rail_row_height as f32;
                },
                _=>for item in &state.files {
                    if y+m.rail_row_height as f32>bottom { break; }
                    let file_rect=rect(r.x+gap,y,r.w-2.0*gap,m.rail_row_height as f32);
                    let count=if item.pending>0 {format!("{} {}",item.pending,if item.pending==1 {"change"} else {"changes"})} else {String::new()};
                    row(out,t,m,file_rect,&item.title,&count,item.active,t.accent,HitAction::File(item.id.clone()));
                    y+=m.rail_row_height as f32;
                },
            }
            if (section==1 && state.agents.is_empty()) || (section==2 && state.files.is_empty()) {
                text(out,rect(r.x+p,y,r.w-2.0*p,line),if section==1 {"No agents connected"} else {"No open files"},small,t.dim,400);y+=line;
            }
            y+=m.rail_section_gap as f32;
        }
    } else {
        let mut x=search.x+search.w+gap;
        for item in &state.sessions {
            let width=text_width(out,&item.title,small,600,false,0.0)+2.0*m.pill_padding as f32;
            let r=rect(x,bar_y,width,pill_h);
            if r.x+r.w>uptime_x-gap { break; }
            chip(out,t,m,r,&item.title,if item.active { t.accent } else { t.dim },Some(HitAction::Session(item.id.clone()))); x+=width+gap;
        }
    }
    if let Some(r)=layout.right_rail {
        let x=r.x+p; let width=r.w-2.0*p; let bottom=r.y+r.h-p;
        let title_y=r.y+m.rail_section_gap as f32;
        let heading_line=m.panel_line_height as f32;
        heading_text(out,rect(x,title_y,width,heading_line),"AGENT COMMANDS / MIN · 30 MIN",heading,t.dim);
        let chart=rect(x,title_y+heading_line+gap,width,m.sparkline_height as f32);
        shape(out,chart,m.item_radius as f32,t.panel,None,None);
        let samples: Vec<f32>=state.activity.iter().rev().take(30).copied().collect();
        let max=samples.iter().copied().filter(|v|v.is_finite()).fold(1.0_f32,f32::max);
        let has_activity=samples.iter().any(|v|v.is_finite() && *v>0.0);
        if !has_activity {
            shape(out,rect(chart.x+gap,chart.y+chart.h-gap,chart.w-2.0*gap,m.border as f32),0.0,t.dim,None,None);
            text(out,rect(chart.x+gap,chart.y,chart.w-2.0*gap,chart.h-gap),"No agent activity",small,t.dim,400);
        }
        let step=(chart.w-2.0*gap)/samples.len().saturating_sub(1).max(1) as f32;
        let stroke=m.sparkline_bar_width as f32; let mut previous:Option<[f32;2]>=None;
        for (i,value) in samples.iter().rev().enumerate().filter(|_|has_activity) {
            let h=if value.is_finite() { (value/max).clamp(0.0,1.0)*(chart.h-2.0*gap) } else { 0.0 };
            let point=[chart.x+gap+i as f32*step,chart.y+chart.h-gap-h];
            if let Some(from)=previous {
                let columns=(point[0]-from[0]).ceil().max(1.0) as usize;
                let dx=(point[0]-from[0])/columns as f32;
                for column in 0..columns {
                    let q=(column as f32+0.5)/columns as f32;let py=from[1]+(point[1]-from[1])*q;
                    shape(out,rect(from[0]+column as f32*dx,py,dx,chart.y+chart.h-gap-py),0.0,Theme::fade(t.violet,0.18),None,None);
                }
                let steps=((point[0]-from[0]).abs().max((point[1]-from[1]).abs())/(stroke/2.0).max(m.border as f32)).ceil().max(1.0) as usize;
                for j in 0..=steps {
                    let f=j as f32/steps as f32;
                    let px=from[0]+(point[0]-from[0])*f;let py=from[1]+(point[1]-from[1])*f;
                    shape(out,rect(px-stroke/2.0,py-stroke/2.0,stroke,stroke),stroke/2.0,t.violet,None,None);
                }
            } else { shape(out,rect(point[0]-stroke/2.0,point[1]-stroke/2.0,stroke,stroke),stroke/2.0,t.violet,None,None); }
            previous=Some(point);
        }
        let access_height=state.panes.len() as f32*m.rail_row_height as f32+line+gap;
        let timeline_y=chart.y+chart.h+m.rail_section_gap as f32;
        let timeline_height=2.0*m.panel_line_height as f32;
        let access_y=(timeline_y+line+gap+state.timeline.len().min(20) as f32*timeline_height+m.rail_section_gap as f32)
            .min(bottom-access_height).max(chart.y+chart.h+line+2.0*gap);
        heading_text(out,rect(x,timeline_y,width,line),"TIMELINE",heading,t.dim);
        let mut y=timeline_y+line+gap;
        for (index,item) in state.timeline.iter().enumerate().rev().take(20) {
            if y+timeline_height>access_y-gap { break; }
            let diameter=gap;
            shape(out,rect(x+diameter/2.0-m.border as f32,y,m.border as f32*2.0,timeline_height),0.0,t.line,None,None);
            shape(out,rect(x,y+gap,diameter,diameter),diameter/2.0,t.teal,None,None);
            let time=state.timeline_times.get(index).map(String::as_str).unwrap_or("");
            let time_width=text_width(out,time,small,400,true,0.0);
            text(out,rect(x+2.0*gap,y,width-3.0*gap-time_width,heading_line),&item.label,small,t.ink,500);
            text(out,rect(x+2.0*gap,y+heading_line,width-2.0*gap,heading_line),&item.detail,small,t.dim,400);
            styled_text(out,rect(x+width-time_width,y,time_width,heading_line),time,small,t.dim,400,true,0.0);
            y+=timeline_height;
        }
        if access_y+line<bottom {
            heading_text(out,rect(x,access_y,width,line),"ACCESS",heading,t.dim); y=access_y+line+gap;
            for item in &state.panes {
                if y+m.access_switch_height as f32>bottom { break; }
                let card=rect(x,y,width,m.rail_row_height as f32-gap/2.0);
                shape(out,card,m.block_radius as f32,t.panel,Some((m.border as f32,t.line)),None);
                let sw=rect(x+width-gap-m.access_switch_width as f32,y+(card.h-m.access_switch_height as f32)/2.0,m.access_switch_width as f32,m.access_switch_height as f32);
                text(out,rect(x+gap,y,width-sw.w-3.0*gap,card.h),&item.title,small,t.dim,500);
                shape(out,sw,sw.h/2.0,if item.allow_agent { t.violet } else { t.line },None,None);
                let inset=m.access_knob_inset as f32; let diameter=(sw.h-2.0*inset).max(0.0);
                let knob_x=if item.allow_agent { sw.x+sw.w-inset-diameter } else { sw.x+inset };
                shape(out,rect(knob_x,sw.y+inset,diameter,diameter),diameter/2.0,t.ink,None,None);
                hit(out,sw,HitAction::Access(item.id.clone())); y+=m.rail_row_height as f32;
            }
        }
    }
}
fn panel_card(out: &mut Builder<'_>,layout: &Layout,state: &LookState,panel: &PanelState) {
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
        let prompt=clanky_render::measure_text(&run("❯",16.0,400,false,0.0));
        println!("prompt mark at 16 px: advance {}, ascent {}, descent {}; card inset 4, pane padding 12",prompt.width,prompt.ascent,prompt.descent);
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
        println!("dark focus (215,577): {glow:?}");
        assert!(glow[2]>=40 && glow[2]>glow[0],"focus glow {glow:?}");
        state.panes[0].allow_agent=true; state.panes[0].agent_running=true;
        let pixels=offscreen_scene(&scene(&layout,&state),1240,760).unwrap();
        let glow=pixel(&pixels,215,222);
        println!("dark agent (215,222): {glow:?}");
        assert!(glow[0] as i16-glow[1] as i16>=20 && glow[2] as i16-glow[1] as i16>=20,"agent glow {glow:?}");
    }
}
