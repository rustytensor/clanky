//! The Settings window: a dialog over the main window with a header holding the section switcher (the seven pages),
//! a body of setting cards that scrolls, and an error banner at its foot while the settings file has a problem.
//!
//! Each page is a module of its own under `settings/pages/` and only says which rows it has ([`Row`]): a title, a
//! description, the config key and the control with its value now. This module lays every page out the same way
//! (the rulebook's Settings row: a card per setting with its title and description on the left and its control on
//! the right, group labels above the groups), builds the controls with the window's control kit, and writes every
//! change through `clanky_state::settings::Settings`, the one store of the settings file, which keeps the file's
//! key order and validates each change. The window that holds Settings applies a change to itself at once
//! (`MainWindow::apply_config`) and looks at the file now and then, so an edit made outside shows too.
//!
//! The control record writes the screen `settings` (`scripts/window-chrome/RECORD.md`, G4 contract): the page, the
//! scroll position, every control with `visible`, the field's text as typed, the slider's value and range, and the
//! landmarks the design gate measures.

pub mod pages;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
};

use clanky_state::{
    config::Config,
    keys::{Chord, Key},
    settings::Settings,
    themes::Colors,
};
use gpui::{
    AnyElement, App, Bounds, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, ScrollWheelEvent, SharedString, Styled, Window, canvas, div, point, px,
};
use serde_json::{Value, json};

use crate::{
    control_look::paint,
    controls::{Control, Group, INTERFACE_FONT_FAMILY, Kind, Kit},
    tokens as t,
};

/// The Settings window's name in the control record.
pub const SCREEN: &str = "settings";
/// The section switcher's segments, in the record's group `settings-pages`.
const PAGES_GROUP: Group = Group::horizontal("settings-pages");
/// Control ids: the page segments, then one per row of the page shown, and each segmented row's options.
const FIRST_PAGE_ID: u32 = 10;
const FIRST_ROW_ID: u32 = 200;
const FIRST_OPTION_ID: u32 = 400;
const OPTIONS_PER_ROW: u32 = 10;

/// The settings that one change writes: a key and its new value, or None to take the key out of the file.
pub type Changes = Vec<(&'static str, Option<Value>)>;
/// Turns a field's text into the value its key stores, or says what is wrong with it.
pub type Parse = fn(&str) -> Result<Value, String>;
/// What a row writes for the value its control chose, when that is more than its own key set to the value.
pub type Writes = Rc<dyn Fn(&Value) -> Changes>;

/// The seven pages, in the switcher's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    General,
    Appearance,
    Terminal,
    Editor,
    Agents,
    History,
    Keys,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::General,
        Page::Appearance,
        Page::Terminal,
        Page::Editor,
        Page::Agents,
        Page::History,
        Page::Keys,
    ];

    /// The page's name: the switcher's segment and the record's `page`.
    pub fn label(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Terminal => "Terminal",
            Page::Editor => "Editor",
            Page::Agents => "Agents",
            Page::History => "History",
            Page::Keys => "Keys",
        }
    }

    /// The page's rows for the settings now.
    pub fn rows(self, context: &PageContext<'_>) -> Vec<Row> {
        match self {
            Page::General => pages::general::rows(context),
            Page::Appearance => pages::appearance::rows(context),
            Page::Terminal => pages::terminal::rows(context),
            Page::Editor => pages::editor::rows(context),
            Page::Agents => pages::agents::rows(context),
            Page::History => pages::history::rows(context),
            Page::Keys => pages::keys::rows(context),
        }
    }
}

/// What a page knows when it lists its rows.
pub struct PageContext<'a> {
    /// The settings as the file holds them now (its defaults where it sets nothing).
    pub config: &'a Config,
}

/// One setting of a page, drawn as a card.
pub struct Row {
    /// The config key the control writes; empty for a row that sets nothing (a shortcut on the Keys page).
    pub key: &'static str,
    /// The card's title, which is also the control's label in the record.
    pub label: SharedString,
    pub description: SharedString,
    /// The label of the group this row starts, drawn above its card.
    pub group: Option<&'static str>,
    pub control: RowControl,
    /// What choosing a value writes, when not just `key` set to it.
    pub writes: Option<Writes>,
}

/// A row's control, with its value now.
// Every kind is laid out here; the pages that have a slider or shortcuts are the ones to construct those.
#[allow(dead_code)]
pub enum RowControl {
    /// On or off; a click writes the other.
    Switch { on: bool },
    /// One option of several: each option's text and the value it stores; `chosen` is the one the setting has
    /// now, if any.
    Segments {
        options: Vec<(SharedString, Value)>,
        chosen: Option<usize>,
    },
    /// Text typed in place: `text` is the setting now as the field shows it; Enter (or leaving the field) writes
    /// what `parse` makes of the typed text, Escape puts the setting's text back.
    Field { text: String, parse: Parse },
    /// A value along a track: a click at a share of the track's width sets `min + share x (max - min)`, the arrows
    /// move it by `step`; the value written is rounded to the step.
    Slider {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    /// No control: the keys that do the row's command, shown at the card's right end (the Keys page).
    Keys(SharedString),
}

// The constructors and parsers below are the pages' kit; each page uses the ones its rows need.
#[allow(dead_code)]
impl Row {
    fn new(
        key: &'static str,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        control: RowControl,
    ) -> Self {
        Row {
            key,
            label: label.into(),
            description: description.into(),
            group: None,
            control,
            writes: None,
        }
    }

    /// A switch for a setting that is true or false.
    pub fn switch(
        key: &'static str,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        on: bool,
    ) -> Self {
        Self::new(key, label, description, RowControl::Switch { on })
    }

    /// Segments for a setting with a few values: each option's text and the value it stores, and the value the
    /// setting has now, which checks the option that stores it.
    pub fn segments(
        key: &'static str,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        options: &[(&str, Value)],
        now: &Value,
    ) -> Self {
        let chosen = options.iter().position(|(_, value)| value == now);
        let options = options
            .iter()
            .map(|(text, value)| (SharedString::from(text.to_string()), value.clone()))
            .collect();
        Self::new(
            key,
            label,
            description,
            RowControl::Segments { options, chosen },
        )
    }

    /// A field: the setting's text now, and how typed text becomes its value ([`whole_number`], [`number`],
    /// [`text`], or a function of the page's own).
    pub fn field(
        key: &'static str,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        now: impl Into<String>,
        parse: Parse,
    ) -> Self {
        Self::new(
            key,
            label,
            description,
            RowControl::Field {
                text: now.into(),
                parse,
            },
        )
    }

    /// A slider over `min..=max` in steps of `step`, at the setting's value now.
    pub fn slider(
        key: &'static str,
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        now: f64,
        range: (f64, f64),
        step: f64,
    ) -> Self {
        Self::new(
            key,
            label,
            description,
            RowControl::Slider {
                value: now,
                min: range.0,
                max: range.1,
                step,
            },
        )
    }

    /// A row that sets nothing and shows the keys of a command.
    pub fn keys(
        label: impl Into<SharedString>,
        description: impl Into<SharedString>,
        keys: impl Into<SharedString>,
    ) -> Self {
        Self::new("", label, description, RowControl::Keys(keys.into()))
    }

    /// The row starts a group with this label.
    #[must_use]
    pub fn in_group(mut self, label: &'static str) -> Self {
        self.group = Some(label);
        self
    }

    /// What choosing a value writes, in place of the row's key set to it (a row that writes two keys, or a value
    /// that depends on another setting).
    #[must_use]
    pub fn writes(mut self, writes: impl Fn(&Value) -> Changes + 'static) -> Self {
        self.writes = Some(Rc::new(writes));
        self
    }

    /// The settings the row writes for a chosen value.
    fn changes(&self, value: Value) -> Changes {
        match &self.writes {
            Some(writes) => writes(&value),
            None => vec![(self.key, Some(value))],
        }
    }
}

/// A field's text as a whole number of at least zero.
pub fn whole_number(text: &str) -> Result<Value, String> {
    text.trim()
        .parse::<u64>()
        .map(|number| json!(number))
        .map_err(|_| format!("\u{201c}{}\u{201d} is not a whole number", text.trim()))
}

/// A field's text as a number; a whole one is stored without a fraction.
#[allow(dead_code)] // for the pages whose fields hold such values
pub fn number(text: &str) -> Result<Value, String> {
    let number = text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("\u{201c}{}\u{201d} is not a number", text.trim()))?;
    Ok(if number.fract() == 0.0 && number.abs() < 1e15 {
        json!(number as i64)
    } else {
        json!(number)
    })
}

/// A field's text as it was typed.
#[allow(dead_code)] // for the pages whose fields hold such values
pub fn text(text: &str) -> Result<Value, String> {
    Ok(json!(text))
}

/// A number as a field shows it: as short as it reads, a whole one without a fraction (`1`, `1.25`, `14.67`).
#[allow(dead_code)] // for the pages whose fields hold such values
pub fn number_text(value: f32) -> String {
    format!("{value}")
}

/// A value rounded to a step and to six places, so a step of 0.01 writes 0.8 and not 0.8000000000000002.
fn on_step(value: f64, step: f64) -> f64 {
    let stepped = if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    };
    (stepped * 1e6).round() / 1e6
}

/// The record's group of a segmented row, `row:KEY`. Group names live as long as the window, so each is made once.
fn row_group(key: &'static str) -> &'static str {
    thread_local! {
        static NAMES: RefCell<HashMap<&'static str, &'static str>> = RefCell::default();
    }
    NAMES.with(|names| {
        *names
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| Box::leak(format!("row:{key}").into_boxed_str()))
    })
}

/// What a control of the Settings window asks of the window that holds it.
pub enum Request {
    Page(Page),
    Store(Changes),
    /// A click on the field of row `index`: it takes the keyboard and starts an edit.
    Edit(usize),
    /// A click on the slider of row `index`, at the pointer.
    Slide(usize),
    /// The wheel over the body, in logical px down.
    Scroll(f32),
}

/// How a control's action reaches the window that holds Settings.
pub type Host = Rc<dyn Fn(Request, &mut Window, &mut App)>;

/// What a key did in Settings.
pub enum KeyOutcome {
    /// Not one of Settings' keys: it goes on to the focused control.
    Ignored,
    Taken,
    /// Escape with no edit open: Settings closes (Ctrl+, is the window's own key, which closes it too).
    Close,
    /// Settings to write (a committed field, a slider's arrow).
    Store(Changes),
}

/// A field's edit: the row, the text as typed, and whether Ctrl+A selected all of it (typing then replaces it).
struct Edit {
    row: usize,
    text: String,
    select_all: bool,
}

/// The Settings window's state, kept by the main window.
pub struct SettingsWindow {
    /// The settings file's store, or why there is none.
    store: Result<Settings, String>,
    open: bool,
    page: Page,
    /// How far the body is scrolled, in logical px.
    scroll: f32,
    /// The body's box and the page's height as last laid out, for the most the body scrolls.
    body: Rc<Cell<Option<Bounds<Pixels>>>>,
    content_height: Rc<Cell<f32>>,
    /// How many cards and group labels the last frame drew: their landmarks are written absent when a page with
    /// fewer of them, or none, takes their place.
    drawn: (u32, u32),
    /// The panel's own focus, which holds the keyboard while no control of it does.
    focus: FocusHandle,
    /// What had the keyboard when Settings opened.
    give_back: Option<FocusHandle>,
    /// Each field's focus, by its row's place on the page.
    fields: Vec<FocusHandle>,
    edit: Option<Edit>,
    /// The last change's failure, shown in the banner until the next change succeeds.
    problem: Option<String>,
}

impl SettingsWindow {
    /// Settings over the settings file at `path`; with none, Settings shows the defaults and says nothing can be
    /// saved.
    pub fn new(path: Option<PathBuf>, cx: &mut App) -> Self {
        let store = path
            .ok_or_else(|| "there is no settings file to save to".to_owned())
            .and_then(Settings::open);
        SettingsWindow {
            store,
            open: false,
            page: Page::General,
            scroll: 0.0,
            body: Rc::default(),
            content_height: Rc::default(),
            drawn: (0, 0),
            focus: cx.focus_handle(),
            give_back: None,
            fields: Vec::new(),
            edit: None,
            problem: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The settings as the store holds them, if there is a store.
    pub fn config(&self) -> Option<&Config> {
        self.store.as_ref().ok().map(Settings::config)
    }

    /// Opens on the General page with the keyboard, remembering what had it.
    pub fn open(&mut self, window: &mut Window, cx: &mut App) {
        if self.open {
            return;
        }
        self.give_back = window.focused(cx);
        self.open = true;
        self.page = Page::General;
        self.scroll = 0.0;
        self.edit = None;
        window.focus(&self.focus, cx);
    }

    /// Closes, dropping an edit not committed, and gives the keyboard back to what had it at open.
    pub fn close(&mut self, window: &mut Window, cx: &mut App) {
        if !self.open {
            return;
        }
        self.open = false;
        self.edit = None;
        if let Some(handle) = self.give_back.take() {
            window.focus(&handle, cx);
        }
    }

    /// The rows of the page shown.
    fn rows(&self) -> Vec<Row> {
        let defaults;
        let config = match self.config() {
            Some(config) => config,
            None => {
                defaults = Config::window_defaults();
                &defaults
            }
        };
        self.page.rows(&PageContext { config })
    }

    /// Writes settings to the file; on success the settings before and after, for the window to apply.
    pub fn store(&mut self, changes: &[(&'static str, Option<Value>)]) -> Option<(Config, Config)> {
        let store = match self.store.as_mut() {
            Ok(store) => store,
            Err(why) => {
                self.problem = Some(format!("Not saved: {why}"));
                return None;
            }
        };
        let before = store.config().clone();
        match store.set_many(changes) {
            Ok(after) => {
                self.problem = None;
                Some((before, after.clone()))
            }
            Err(error) => {
                self.problem = Some(error);
                None
            }
        }
    }

    /// Looks at the settings file for a change made outside the window. `Some` with the settings before and
    /// after when it holds new valid settings; the bool is true when anything Settings shows changed.
    pub fn poll(&mut self) -> (Option<(Config, Config)>, bool) {
        let Ok(store) = self.store.as_mut() else {
            return (None, false);
        };
        let before = store.config().clone();
        let shown = store.error().map(str::to_owned);
        match store.reload() {
            Ok(true) => (Some((before, store.config().clone())), true),
            Ok(false) => (None, false),
            Err(_) => (None, store.error().map(str::to_owned) != shown),
        }
    }

    /// The banner's words: the last change's failure, else the settings file's own problem.
    fn banner(&self) -> Option<String> {
        self.problem.clone().or_else(|| {
            self.store
                .as_ref()
                .ok()
                .and_then(|store| store.error().map(str::to_owned))
        })
    }

    pub fn show_page(&mut self, page: Page, window: &mut Window, cx: &mut App) {
        self.page = page;
        self.scroll = 0.0;
        self.edit = None;
        // A field of the page left behind may hold the keyboard; the panel takes it back.
        if self.fields.iter().any(|field| field.is_focused(window)) {
            window.focus(&self.focus, cx);
        }
    }

    /// The wheel over the body: scrolled by `delta` logical px, kept within the page and on whole device px.
    pub fn scroll_by(&mut self, delta: f32, scale: f32) {
        self.scroll = self.clamped(self.scroll + delta, scale);
    }

    /// The most the body scrolls, from the last layout.
    fn max_scroll(&self) -> f32 {
        let body = self
            .body
            .get()
            .map_or(0.0, |bounds| f32::from(bounds.size.height));
        (self.content_height.get() - body).max(0.0)
    }

    fn clamped(&self, scroll: f32, scale: f32) -> f32 {
        let scroll = scroll.clamp(0.0, self.max_scroll());
        (scroll * scale).round() / scale
    }

    /// A click on a field: the edit starts with the setting's text.
    pub fn begin_edit(&mut self, row: usize) {
        if self.edit.as_ref().is_some_and(|edit| edit.row == row) {
            return;
        }
        if let Some(RowControl::Field { text, .. }) = self.rows().get(row).map(|row| &row.control) {
            self.edit = Some(Edit {
                row,
                text: text.clone(),
                select_all: false,
            });
        }
    }

    /// The edit's text, as the row's parse makes it a value: the settings to write, or None (the banner says why
    /// when the text is not a value).
    fn commit(&mut self) -> Option<Changes> {
        let edit = self.edit.take()?;
        let rows = self.rows();
        let row = rows.get(edit.row)?;
        let RowControl::Field { text, parse } = &row.control else {
            return None;
        };
        if *text == edit.text {
            return None;
        }
        match parse(&edit.text) {
            Ok(value) => Some(row.changes(value)),
            Err(why) => {
                self.problem = Some(format!("{}: {why}", row.label));
                None
            }
        }
    }

    /// Called at the start of each frame: an edit whose field lost the keyboard (a click elsewhere) is committed.
    pub fn frame(&mut self, window: &Window) -> Option<Changes> {
        let row = self.edit.as_ref()?.row;
        let focused = self
            .fields
            .get(row)
            .is_some_and(|field| field.is_focused(window));
        if focused { None } else { self.commit() }
    }

    /// A click on a slider: the value at the pointer's place along its track, as the settings to write.
    pub fn slide(&self, row: usize, kit: &Kit, window: &Window) -> Option<Changes> {
        let rows = self.rows();
        let row_now = rows.get(row)?;
        let RowControl::Slider { min, max, step, .. } = row_now.control else {
            return None;
        };
        let bounds = kit.bounds_of(SCREEN, FIRST_ROW_ID + row as u32)?;
        let scale = window.scale_factor();
        // In device pixels, as the pointer lands: the track's first pixel is the least, its last the most.
        let left = (f32::from(bounds.origin.x) * scale).round();
        let right = ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale).round();
        let pointer = f32::from(window.mouse_position().x) * scale;
        let share = f64::from(((pointer - left) / (right - left - 1.0).max(1.0)).clamp(0.0, 1.0));
        Some(row_now.changes(json!(on_step(min + share * (max - min), step))))
    }

    /// Keys while Settings is open, before the focused control's own: Ctrl+, and Escape close it (Escape first
    /// takes back an edit), Tab walks its controls and scrolls the one reached into view, and a field takes typing.
    pub fn key(
        &mut self,
        chord: Chord,
        typed: Option<&str>,
        kit: &Kit,
        window: &mut Window,
        cx: &mut App,
    ) -> KeyOutcome {
        let plain = !chord.mods.ctrl && !chord.mods.alt;
        match chord.key {
            Key::Escape if plain => {
                if self.edit.take().is_some() {
                    return KeyOutcome::Taken;
                }
                return KeyOutcome::Close;
            }
            Key::Tab if plain => {
                if !kit.focus_step(!chord.mods.shift, window, cx) {
                    kit.focus_first_control(window, cx);
                }
                self.scroll_to_focus(kit, window);
                return KeyOutcome::Taken;
            }
            _ => {}
        }
        if let Some(row) = self
            .fields
            .iter()
            .position(|field| field.is_focused(window))
        {
            return self.field_key(row, chord, typed);
        }
        if let Some(step) = match chord.key {
            Key::Left | Key::Down if plain => Some(-1.0),
            Key::Right | Key::Up if plain => Some(1.0),
            _ => None,
        } && let Some((SCREEN, id, _)) = kit.focused_control(window)
            && let Some(changes) = self.step_slider(id, step)
        {
            return KeyOutcome::Store(changes);
        }
        KeyOutcome::Ignored
    }

    /// A key in the field of row `row`.
    fn field_key(&mut self, row: usize, chord: Chord, typed: Option<&str>) -> KeyOutcome {
        if chord.key == Key::Enter && !chord.mods.ctrl && !chord.mods.alt {
            return self.commit().map_or(KeyOutcome::Taken, KeyOutcome::Store);
        }
        self.begin_edit(row);
        let Some(edit) = self.edit.as_mut() else {
            return KeyOutcome::Ignored;
        };
        if chord.mods.ctrl && !chord.mods.alt && chord.key == Key::Char('a') {
            edit.select_all = true;
            return KeyOutcome::Taken;
        }
        if chord.key == Key::Backspace {
            if edit.select_all {
                edit.text.clear();
                edit.select_all = false;
            } else {
                edit.text.pop();
            }
            return KeyOutcome::Taken;
        }
        match typed.filter(|_| !chord.mods.ctrl && !chord.mods.alt) {
            Some(typed) if self.insert(row, typed) => KeyOutcome::Taken,
            _ => KeyOutcome::Ignored,
        }
    }

    /// Text typed with a key the key table has no chord for (Space, and letters of other layouts), into the field
    /// that has the keyboard. True when a field took it.
    pub fn typed(&mut self, typed: &str, window: &Window) -> bool {
        match self
            .fields
            .iter()
            .position(|field| field.is_focused(window))
        {
            Some(row) => self.insert(row, typed),
            None => false,
        }
    }

    /// Adds typed text to the edit of row `row`, replacing it all after Ctrl+A. Control characters are not text.
    fn insert(&mut self, row: usize, typed: &str) -> bool {
        if typed.is_empty() || typed.chars().any(char::is_control) {
            return false;
        }
        self.begin_edit(row);
        let Some(edit) = self.edit.as_mut() else {
            return false;
        };
        if edit.select_all {
            edit.text.clear();
            edit.select_all = false;
        }
        edit.text.push_str(typed);
        true
    }

    /// The names of the settings the file sets itself (an unreadable or broken file sets none), so a theme the
    /// file leaves out takes the window's own default, as at startup.
    pub fn keys_set(&self) -> Vec<String> {
        let Ok(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let Ok(text) = std::fs::read_to_string(store.path()) else {
            return Vec::new();
        };
        let document = if clanky_state::config::is_json_path(store.path()) {
            serde_json::from_str::<Value>(&text).ok()
        } else {
            clanky_state::config_xml::load_soft(&text)
                .ok()
                .map(|(_, document, _)| document)
        };
        document
            .and_then(|document| {
                document
                    .as_object()
                    .map(|object| object.keys().cloned().collect())
            })
            .unwrap_or_default()
    }

    /// An arrow on a focused slider: its value one step along.
    fn step_slider(&self, id: u32, direction: f64) -> Option<Changes> {
        let rows = self.rows();
        let row = rows.get(id.checked_sub(FIRST_ROW_ID)? as usize)?;
        let RowControl::Slider {
            value,
            min,
            max,
            step,
        } = row.control
        else {
            return None;
        };
        let value = on_step((value + direction * step).clamp(min, max), step);
        Some(row.changes(json!(value)))
    }

    /// Scrolls the body so the control with the keyboard is wholly in view, from the boxes of the last frame.
    fn scroll_to_focus(&mut self, kit: &Kit, window: &Window) {
        let (Some((SCREEN, id, control)), Some(body)) =
            (kit.focused_control(window), self.body.get())
        else {
            return;
        };
        if id < FIRST_ROW_ID {
            return;
        }
        let top = f32::from(control.origin.y);
        let bottom = top + f32::from(control.size.height);
        let body_top = f32::from(body.origin.y);
        let body_bottom = body_top + f32::from(body.size.height);
        let delta = if top < body_top {
            top - body_top
        } else if bottom > body_bottom {
            bottom - body_bottom
        } else {
            return;
        };
        self.scroll = self.clamped(self.scroll + delta, window.scale_factor());
    }

    /// The Settings window over the main window while it is open; None while it is closed. The record's
    /// `settings` line and the error banner's landmark are written either way.
    pub fn render(
        &mut self,
        kit: &Kit,
        colors: &Colors,
        host: &Host,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let record = kit.record().clone();
        record.landmark_absent("error-banner", 0);
        record.set_landmark_text("error-banner", 0, String::new());
        for name in ["settings-panel", "settings-header", "settings-tab-pill"] {
            record.landmark_absent(name, 0);
        }
        let (cards, labels) = std::mem::take(&mut self.drawn);
        for index in 0..cards {
            for name in [
                "settings-card",
                "settings-card-title",
                "settings-card-description",
            ] {
                record.landmark_absent(name, index);
            }
        }
        for index in 0..labels {
            record.landmark_absent("settings-group-label", index);
        }
        if !self.open {
            return None;
        }
        kit.trap_in_build_order(SCREEN);
        let scale = window.scale_factor();
        self.scroll = self.clamped(self.scroll, scale);
        let mut fields = serde_json::Map::new();
        fields.insert("page".into(), json!(self.page.label()));
        fields.insert(
            "scroll".into(),
            json!({
                "top": (self.scroll * scale).round() as i64,
                "max": (self.max_scroll() * scale).round() as i64,
            }),
        );
        record.set_screen_fields(SCREEN, fields);
        let hair = t::hairline(scale);
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let panel_height = (height - 2.0 * t::SETTINGS_PANEL_MARGIN)
            .clamp(t::SETTINGS_PANEL_MIN_HEIGHT, t::SETTINGS_PANEL_MAX_HEIGHT);
        let whole = |value: f32| (value * scale).round() / scale;
        let left = whole((width - t::SETTINGS_PANEL_WIDTH) / 2.0);
        let top = whole((height - panel_height) / 2.0);
        let header = self.header(kit, colors, host, hair, window, cx);
        let body = self.body(kit, colors, host, hair, window, cx);
        let banner = self.banner().map(|text| {
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .h(px(t::SETTINGS_BANNER_HEIGHT))
                .px(px(t::SETTINGS_PADDING_SIDE))
                .border_t(px(hair))
                .border_color(paint(colors.border, 1.0))
                .text_size(px(t::TEXT_UI))
                .text_color(paint(colors.err, 1.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(text.clone())
                .child(record.landmark_with_text("error-banner", 0, text))
        });
        let shadow = t::SHADOW_POPOVER;
        let panel = div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .track_focus(&self.focus)
            .occlude()
            .rounded(px(t::SETTINGS_PANEL_RADIUS))
            .border(px(hair))
            .border_color(paint(colors.border, 1.0))
            .bg(paint(colors.background, 1.0))
            .shadow(vec![gpui::BoxShadow {
                color: paint(shadow.color(), 1.0).into(),
                offset: point(px(0.0), px(shadow.offset_y)),
                blur_radius: px(shadow.sigma()),
                spread_radius: px(0.0),
                inset: false,
            }])
            .font_family(INTERFACE_FONT_FAMILY)
            .text_color(paint(colors.foreground, 1.0))
            .child(header)
            .child(body)
            .children(banner);
        // The measuring child is the wrapper's, which has no edge of its own, so it measures the panel's outer box.
        Some(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(t::SETTINGS_PANEL_WIDTH))
                .h(px(panel_height))
                .child(panel)
                .child(record.landmark("settings-panel", 0))
                .into_any_element(),
        )
    }

    /// The header: the section switcher in its pill, at the column's left edge, over the divider.
    fn header(
        &self,
        kit: &Kit,
        colors: &Colors,
        host: &Host,
        hair: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let mut pill = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(t::SPACE_2))
            .h(px(t::SETTINGS_TAB_PILL_HEIGHT))
            .p(px(t::SPACE_2))
            .rounded(px(t::SETTINGS_PILL_RADIUS))
            .bg(paint(colors.card, 1.0));
        for (index, page) in Page::ALL.into_iter().enumerate() {
            let host = Rc::clone(host);
            let segment = Control::new(Kind::Segment, FIRST_PAGE_ID + index as u32, page.label())
                .checked(page == self.page)
                .group(PAGES_GROUP)
                .on_click(move |window, cx| host(Request::Page(page), window, cx));
            pill = pill.child(kit.build(SCREEN, segment, window, cx));
        }
        let pill = pill.child(kit.record().landmark("settings-tab-pill", 0));
        let face = div()
            .flex()
            .items_center()
            .h(px(t::SETTINGS_HEADER_HEIGHT))
            .px(px(t::SETTINGS_PADDING_SIDE))
            .border_b(px(hair))
            .border_color(paint(colors.border, 1.0))
            .bg(paint(colors.background, 1.0))
            .child(pill);
        // Measured from a wrapper with no edge, so the box holds the bottom divider too.
        div()
            .relative()
            .flex_none()
            .child(face)
            .child(kit.record().landmark("settings-header", 0))
            .into_any_element()
    }

    /// The body: the page's cards in one column, scrolled by the wheel.
    fn body(
        &mut self,
        kit: &Kit,
        colors: &Colors,
        host: &Host,
        hair: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let rows = self.rows();
        while self.fields.len() < rows.len() {
            self.fields.push(cx.focus_handle());
        }
        let mut column = div()
            .relative()
            .top(px(-self.scroll))
            .flex()
            .flex_col()
            .w_full()
            .px(px(t::SETTINGS_PADDING_SIDE))
            .pt(px(t::SETTINGS_PADDING_TOP))
            .pb(px(t::SETTINGS_PADDING_BOTTOM));
        let mut labels = 0;
        for (index, row) in rows.iter().enumerate() {
            if let Some(group) = row.group {
                let gap = if index == 0 {
                    0.0
                } else {
                    t::SETTINGS_GROUP_GAP
                };
                column = column.child(
                    div()
                        .relative()
                        .mt(px(gap))
                        .line_height(px(t::TEXT_UI * t::INTER_LINE_SPAN))
                        .text_size(px(t::TEXT_UI))
                        .font_weight(FontWeight(f32::from(t::WEIGHT_MEDIUM)))
                        .text_color(paint(colors.muted_foreground, 1.0))
                        .child(group)
                        .child(kit.record().landmark("settings-group-label", labels)),
                );
                labels += 1;
            }
            let gap = if index == 0 && row.group.is_none() {
                0.0
            } else {
                t::SETTINGS_CARD_GAP
            };
            let card = self.card(index, row, kit, colors, host, hair, window, cx);
            column = column.child(div().mt(px(gap)).child(card));
        }
        self.drawn = (rows.len() as u32, labels);
        let height = Rc::clone(&self.content_height);
        let column = column.child(
            canvas(
                move |bounds, _, _| height.set(f32::from(bounds.size.height)),
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let body_box = Rc::clone(&self.body);
        let wheel = Rc::clone(host);
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .on_scroll_wheel(move |event: &ScrollWheelEvent, window, cx| {
                let delta = event.delta.pixel_delta(px(t::SETTINGS_SCROLL_LINE));
                wheel(Request::Scroll(-f32::from(delta.y)), window, cx);
            })
            .child(column)
            .child(
                canvas(
                    move |bounds, _, _| body_box.set(Some(bounds)),
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(kit.record().screen_body(SCREEN))
            .into_any_element()
    }

    /// One setting's card: its title and description, and its control at the right end.
    #[allow(clippy::too_many_arguments)]
    fn card(
        &self,
        index: usize,
        row: &Row,
        kit: &Kit,
        colors: &Colors,
        host: &Host,
        hair: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let record = kit.record();
        let card = index as u32;
        let words = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .relative()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .line_height(px(t::TEXT_TITLE * t::INTER_LINE_SPAN))
                    .text_size(px(t::TEXT_TITLE))
                    .font_weight(FontWeight(f32::from(t::WEIGHT_MEDIUM)))
                    .text_color(paint(colors.foreground, 1.0))
                    .child(row.label.clone())
                    .child(record.landmark("settings-card-title", card)),
            )
            .child(
                div()
                    .relative()
                    .mt(px(t::SETTINGS_TITLE_GAP))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .line_height(px(t::TEXT_DESCRIPTION * t::INTER_LINE_SPAN))
                    .text_size(px(t::TEXT_DESCRIPTION))
                    .font_weight(FontWeight(f32::from(t::WEIGHT_REGULAR)))
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(row.description.clone())
                    .child(record.landmark("settings-card-description", card)),
            );
        let face = div()
            .flex()
            .items_center()
            .gap(px(t::SETTINGS_CONTROL_GAP))
            .w_full()
            .px(px(t::SETTINGS_CARD_PADDING_X))
            .py(px(t::SETTINGS_CARD_PADDING_Y))
            .rounded(px(t::SETTINGS_CARD_RADIUS))
            .border(px(hair))
            .border_color(paint(colors.border, t::SETTINGS_CARD_OPACITY))
            .bg(paint(colors.card, t::SETTINGS_CARD_OPACITY))
            .child(words)
            .child(self.control(index, row, kit, colors, host, window, cx));
        // The measuring child is the wrapper's, which has no edge of its own, so it measures the card's outer box
        // (an absolute child of the card itself would sit inside its border).
        div()
            .relative()
            .w_full()
            .child(face)
            .child(record.landmark("settings-card", card))
            .into_any_element()
    }

    /// A row's control, built with the kit.
    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        index: usize,
        row: &Row,
        kit: &Kit,
        colors: &Colors,
        host: &Host,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = FIRST_ROW_ID + index as u32;
        let label = row.label.clone();
        match &row.control {
            RowControl::Switch { on } => {
                let host = Rc::clone(host);
                let changes = row.changes(json!(!on));
                let switch = Control::new(Kind::Switch, id, label)
                    .checked(*on)
                    .scrolls()
                    .on_click(move |window, cx| host(Request::Store(changes.clone()), window, cx));
                kit.build(SCREEN, switch, window, cx)
            }
            RowControl::Segments { options, chosen } => {
                let mut tray = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(t::SPACE_2))
                    .h(px(t::SETTINGS_TAB_PILL_HEIGHT))
                    .p(px(t::SPACE_2))
                    .rounded(px(t::SETTINGS_PILL_RADIUS))
                    .bg(paint(colors.card, 1.0));
                let group = Group::horizontal(row_group(row.key));
                for (place, (text, value)) in options.iter().enumerate() {
                    let host = Rc::clone(host);
                    let changes = row.changes(value.clone());
                    let option_id = FIRST_OPTION_ID + OPTIONS_PER_ROW * index as u32 + place as u32;
                    let segment = Control::new(Kind::Segment, option_id, text.clone())
                        .checked(*chosen == Some(place))
                        .group(group)
                        .scrolls()
                        .on_click(move |window, cx| {
                            host(Request::Store(changes.clone()), window, cx);
                        });
                    tray = tray.child(kit.build(SCREEN, segment, window, cx));
                }
                tray.into_any_element()
            }
            RowControl::Field { text, .. } => {
                let focus = self.fields[index].clone();
                let editing = self.edit.as_ref().filter(|edit| edit.row == index);
                let shown = editing.map_or(text.as_str(), |edit| edit.text.as_str());
                let selected = editing.is_some_and(|edit| edit.select_all);
                let typing = focus.is_focused(window);
                let inside = field_text(shown, selected, typing, colors, window);
                let host = Rc::clone(host);
                let field = Control::new(Kind::Field, id, label)
                    .focus(focus)
                    .width(t::SETTINGS_FIELD_WIDTH)
                    .field_text(shown)
                    .content(inside)
                    .scrolls()
                    .on_click(move |window, cx| host(Request::Edit(index), window, cx));
                kit.build(SCREEN, field, window, cx)
            }
            RowControl::Slider {
                value, min, max, ..
            } => {
                let host = Rc::clone(host);
                let slider = Control::new(Kind::Slider, id, label)
                    .slider(*value, *min, *max)
                    .scrolls()
                    .on_click(move |window, cx| host(Request::Slide(index), window, cx));
                kit.build(SCREEN, slider, window, cx)
            }
            RowControl::Keys(keys) => div()
                .flex_none()
                .font_family(t::TERMINAL_FONT_FAMILY)
                .text_size(px(t::TEXT_UI))
                .text_color(paint(colors.muted_foreground, 1.0))
                .child(keys.clone())
                .into_any_element(),
        }
    }
}

/// A field's inside: its text (all of it on the selection colour after Ctrl+A) and the caret while it is typed in.
fn field_text(
    shown: &str,
    selected: bool,
    typing: bool,
    colors: &Colors,
    window: &Window,
) -> AnyElement {
    let mut words = div()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_color(paint(colors.foreground, 1.0))
        .child(shown.to_owned());
    if selected {
        words = words.bg(paint(colors.accent, 1.0));
    }
    let caret = typing.then(|| {
        div()
            .flex_none()
            .w(px(t::hairline(window.scale_factor())))
            .h(px(t::CARET_HEIGHT))
            .bg(paint(colors.foreground, 1.0))
    });
    div()
        .flex()
        .items_center()
        .w_full()
        .text_size(px(t::TEXT_UI))
        .child(words)
        .children(caret)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_numbers_become_the_values_the_config_stores() {
        assert_eq!(whole_number(" 45 "), Ok(json!(45)));
        assert!(whole_number("4.5").is_err());
        assert!(whole_number("-1").is_err());
        assert_eq!(number("15"), Ok(json!(15)));
        assert_eq!(number("1.1"), Ok(json!(1.1)));
        assert!(number("wide").is_err());
        assert_eq!(text("en_GB"), Ok(json!("en_GB")));
    }

    #[test]
    fn numbers_show_as_short_as_they_read() {
        assert_eq!(number_text(14.67), "14.67");
        assert_eq!(number_text(1.0), "1");
        assert_eq!(number_text(1.25), "1.25");
    }

    #[test]
    fn a_slider_value_lands_on_its_step() {
        // 0.5 + 0.5975 x 0.5 = 0.79874, the gate's click at scale 1, is 0.8 on the 0.01 step.
        assert_eq!(on_step(0.798_742, 0.01), 0.8);
        assert_eq!(on_step(0.8 + 0.01, 0.01), 0.81);
    }

    #[test]
    fn a_segmented_row_checks_the_option_that_stores_the_value_now() {
        let options = [("Top", json!("horizontal")), ("Side", json!("vertical"))];
        let row = Row::segments("tab_style", "Tabs", "", &options, &json!("vertical"));
        let RowControl::Segments { chosen, .. } = row.control else {
            panic!("a segmented row");
        };
        assert_eq!(chosen, Some(1));
        assert_eq!(row_group("tab_style"), "row:tab_style");
        assert_eq!(
            row.changes(json!("horizontal")),
            vec![("tab_style", Some(json!("horizontal")))]
        );
    }
}
