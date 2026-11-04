//! App-chosen colour and spacing tokens. Terminal-provided ANSI colours are data.
pub type Color = [u8; 4];
pub const COLOR_TOKENS: [&str; 14] = ["window", "rail", "panel", "line", "ink", "dim", "accent", "violet", "teal", "ok", "err", "focus", "ok_ink", "err_ink"];

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub window: Color, pub rail: Color, pub panel: Color, pub line: Color,
    pub ink: Color, pub dim: Color, pub accent: Color, pub violet: Color,
    pub teal: Color, pub ok: Color, pub err: Color, pub focus: Color,
    pub ok_ink: Color, pub err_ink: Color,
    pub card: Color, pub chip_ok: Color, pub chip_err: Color, pub chip_neutral: Color,
    pub shadow: Color, pub focus_glow: Color, pub agent_glow: Color,
    pub selection: Color, pub review_old: Color, pub review_new: Color,
    pub link: Color, pub spelling: Color,
    pub card_mix: f32, pub chip_mix: f32,
    pub is_light: bool,
}
impl Default for Theme { fn default() -> Self { Self::dark() } }
impl Theme {
    pub fn dark() -> Self {
        let mut theme = Self {
            window: [15,21,34,255], rail: [11,17,28,255], panel: [19,27,43,255],
            line: [31,42,63,255], ink: [219,228,243,255], dim: [141,155,184,255],
            accent: [255,159,90,255], violet: [167,139,250,255], teal: [79,209,197,255],
            ok: [95,211,141,255], err: [255,107,122,255], focus: [143,180,255,255],
            ok_ink: [95,211,141,255], err_ink: [255,107,122,255],
            card: [0;4], chip_ok: [0;4], chip_err: [0;4], chip_neutral: [0;4],
            shadow: [0,0,0,120], focus_glow: [143,180,255,112], agent_glow: [167,139,250,255],
            selection: [40,70,120,255], review_old: [80,24,24,255], review_new: [24,80,40,255],
            link: [80,160,255,255], spelling: [255,96,96,255], card_mix: 0.03, chip_mix: 0.18, is_light: false,
        };
        theme.refresh_mixes();
        theme
    }
    pub fn light() -> Self {
        let mut theme = Self::dark();
        theme.is_light = true;
        theme.window = [233,238,246,255]; theme.rail = [243,246,251,255];
        theme.panel = [255,255,255,255]; theme.line = [214,222,234,255];
        theme.ink = [26,34,51,255]; theme.dim = [79,92,118,255];
        theme.accent = [181,79,11,255]; theme.violet = [109,79,216,255];
        theme.teal = [11,127,120,255]; theme.ok = [29,128,72,255];
        theme.err = [200,48,63,255]; theme.focus = [47,99,214,255];
        theme.ok_ink = [22,106,59,255]; theme.err_ink = [168,34,47,255];
        theme.card_mix = 0.04; theme.chip_mix = 0.10;
        theme.refresh_mixes();
        theme
    }
    pub fn override_color(&mut self, token: &str, color: Color) -> Result<(), String> {
        let target = match token {
            "window" => &mut self.window, "rail" => &mut self.rail, "panel" => &mut self.panel,
            "line" => &mut self.line, "ink" => &mut self.ink, "dim" => &mut self.dim,
            "accent" => &mut self.accent, "violet" => &mut self.violet, "teal" => &mut self.teal,
            "ok" => &mut self.ok, "err" => &mut self.err, "focus" => &mut self.focus,
            "ok_ink" => &mut self.ok_ink, "err_ink" => &mut self.err_ink,
            _ => return Err(format!("unknown color token: {token}")),
        };
        *target = color;
        self.refresh_mixes();
        Ok(())
    }
    /// Recompute derived opaque fills after changing palette tokens or mix fractions.
    pub fn refresh_mixes(&mut self) {
        self.card = mix(self.panel, if self.is_light { self.ink } else { [255;4] }, self.card_mix);
        self.focus_glow = Self::fade(self.focus, 112.0 / 255.0);
        self.agent_glow = self.violet;
        self.selection = mix(self.panel, self.focus, 0.20);
        self.review_old = mix(self.panel, self.err, 0.15);
        self.review_new = mix(self.panel, self.ok, 0.15);
        self.link = self.focus;
        self.spelling = self.err;
        self.chip_ok = mix(self.card, self.ok, self.chip_mix);
        self.chip_err = mix(self.card, self.err, self.chip_mix);
        self.chip_neutral = mix(self.card, self.dim, self.chip_mix);
    }
    /// Straight-alpha multiplication, also used for fade/rise transitions.
    pub fn fade(color: Color, opacity: f32) -> Color {
        let opacity = if opacity.is_finite() { opacity.clamp(0.0,1.0) } else { 1.0 };
        [color[0],color[1],color[2],(color[3] as f32 * opacity).round() as u8]
    }
    /// Translate the terminal's default foreground; explicit nondefault ANSI RGB stays data.
    pub fn terminal_color(&self, color: [u8;3], background: bool) -> [u8;3] {
        if !background && color == [229,229,229] { [self.ink[0],self.ink[1],self.ink[2]] }
        else { color }
    }
    /// Translate legacy editor/widget roles at the scene boundary, never terminal RGB.
    pub fn ui_color(&self, color: [u8;3], background: bool) -> [u8;3] {
        let mapped = match (color, background) {
            ([24,24,28] | [32,32,32] | [35,40,48] | [35,35,40], true) => self.panel,
            ([40,70,120] | [50,80,120] | [50,60,85], true) => self.selection,
            ([80,24,24], true) => self.review_old,
            ([24,80,40], true) => self.review_new,
            ([220,220,225] | [235,235,235] | [220,220,220] | [229,229,229] | [255,255,255], false) => self.ink,
            _ => return color,
        };
        [mapped[0],mapped[1],mapped[2]]
    }
}
fn mix(base: Color, ink: Color, weight: f32) -> Color {
    let weight = if weight.is_finite() { weight.clamp(0.0,1.0) } else { 0.0 };
    std::array::from_fn(|i| (base[i] as f32 * (1.0-weight) + ink[i] as f32 * weight).round() as u8)
}

/// Logical pixels and clock durations in one place. `scaled` changes spatial
/// fields only; animation times never depend on monitor scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub top_bar: f64, pub left_rail: f64, pub right_rail: f64,
    pub outer_gap: f64, pub pane_gap: f64, pub pane_padding: f64, pub pane_header: f64,
    pub pane_radius: f64, pub block_radius: f64, pub card_inset: f64,
    pub border: f64, pub control_gap: f64, pub rail_padding: f64,
    pub rail_row_height: f64, pub rail_section_gap: f64, pub rail_header_height: f64,
    pub item_radius: f64, pub ui_font_px: f64, pub small_font_px: f64,
    pub title_font_px: f64, pub brand_font_px: f64,
    pub search_width: f64, pub search_height: f64,
    pub pill_height: f64, pub pill_padding: f64,
    pub chip_inset_y: f64, pub chip_right_gap: f64, pub chip_padding_x: f64, pub chip_font_px: f64,
    pub panel_padding: f64, pub panel_header: f64, pub panel_radius: f64, pub panel_line_height: f64,
    pub panel_max_width: f64, pub panel_max_height: f64, pub panel_rise: f64,
    pub compose_dock_height: f64, pub compose_min_height: f64,
    pub focus_glow_blur: f64, pub focus_glow_spread: f64,
    pub agent_glow_blur: f64, pub agent_glow_spread: f64,
    pub panel_shadow_blur: f64, pub panel_shadow_offset_y: f64,
    pub cursor_width: f64, pub cursor_inset_y: f64, pub cursor_radius: f64,
    pub sparkline_height: f64, pub sparkline_bar_width: f64, pub sparkline_bar_gap: f64,
    pub timeline_row_height: f64, pub access_switch_width: f64, pub access_switch_height: f64,
    pub access_knob_inset: f64,
    pub cursor_ms: f32, pub panel_ms: f32, pub scroll_ms: f32, pub pulse_ms: f32,
}
impl Default for Metrics { fn default() -> Self { Self::DEFAULT } }
impl Metrics {
    pub const DEFAULT: Self = Self {
        top_bar:40.0,left_rail:210.0,right_rail:270.0,outer_gap:10.0,pane_gap:10.0,
        pane_padding:12.0,pane_header:24.0,pane_radius:11.0,block_radius:8.0,card_inset:4.0,
        border:1.0,control_gap:8.0,rail_padding:16.0,rail_row_height:34.0,rail_section_gap:18.0,
        rail_header_height:24.0,item_radius:7.0,ui_font_px:14.0,small_font_px:11.0,title_font_px:15.0,
        brand_font_px:16.0,search_width:320.0,search_height:26.0,pill_height:24.0,pill_padding:8.0,
        chip_inset_y:2.0,chip_right_gap:6.0,chip_padding_x:8.0,chip_font_px:12.0,
        panel_padding:12.0,panel_header:24.0,panel_radius:11.0,panel_line_height:20.0,
        panel_max_width:600.0,panel_max_height:480.0,panel_rise:8.0,
        compose_dock_height:220.0,compose_min_height:120.0,
        focus_glow_blur:16.0,focus_glow_spread:1.0,agent_glow_blur:20.0,agent_glow_spread:3.0,
        panel_shadow_blur:20.0,panel_shadow_offset_y:8.0,
        cursor_width:2.0,cursor_inset_y:2.0,cursor_radius:1.0,
        sparkline_height:40.0,sparkline_bar_width:3.0,sparkline_bar_gap:2.0,timeline_row_height:28.0,
        access_switch_width:32.0,access_switch_height:18.0,access_knob_inset:3.0,
        cursor_ms:80.0,panel_ms:120.0,scroll_ms:120.0,pulse_ms:1200.0,
    };
    pub fn is_valid(&self) -> bool {
        [self.top_bar,self.left_rail,self.right_rail,self.outer_gap,self.pane_gap,self.pane_padding,
            self.pane_header,self.pane_radius,self.block_radius,self.card_inset,self.border,
            self.control_gap,self.rail_padding,self.rail_row_height,self.rail_section_gap,
            self.rail_header_height,self.item_radius,self.ui_font_px,self.small_font_px,self.title_font_px,
            self.brand_font_px,self.search_width,self.search_height,self.pill_height,self.pill_padding,
            self.chip_inset_y,self.chip_right_gap,self.chip_padding_x,self.chip_font_px,self.panel_padding,
            self.panel_header,self.panel_radius,self.panel_line_height,self.panel_max_width,self.panel_max_height,
            self.panel_rise,self.compose_dock_height,self.compose_min_height,self.focus_glow_blur,
            self.focus_glow_spread,self.agent_glow_blur,self.agent_glow_spread,self.panel_shadow_blur,
            self.panel_shadow_offset_y,self.cursor_width,self.cursor_inset_y,self.cursor_radius,
            self.sparkline_height,self.sparkline_bar_width,self.sparkline_bar_gap,self.timeline_row_height,
            self.access_switch_width,self.access_switch_height,self.access_knob_inset]
            .into_iter().all(|v| v.is_finite() && v >= 0.0 && v <= f32::MAX as f64)
            && [self.cursor_ms,self.panel_ms,self.scroll_ms,self.pulse_ms].into_iter().all(|v| v.is_finite() && v >= 0.0)
    }
    pub fn scaled(self, scale: f64) -> Self {
        let mut out = self;
        macro_rules! scale_fields { ($($field:ident),* $(,)?) => { $(out.$field *= scale;)* }; }
        scale_fields!(top_bar,left_rail,right_rail,outer_gap,pane_gap,pane_padding,pane_header,
            pane_radius,block_radius,card_inset,border,control_gap,rail_padding,rail_row_height,
            rail_section_gap,rail_header_height,item_radius,ui_font_px,small_font_px,title_font_px,
            brand_font_px,search_width,search_height,pill_height,pill_padding,chip_inset_y,chip_right_gap,
            chip_padding_x,chip_font_px,panel_padding,panel_header,panel_radius,panel_line_height,
            panel_max_width,panel_max_height,panel_rise,compose_dock_height,compose_min_height,
            focus_glow_blur,focus_glow_spread,agent_glow_blur,agent_glow_spread,panel_shadow_blur,
            panel_shadow_offset_y,cursor_width,cursor_inset_y,cursor_radius,sparkline_height,
            sparkline_bar_width,sparkline_bar_gap,timeline_row_height,access_switch_width,
            access_switch_height,access_knob_inset);
        out
    }
}
