//! The control record: with `CLANKY_CONTROL_RECORD=<path>` the window appends, one JSON object a line, every
//! control's painted box, hit box and state, the landmarks the design gate measures, and each activation, in the
//! format of `scripts/window-chrome/RECORD.md`. The controls gate and the design gate read it. Without the variable
//! every call here does nothing and allocates nothing, so an idle window still draws no frames.
//!
//! How a control is measured. [`ControlRecord::wrap`] puts the control in a wrapper that has no border of its own and
//! lays a measuring child over it, so the child's bounds are the control's outer bounds as laid out (layout has
//! already snapped them, `gpui` `taffy.rs` `layout_bounds`) and no border is ever added back. The child inserts a
//! hitbox over those bounds (`Window::insert_hitbox`, `window.rs`), which is where `hit` and the hover state come from.
//! Device pixels are logical pixels times the scale, rounded half toward zero edge by edge, as painting rounds them
//! (`Window::snap_bounds`, `window.rs`, with `round_to_device_pixel` in `util.rs`).
//!
//! The rule for the control builders: the control's interactive element (the one that carries its click, hover and
//! press handlers) must be the element [`ControlRecord::wrap`] wraps and must fill the wrapper, with no margin or
//! padding of the wrapper's own, so the measuring child's hitbox covers exactly what takes the clicks. gpui does not
//! let this module read another element's hitbox, so `hit` can only be as true as that: handlers on some other,
//! smaller or larger element would make `hit` a copy of `box` in effect. The gates' pointer probes one device pixel
//! inside and outside each edge are the check that the real control answers where the record says.
// The control builders call into this module; until they do, nothing in the binary uses it.
#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::{
    AnyElement, App, Bounds, ClickEvent, DispatchPhase, FocusHandle, Hitbox, HitboxBehavior,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Styled, Window,
    canvas, deferred, div, point, px, size,
};
use serde_json::{Value, json};

/// A box in whole device pixels: `[x, y, w, h]` from the window's top-left corner.
pub type DeviceBox = [i32; 4];

/// The environment variable that turns the record on.
const RECORD_VARIABLE: &str = "CLANKY_CONTROL_RECORD";
/// The planted-fault variable, read only by a build with the `control-faults` feature.
#[cfg(feature = "control-faults")]
const FAULT_VARIABLE: &str = "CLANKY_CONTROL_FAULT";
/// How much larger the `hit-larger` fault makes a hit box, in device pixels on each side.
const HIT_LARGER_DEVICE_PX: i32 = 4;

/// Rounds half toward zero, as `util.rs` `round_half_toward_zero` does for painting.
fn round_half_toward_zero(value: f32) -> f32 {
    (value.abs() - 0.5).ceil().copysign(value)
}

/// Device pixels from logical `[x, y, w, h]` at `scale`: each edge rounded, the size the difference of the edges.
pub fn device_box(logical: [f32; 4], scale: f32) -> DeviceBox {
    let edge = |value: f32| round_half_toward_zero(value * scale) as i32;
    let [x, y, w, h] = logical;
    let (left, top) = (edge(x), edge(y));
    let (right, bottom) = (edge(x + w).max(left), edge(y + h).max(top));
    [left, top, right - left, bottom - top]
}

fn logical_of(bounds: Bounds<Pixels>) -> [f32; 4] {
    [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ]
}

/// The fault the gates plant (`CLANKY_CONTROL_FAULT`), if any: a build without the `control-faults` feature has none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Faults {
    #[default]
    None,
    HitLarger,
    PressFires,
    TabSkip,
    BorderTwoPixels,
    PopoverOverOpener,
    EscLosesFocus,
    PaletteNoRun,
    SaveNotWritten,
    MarkerStays,
    SyntaxOffByOne,
    CloseDropsPending,
    SideWidth,
}

impl Faults {
    /// The fault of that name, or none for any other name.
    pub fn from_name(name: &str) -> Self {
        match name {
            "hit-larger" => Self::HitLarger,
            "press-fires" => Self::PressFires,
            "tab-skip" => Self::TabSkip,
            "border-2px" => Self::BorderTwoPixels,
            "popover-over-opener" => Self::PopoverOverOpener,
            "esc-loses-focus" => Self::EscLosesFocus,
            "palette-no-run" => Self::PaletteNoRun,
            "save-not-written" => Self::SaveNotWritten,
            "marker-stays" => Self::MarkerStays,
            "syntax-off-by-one" => Self::SyntaxOffByOne,
            "close-drops-pending" => Self::CloseDropsPending,
            "side-width" => Self::SideWidth,
            _ => Self::None,
        }
    }

    #[cfg(feature = "control-faults")]
    fn from_environment() -> Self {
        std::env::var(FAULT_VARIABLE)
            .map_or_else(|_| Self::default(), |name| Self::from_name(&name))
    }

    #[cfg(not(feature = "control-faults"))]
    fn from_environment() -> Self {
        Self::default()
    }

    /// A hit box 4 device pixels larger than the painted box on every side.
    pub fn hit_larger(self) -> bool {
        self == Self::HitLarger
    }

    /// A control that fires on press instead of on release.
    pub fn press_fires(self) -> bool {
        self == Self::PressFires
    }

    /// Whether the control at this place in the Tab order is left out of it (the third one).
    pub fn tab_skip(self, index: usize) -> bool {
        self == Self::TabSkip && index == 2
    }

    /// A focus border drawn two device pixels wide.
    pub fn border_two_pixels(self) -> bool {
        self == Self::BorderTwoPixels
    }

    /// A popover placed on its opener's top-left corner, covering it.
    pub fn popover_over_opener(self) -> bool {
        self == Self::PopoverOverOpener
    }

    /// Esc closes a popover without giving the focus back to what had it.
    pub fn esc_loses_focus(self) -> bool {
        self == Self::EscLosesFocus
    }

    /// The palette leaves out its last row, the offer to run what was typed in the pane.
    pub fn palette_no_run(self) -> bool {
        self == Self::PaletteNoRun
    }

    /// Ctrl+S in an editor pane that writes nothing to the file.
    pub fn save_not_written(self) -> bool {
        self == Self::SaveNotWritten
    }

    /// An editor pane's pending-change marker that stays once it has shown, after a save too.
    pub fn marker_stays(self) -> bool {
        self == Self::MarkerStays
    }

    /// Editor keywords drawn one ANSI colour along from the theme's (ANSI 6 instead of 5).
    pub fn syntax_off_by_one(self) -> bool {
        self == Self::SyntaxOffByOne
    }

    /// An editor pane that closes with its pending changes, which are lost.
    pub fn close_drops_pending(self) -> bool {
        self == Self::CloseDropsPending
    }

    /// The side column drawn `SIDE_COLUMN_FAULT_EXTRA` wider than its token.
    pub fn side_width(self) -> bool {
        self == Self::SideWidth
    }

    fn hit(self, painted: DeviceBox) -> DeviceBox {
        if self.hit_larger() {
            let grow = HIT_LARGER_DEVICE_PX;
            [
                painted[0] - grow,
                painted[1] - grow,
                painted[2] + 2 * grow,
                painted[3] + 2 * grow,
            ]
        } else {
            painted
        }
    }
}

/// How a control fired: `release` for a mouse click, `key` for Enter or Space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Release,
    Key,
}

impl Via {
    pub fn of(event: &ClickEvent) -> Self {
        if matches!(event, ClickEvent::Keyboard(_)) {
            Self::Key
        } else {
            Self::Release
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Key => "key",
        }
    }
}

/// A part of a control measured on its own: the label text, or the icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    Label,
    Icon,
}

/// What the builder of a control says about it; the rest is measured.
pub struct ControlSpec {
    pub screen: &'static str,
    pub id: u32,
    pub kind: &'static str,
    pub label: String,
    pub enabled: bool,
    pub group: Option<&'static str>,
    pub axis: Option<&'static str>,
    pub checked: Option<bool>,
    pub primary: bool,
    pub focus: Option<FocusHandle>,
    /// A field's text as typed now; written as `field_text` for fields only.
    pub field_text: Option<String>,
}

/// One control as measured in the frame being drawn.
struct Entry {
    spec: ControlSpec,
    painted: DeviceBox,
    hit: DeviceBox,
    state: &'static str,
    focused: bool,
}

type Key = (&'static str, u32);

#[derive(Default)]
struct Frame {
    controls: Vec<Entry>,
    parts: BTreeMap<(&'static str, u32, Part), DeviceBox>,
    landmarks: BTreeMap<(&'static str, u32), DeviceBox>,
    /// The words a landmark shows, for the landmarks that carry text.
    landmark_texts: BTreeMap<(&'static str, u32), String>,
    /// Further fields a landmark's line carries (the side tabs' rows: tab, sub, mark, state and the rest).
    landmark_fields: BTreeMap<(&'static str, u32), serde_json::Map<String, Value>>,
    /// Each open popover's `popover` object, by the screen it belongs to.
    popovers: BTreeMap<&'static str, Value>,
    keyboard: bool,
    scale: f32,
}

struct Writer {
    out: Box<dyn Write>,
    faults: Faults,
    /// The last text written for each screen and landmark, so only changes are written.
    last: BTreeMap<String, String>,
    screens: BTreeSet<&'static str>,
    pressed: Option<Key>,
    frame: Frame,
}

/// The record's handle: cheap to clone, and inert when the record is off.
#[derive(Clone, Default)]
pub struct ControlRecord(Option<Rc<RefCell<Writer>>>);

impl ControlRecord {
    /// On when `CLANKY_CONTROL_RECORD` names a file, which is created or appended to.
    pub fn from_environment() -> io::Result<Self> {
        Self::from_path(
            std::env::var_os(RECORD_VARIABLE)
                .map(PathBuf::from)
                .as_deref(),
        )
    }

    /// On when given a path; the same as the environment variable, for callers and tests that pass it.
    pub fn from_path(path: Option<&Path>) -> io::Result<Self> {
        let Some(path) = path else {
            return Ok(Self(None));
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("opening the control record {}: {error}", path.display()),
                )
            })?;
        Ok(Self::to_writer(Box::new(file)))
    }

    fn to_writer(out: Box<dyn Write>) -> Self {
        let writer = Writer {
            out,
            faults: Faults::from_environment(),
            last: BTreeMap::new(),
            screens: BTreeSet::new(),
            pressed: None,
            frame: Frame::default(),
        };
        Self(Some(Rc::new(RefCell::new(writer))))
    }

    pub fn enabled(&self) -> bool {
        self.0.is_some()
    }

    /// The planted faults of this run; none unless the build has `control-faults` and the variable names one.
    pub fn faults(&self) -> Faults {
        self.0
            .as_ref()
            .map_or_else(Faults::default, |writer| writer.borrow().faults)
    }

    /// Called once at the start of every frame the window draws, before its controls are laid out.
    pub fn begin_frame(&self) {
        if let Some(writer) = &self.0 {
            writer.borrow_mut().frame = Frame::default();
        }
    }

    /// Wraps a control: the control itself, plus a measuring child over its outer bounds.
    pub fn wrap(&self, spec: ControlSpec, control: impl IntoElement) -> AnyElement {
        let Some(writer) = self.0.clone() else {
            return control.into_any_element();
        };
        let faults = self.faults();
        let overlay = canvas(
            move |bounds, window, _| {
                let scale = window.scale_factor();
                let painted = device_box(logical_of(bounds), scale);
                let hit = faults.hit(painted);
                let hit_bounds = Bounds::new(
                    point(px(hit[0] as f32 / scale), px(hit[1] as f32 / scale)),
                    size(px(hit[2] as f32 / scale), px(hit[3] as f32 / scale)),
                );
                (
                    window.insert_hitbox(hit_bounds, HitboxBehavior::Normal),
                    painted,
                    hit,
                )
            },
            move |_, (hitbox, painted, hit), window, _| {
                measure_control(&writer, spec, &hitbox, painted, hit, window);
            },
        )
        .size_full();
        div()
            .relative()
            .flex_none()
            .child(control)
            .child(div().absolute().top_0().left_0().size_full().child(overlay))
            .into_any_element()
    }

    /// A measuring child for the label or the icon of a control: place it, absolute and full size, inside the
    /// element that holds the text or the icon.
    pub fn part(&self, screen: &'static str, id: u32, part: Part) -> AnyElement {
        let Some(writer) = self.0.clone() else {
            return div().into_any_element();
        };
        let overlay = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let measured = device_box(logical_of(bounds), window.scale_factor());
                writer
                    .borrow_mut()
                    .frame
                    .parts
                    .insert((screen, id, part), measured);
            },
        )
        .size_full();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(overlay)
            .into_any_element()
    }

    /// A measuring child for a landmark (`title-bar`, `status-line`, `pane`, `status-count`, `meta-text`,
    /// `pane-text`): place it, absolute and full size, inside the element it names.
    pub fn landmark(&self, name: &'static str, index: u32) -> AnyElement {
        let Some(writer) = self.0.clone() else {
            return div().into_any_element();
        };
        let overlay = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let measured = device_box(logical_of(bounds), window.scale_factor());
                writer
                    .borrow_mut()
                    .frame
                    .landmarks
                    .insert((name, index), measured);
            },
        )
        .size_full();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(overlay)
            .into_any_element()
    }

    /// A landmark that also records the words it shows (`text` in its line).
    pub fn landmark_with_text(&self, name: &'static str, index: u32, text: String) -> AnyElement {
        if let Some(writer) = &self.0 {
            writer
                .borrow_mut()
                .frame
                .landmark_texts
                .insert((name, index), text);
        }
        self.landmark(name, index)
    }

    /// A landmark whose line also carries `fields` (each written next to `box`).
    pub fn landmark_with_fields(
        &self,
        name: &'static str,
        index: u32,
        fields: serde_json::Map<String, Value>,
    ) -> AnyElement {
        self.set_landmark_fields(name, index, fields);
        self.landmark(name, index)
    }

    /// Fields for a landmark's line, whether it is measured on screen or written absent.
    pub fn set_landmark_fields(
        &self,
        name: &'static str,
        index: u32,
        fields: serde_json::Map<String, Value>,
    ) {
        if let Some(writer) = &self.0 {
            writer
                .borrow_mut()
                .frame
                .landmark_fields
                .insert((name, index), fields);
        }
    }

    /// A landmark not on screen in this frame: written as an empty box at the window's corner, unless something drawn
    /// later in the frame measures it.
    pub fn landmark_absent(&self, name: &'static str, index: u32) {
        if let Some(writer) = &self.0 {
            writer
                .borrow_mut()
                .frame
                .landmarks
                .insert((name, index), [0, 0, 0, 0]);
        }
    }

    /// A measuring child for a popover: place it, absolute and full size, inside the element that holds the popover's
    /// outer box (its border included). It writes the screen line's `popover` object: that box, the opener's box
    /// (`opener`, logical `[x, y, w, h]` in the window), and the opener's control id or pane id.
    pub fn popover(
        &self,
        screen: &'static str,
        opener: [f32; 4],
        opener_id: Option<u32>,
        opener_pane: Option<String>,
    ) -> AnyElement {
        let Some(writer) = self.0.clone() else {
            return div().into_any_element();
        };
        let overlay = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let scale = window.scale_factor();
                let line = json!({
                    "box": device_box(logical_of(bounds), scale), "opener": device_box(opener, scale),
                    "opener_id": opener_id, "opener_pane": opener_pane,
                });
                let mut writer = writer.borrow_mut();
                writer.screens.insert(screen);
                writer.frame.popovers.insert(screen, line);
            },
        )
        .size_full();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(overlay)
            .into_any_element()
    }

    /// Placed in the window: when the frame has drawn, writes what changed since the last one. It is deferred behind
    /// every other deferred element (popovers paint after the window's own tree), so their controls are measured
    /// before it writes.
    pub fn flush(&self) -> AnyElement {
        let Some(writer) = self.0.clone() else {
            return div().into_any_element();
        };
        deferred(canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let mut writer = writer.borrow_mut();
                writer.frame.keyboard = window.last_input_was_keyboard();
                writer.frame.scale = window.scale_factor();
                if let Err(error) = writer.write_changes() {
                    eprintln!("writing the control record: {error}");
                }
            },
        ))
        .priority(usize::MAX)
        .into_any_element()
    }

    /// Writes an activation line. Nothing is written on press: a control fires on release or on a key.
    pub fn activate(&self, screen: &'static str, id: u32, label: &str, via: Via) {
        let Some(writer) = &self.0 else { return };
        let line = json!({"record": "activate", "screen": screen, "id": id, "label": label, "via": via.name()});
        if let Err(error) = writer.borrow_mut().write_line(&line) {
            eprintln!("writing the control record: {error}");
        }
    }
}

fn measure_control(
    writer: &Rc<RefCell<Writer>>,
    spec: ControlSpec,
    hitbox: &Hitbox,
    painted: DeviceBox,
    hit: DeviceBox,
    window: &mut Window,
) {
    let key = (spec.screen, spec.id);
    let hovered = hitbox.is_hovered(window);
    let focused = spec
        .focus
        .as_ref()
        .is_some_and(|handle| handle.is_focused(window));
    let keyboard = window.last_input_was_keyboard();
    let pressed = writer.borrow().pressed == Some(key);
    let state = control_state(
        spec.enabled,
        interaction(pressed && hovered, focused && keyboard, hovered),
    );
    {
        let mut inner = writer.borrow_mut();
        inner.screens.insert(spec.screen);
        inner.frame.controls.push(Entry {
            spec,
            painted,
            hit,
            state,
            focused,
        });
    }
    let (down, up) = (Rc::clone(writer), Rc::clone(writer));
    let hit_for_down = hitbox.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _: &mut App| {
        if phase == DispatchPhase::Bubble
            && event.button == MouseButton::Left
            && hit_for_down.is_hovered(window)
        {
            down.borrow_mut().pressed = Some(key);
            window.refresh();
        }
    });
    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _: &mut App| {
        if phase == DispatchPhase::Bubble && up.borrow().pressed == Some(key) {
            up.borrow_mut().pressed = None;
            window.refresh();
        }
    });
}

/// What the pointer and keyboard are doing to a control, the strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interaction {
    Pressed,
    KeyboardFocus,
    Hover,
    Idle,
}

/// The interaction, in the order the rulebook's states table gives precedence: pressed, then keyboard focus, then
/// hover.
fn interaction(pressed: bool, keyboard_focus: bool, hovered: bool) -> Interaction {
    if pressed {
        Interaction::Pressed
    } else if keyboard_focus {
        Interaction::KeyboardFocus
    } else if hovered {
        Interaction::Hover
    } else {
        Interaction::Idle
    }
}

/// A control's state name: a disabled control is disabled whatever else happens to it.
fn control_state(enabled: bool, interaction: Interaction) -> &'static str {
    if !enabled {
        return "disabled";
    }
    match interaction {
        Interaction::Pressed => "pressed",
        Interaction::KeyboardFocus => "focus",
        Interaction::Hover => "hover",
        Interaction::Idle => "rest",
    }
}

fn opt_box(measured: Option<&DeviceBox>) -> Value {
    measured.map_or(Value::Null, |b| json!(b))
}

impl Writer {
    fn write_line(&mut self, line: &Value) -> io::Result<()> {
        writeln!(self.out, "{line}")?;
        self.out.flush()
    }

    /// Writes the lines of the frame that differ from the last ones written: landmarks, then one line per screen.
    fn write_changes(&mut self) -> io::Result<()> {
        let landmarks: Vec<(String, Value)> = self
            .frame
            .landmarks
            .iter()
            .map(|((name, index), measured)| {
                let mut line =
                    json!({"record": "landmark", "name": name, "index": index, "box": measured});
                if let Some(text) = self.frame.landmark_texts.get(&(*name, *index)) {
                    line["text"] = json!(text);
                }
                if let Some(fields) = self.frame.landmark_fields.get(&(*name, *index)) {
                    for (key, value) in fields {
                        line[key.as_str()] = value.clone();
                    }
                }
                (format!("landmark {name} {index}"), line)
            })
            .collect();
        let screens: Vec<(String, Value)> = self
            .screens
            .iter()
            .map(|screen| (format!("screen {screen}"), self.screen_line(screen)))
            .collect();
        for (key, line) in landmarks.into_iter().chain(screens) {
            let text = line.to_string();
            if self.last.get(&key) != Some(&text) {
                self.write_line(&line)?;
                self.last.insert(key, text);
            }
        }
        Ok(())
    }

    /// The screen line: its controls in the frame, the focused control's id, and its popover's place when one is
    /// open.
    fn screen_line(&self, screen: &'static str) -> Value {
        let frame = &self.frame;
        let controls: Vec<Value> = frame
            .controls
            .iter()
            .filter(|entry| entry.spec.screen == screen)
            .map(|entry| {
                let spec = &entry.spec;
                let mut line = json!({
                    "id": spec.id, "kind": spec.kind, "label": spec.label, "box": entry.painted,
                    "hit": entry.hit, "state": entry.state, "enabled": spec.enabled, "tab": spec.enabled,
                    "group": spec.group, "axis": spec.axis, "checked": spec.checked, "primary": spec.primary,
                    "label_box": opt_box(frame.parts.get(&(screen, spec.id, Part::Label))),
                    "icon": opt_box(frame.parts.get(&(screen, spec.id, Part::Icon))),
                });
                if let Some(text) = &spec.field_text {
                    line["field_text"] = json!(text);
                }
                line
            })
            .collect();
        let focus = frame
            .controls
            .iter()
            .find(|entry| entry.spec.screen == screen && entry.focused)
            .map(|entry| entry.spec.id);
        json!({
            "record": "screen", "screen": screen, "scale": frame.scale, "focus": focus,
            "keyboard": frame.keyboard, "popover": frame.popovers.get(screen).cloned().unwrap_or(Value::Null),
            "controls": controls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shared buffer the record writes into, so a test can read what was written.
    #[derive(Clone, Default)]
    struct Sink(Rc<RefCell<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Sink {
        fn lines(&self) -> Vec<Value> {
            let text = String::from_utf8(self.0.borrow().clone()).expect("the record is UTF-8");
            text.lines()
                .map(|line| serde_json::from_str(line).expect("each line is JSON"))
                .collect()
        }
    }

    /// The fake control of the spec: its logical bounds are whole device pixels over 1.5.
    fn fake_control_logical() -> [f32; 4] {
        [100.0 / 1.5, 20.0 / 1.5, 30.0 / 1.5, 28.0 / 1.5]
    }

    #[test]
    fn a_box_at_scale_one_and_a_half_records_whole_device_pixels() {
        let painted = device_box(fake_control_logical(), 1.5);
        assert_eq!(painted, [100, 20, 30, 28]);
        assert_eq!(Faults::default().hit(painted), [100, 20, 30, 28]);
    }

    #[test]
    fn the_hit_larger_fault_grows_the_hit_box_four_pixels_each_way() {
        let painted = device_box(fake_control_logical(), 1.5);
        let faults = Faults::from_name("hit-larger");
        assert_eq!(faults.hit(painted), [96, 16, 38, 36]);
        assert!(faults.hit_larger());
        assert!(!faults.press_fires());
    }

    #[test]
    fn rounding_follows_painting_for_half_pixels_and_never_goes_negative() {
        assert_eq!(device_box([0.25, 0.25, 0.5, 0.5], 2.0), [0, 0, 1, 1]);
        assert_eq!(device_box([10.5, 10.5, 1.0, 1.0], 1.0), [10, 10, 1, 1]);
        assert_eq!(device_box([5.0, 5.0, -3.0, -3.0], 1.0), [5, 5, 0, 0]);
    }

    #[test]
    fn the_other_faults_are_named_and_unknown_names_plant_nothing() {
        assert!(Faults::from_name("press-fires").press_fires());
        assert!(Faults::from_name("border-2px").border_two_pixels());
        assert!(Faults::from_name("popover-over-opener").popover_over_opener());
        assert!(Faults::from_name("esc-loses-focus").esc_loses_focus());
        let skipping = Faults::from_name("tab-skip");
        assert!(skipping.tab_skip(2) && !skipping.tab_skip(1));
        assert_eq!(Faults::from_name("nonsense"), Faults::default());
    }

    #[test]
    fn with_no_path_nothing_is_written_and_every_call_is_inert() {
        let record = ControlRecord::from_path(None).expect("no path is not an error");
        assert!(!record.enabled());
        record.begin_frame();
        record.activate("main-window", 1, "Files", Via::Release);
        assert_eq!(record.faults(), Faults::default());
    }

    #[test]
    fn a_state_is_chosen_in_the_order_of_the_states_table() {
        assert_eq!(
            control_state(false, interaction(true, true, true)),
            "disabled"
        );
        assert_eq!(
            control_state(true, interaction(true, true, true)),
            "pressed"
        );
        assert_eq!(control_state(true, interaction(false, true, true)), "focus");
        assert_eq!(
            control_state(true, interaction(false, false, true)),
            "hover"
        );
        assert_eq!(
            control_state(true, interaction(false, false, false)),
            "rest"
        );
    }

    fn record_into(sink: &Sink) -> ControlRecord {
        ControlRecord::to_writer(Box::new(sink.clone()))
    }

    fn spec(id: u32, label: &str) -> ControlSpec {
        ControlSpec {
            screen: "main-window",
            id,
            kind: "button",
            label: label.to_owned(),
            enabled: true,
            group: None,
            axis: None,
            checked: None,
            primary: false,
            focus: None,
            field_text: None,
        }
    }

    fn fill(record: &ControlRecord, hit: DeviceBox) {
        let writer = record.0.as_ref().expect("the record is on");
        let mut writer = writer.borrow_mut();
        writer.frame = Frame {
            scale: 1.5,
            keyboard: false,
            ..Frame::default()
        };
        writer.screens.insert("main-window");
        let painted = device_box(fake_control_logical(), 1.5);
        writer.frame.controls.push(Entry {
            spec: spec(7, "Files"),
            painted,
            hit,
            state: "rest",
            focused: false,
        });
        writer
            .frame
            .parts
            .insert(("main-window", 7, Part::Label), [110, 20, 10, 28]);
        writer
            .frame
            .landmarks
            .insert(("title-bar", 0), [0, 0, 1800, 60]);
    }

    #[test]
    fn a_frame_is_written_as_the_record_format_says_and_only_when_it_changed() {
        let sink = Sink::default();
        let record = record_into(&sink);
        fill(&record, [100, 20, 30, 28]);
        let writer = record.0.as_ref().expect("the record is on");
        writer
            .borrow_mut()
            .write_changes()
            .expect("writing to memory");
        writer
            .borrow_mut()
            .write_changes()
            .expect("writing to memory");
        let lines = sink.lines();
        assert_eq!(
            lines.len(),
            2,
            "one landmark and one screen line, the repeat writes nothing: {lines:?}"
        );
        assert_eq!(
            lines[0],
            json!({"record": "landmark", "name": "title-bar", "index": 0, "box": [0, 0, 1800, 60]})
        );
        assert_eq!(lines[1]["record"], "screen");
        assert_eq!(lines[1]["scale"], 1.5);
        let control = &lines[1]["controls"][0];
        assert_eq!(control["box"], json!([100, 20, 30, 28]));
        assert_eq!(control["hit"], json!([100, 20, 30, 28]));
        assert_eq!(control["label_box"], json!([110, 20, 10, 28]));
        assert_eq!(control["icon"], Value::Null);
        assert_eq!(control["state"], "rest");
        fill(&record, [96, 16, 38, 36]);
        writer
            .borrow_mut()
            .write_changes()
            .expect("writing to memory");
        assert_eq!(sink.lines().len(), 3, "a changed hit box is written again");
    }

    #[test]
    fn an_activation_line_names_the_screen_control_and_way() {
        let sink = Sink::default();
        let record = record_into(&sink);
        record.activate("main-window", 7, "Files", Via::Key);
        assert_eq!(
            sink.lines(),
            vec![
                json!({"record": "activate", "screen": "main-window", "id": 7, "label": "Files", "via": "key"})
            ]
        );
    }
}
