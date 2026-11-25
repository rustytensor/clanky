//! The main window: the title bar with the tabs, the active tab's panes in their split tree, and the status line.
//!
//! The session server owns the workspace (its tabs, each a split tree of panes with a focused pane, and the active
//! tab); this window shows it and changes it through the server, then reads it back. Every pane the workspace names
//! has a `TerminalPane` here, whether its tab is showing or not, so switching tabs shows the pane as it already is.

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use clanky::config::{Config, TabGroups, TabStyle, ThemePreference};
use clanky_api::Client;
use clanky_core::{
    Workspace,
    layout::{Axis, Layout, Rect},
};
use clanky_state::{
    cursor_blink::Blink,
    keys::{Action, Chord, Key, Mods, action_for},
    themes::{Colors, Theme},
};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent,
    Keystroke, MouseButton, MouseDownEvent, ParentElement, Render, Styled, Subscription, Task,
    Window, canvas, div, px,
};
use serde_json::{Value, json};

mod agent_picker;
mod command_bar;
mod command_palette;
mod completion_list;
mod context_menu;
mod drag;
mod editors;
mod find_bar;
mod floating;
mod settings_window;
mod side_tabs;
#[cfg(feature = "test-popover")]
mod test_popover;
mod tools;

use crate::{
    control_look::paint,
    controls::{Control, INTERFACE_FONT_FAMILY, Kind, Kit},
    grid_element::{PANE_PADDING, TextSettings},
    icons::Icon,
    link::Link,
    palette::Palette,
    pane::{PaneEvent, PaneSetup, SharedInput, SharedTrace, TerminalPane},
    popover::Popover,
    request_log,
    settings::{self, SettingsWindow},
    tokens as t,
};
use agent_picker::AgentTools;
use command_bar::{Chips, read_chips};
use drag::{Drag, Places};
use editors::Editors;
use side_tabs::{PaneLook, SideTabs, escape, read_pane_look};
use tools::{Tools, ToolsData, ToolsQuery, read_tools};

/// The main window's name in the control record.
const SCREEN: &str = "main-window";
/// How often the window asks the server for what the title bar and status line show.
const STATUS_INTERVAL: Duration = Duration::from_secs(1);
/// A cell's size before any pane has been laid out, for the first guess at a new pane's grid.
const FALLBACK_CELL: (f32, f32) = (9.6, 19.2);
/// Control ids: the new-tab button, then two per tab (the tab, its close button).
const NEW_TAB_ID: u32 = 1;
const TOOLS_TOGGLE_ID: u32 = 2;
const FIRST_TAB_ID: u32 = 100;

/// What the window shows of the server beyond the workspace, read once a second; a change draws a frame.
#[derive(Clone, Debug, Default, PartialEq)]
struct Status {
    /// The workspace revision the server reported; a new one means another window changed the workspace.
    revision: u64,
    /// Each tab's title: the folder of its focused pane, by pane id.
    folders: BTreeMap<String, String>,
    /// The server's uptime as the status line words it, which changes once a minute.
    uptime: String,
    masking_ready: bool,
    agents: usize,
    /// What the tools panel shows.
    tools: ToolsData,
    /// Each pane's command input chips.
    chips: BTreeMap<String, Chips>,
    /// What the side tabs and the top tabs' dots show of each tab's focused pane, by pane id.
    side: BTreeMap<String, PaneLook>,
}

pub struct WindowSetup {
    pub client: Client,
    pub palette: Palette,
    pub text: TextSettings,
    pub trace: SharedTrace,
    /// The cursor's blink, for every pane.
    pub blink: Blink,
    pub kit: Kit,
    pub colors: Colors,
    pub post_ai: bool,
    /// The tab started with --exec: it stays open after its command ends.
    pub exec_tab: Option<String>,
    /// The tools panel open at start, the tools the config lists, and its agent tools (for Recent's labels).
    pub tools_open: bool,
    pub tools: Vec<clanky::config::Tool>,
    pub agent_programs: Vec<clanky::config::AgentTool>,
    /// Whether a pane started for an agent gets agent access (read + run in that pane only).
    pub agent_pane_access: bool,
    /// Each terminal pane shows a command input (the config's editor mode); off, panes show the shell's prompt.
    pub command_input: bool,
    /// Panes the window's first tab opens with: 2 splits it down once the window has drawn (a new window, the
    /// config's `new_window_panes`); 1 leaves the tab as it is.
    pub opens_with: u8,
    /// With restore_layout on: the saved layout's file and the session socket, whose startup lock guards each save.
    pub layout_file: Option<(PathBuf, PathBuf)>,
    /// The editor panes' token colours, from the interface theme.
    pub syntax: clanky::editor::SyntaxColors,
    /// Tabs across the title bar or down the side column, and how the side column groups them.
    pub tab_style: TabStyle,
    pub tab_groups: TabGroups,
    /// The settings file the window was started with, where the palette's tab switch saves `tab_style`.
    pub config_file: Option<PathBuf>,
}

pub struct MainWindow {
    client: Client,
    palette: Palette,
    text: TextSettings,
    trace: SharedTrace,
    blink: Blink,
    kit: Kit,
    colors: Colors,
    post_ai: bool,
    exec_tab: Option<String>,
    layout_file: Option<(PathBuf, PathBuf)>,
    workspace: Workspace,
    panes: HashMap<String, (Entity<TerminalPane>, Subscription)>,
    status: Status,
    /// The last action's failure, shown in the status line until the next action succeeds.
    notice: Option<String>,
    /// When the frame now being drawn started, while a frame trace is on.
    draw_started: Option<Instant>,
    drag: Option<Drag>,
    /// The pane the keyboard came from with Ctrl+F6; Escape in the controls returns it there.
    controls_from: Option<String>,
    places: Places,
    tools: Tools,
    /// What the panes' command inputs share; None with the classic input.
    inputs: Option<SharedInput>,
    /// The floating screen open above the panes, if any.
    popover: Popover,
    /// The pane's find bar while it is open.
    find: Option<find_bar::FindBar>,
    palette_state: command_palette::PaletteState,
    pane_menu: context_menu::PaneMenu,
    /// The agent tools the picker offers, and the one being started.
    agent_tools: AgentTools,
    /// The config's agent tools, which name a pane's program as an agent in the side tabs.
    agent_programs: Vec<clanky::config::AgentTool>,
    editors: Editors,
    tab_style: TabStyle,
    tab_groups: TabGroups,
    config_file: Option<PathBuf>,
    side: SideTabs,
    /// The Settings window over this one, open or closed, with the settings file's store.
    settings: SettingsWindow,
    _status: Task<()>,
    /// Looks at the settings file for changes made outside the window.
    _settings_poll: Task<()>,
    /// Draws a frame when the window gains or loses focus, which starts or stops the badges' animation.
    _activation: Subscription,
}

impl MainWindow {
    pub fn new(setup: WindowSetup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let status = cx.spawn_in(window, async move |this, cx| {
            // The first look runs at once, so the status line, chips and tools are filled in with the window's
            // startup frames rather than a second later, when the window would otherwise be idle.
            loop {
                let Ok((client, ids, query, inputs)) = this.update_in(cx, |this, window, cx| {
                    let focused = this.focused_pane(window, cx);
                    let inputs: Vec<String> = if this.inputs.is_some() {
                        this.panes.keys().cloned().collect()
                    } else {
                        Vec::new()
                    };
                    (
                        this.client.clone(),
                        this.tab_focuses(),
                        this.tools_query(focused),
                        inputs,
                    )
                }) else {
                    break;
                };
                let status = cx
                    .background_executor()
                    .spawn(async move { read_status(&client, &ids, &inputs, &query) })
                    .await;
                let applied = this.update_in(cx, |this, window, cx| {
                    let moved = status.revision != this.workspace.revision;
                    if status != this.status {
                        this.status = status;
                        cx.notify();
                    }
                    // New output in a tab not on screen draws the frame that marks it unread.
                    if this.unread_tabs(cx) != this.side.unread_shown {
                        cx.notify();
                    }
                    if moved {
                        this.after_change(window, cx);
                    }
                });
                if applied.is_err() {
                    break;
                }
                cx.background_executor().timer(STATUS_INTERVAL).await;
            }
        });
        let settings_poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(t::SETTINGS_POLL_MS))
                    .await;
                if this
                    .update_in(cx, |this, _, cx| this.settings_poll(cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        setup.kit.record().declare_screen(settings::SCREEN);
        let settings = SettingsWindow::new(setup.config_file.clone(), cx);
        let mut this = Self {
            client: setup.client,
            palette: setup.palette,
            text: setup.text,
            trace: setup.trace,
            blink: setup.blink,
            kit: setup.kit,
            colors: setup.colors,
            post_ai: setup.post_ai,
            exec_tab: setup.exec_tab,
            layout_file: setup.layout_file,
            workspace: Workspace {
                revision: 0,
                groups: Vec::new(),
                active: 0,
            },
            panes: HashMap::new(),
            status: Status::default(),
            notice: None,
            draw_started: None,
            drag: None,
            controls_from: None,
            places: Places::default(),
            agent_tools: AgentTools::new(setup.agent_programs.clone(), setup.agent_pane_access),
            agent_programs: setup.agent_programs.clone(),
            tools: Tools::new(setup.tools_open, setup.tools, setup.agent_programs),
            inputs: setup.command_input.then(SharedInput::new),
            popover: Popover::default(),
            find: None,
            palette_state: command_palette::PaletteState::default(),
            pane_menu: context_menu::PaneMenu::default(),
            editors: Editors::new(setup.syntax),
            tab_style: setup.tab_style,
            tab_groups: setup.tab_groups,
            config_file: setup.config_file,
            side: SideTabs::new(cx.focus_handle()),
            settings,
            _status: status,
            _settings_poll: settings_poll,
            _activation: cx.observe_window_activation(window, |_, _, cx| cx.notify()),
        };
        this.after_change(window, cx);
        if setup.opens_with >= 2 {
            // After the first frame, so the new pane is sized from the measured one.
            cx.on_next_frame(window, |this, window, cx| {
                this.act(window, cx, |this, window, cx| {
                    this.split(Axis::Rows, window, cx)
                });
            });
        }
        this
    }

    /// Each tab's focused pane, in tab order.
    fn tab_focuses(&self) -> Vec<String> {
        self.workspace
            .groups
            .iter()
            .map(|group| group.focus.clone())
            .collect()
    }

    /// Reads the workspace back from the server, starts a pane for each new id and drops the panes that are gone.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let workspace = self
            .client
            .workspace_get()
            .map_err(|e| format!("reading the workspace: {}", e.message))?;
        let leaves: Vec<String> = workspace
            .groups
            .iter()
            .flat_map(|group| group.layout.leaves())
            .collect();
        self.panes.retain(|id, _| leaves.contains(id));
        for id in leaves {
            if !self.panes.contains_key(&id) {
                let pane = self.start_pane(id.clone(), window, cx)?;
                self.panes.insert(id, pane);
            }
        }
        self.workspace = workspace;
        Ok(())
    }

    fn start_pane(
        &self,
        tab: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(Entity<TerminalPane>, Subscription), String> {
        let watch_exit = self.exec_tab.as_deref() != Some(tab.as_str());
        let (link, updates) = Link::start(&self.client, tab.clone(), watch_exit)?;
        let setup = PaneSetup {
            client: self.client.clone(),
            tab: tab.clone(),
            palette: self.palette.clone(),
            colors: self.colors.clone(),
            record: self.kit.record().clone(),
            input: self.inputs.clone(),
            text: self.text.clone(),
            size_sent: (0, 0),
            link,
            updates,
            trace: Rc::clone(&self.trace),
            blink: self.blink,
        };
        let pane = cx.new(|cx| TerminalPane::new(setup, window, cx));
        let subscription =
            cx.subscribe_in(
                &pane,
                window,
                move |this, _, event, window, cx| match event {
                    PaneEvent::Finished => this.pane_finished(&tab, window, cx),
                    PaneEvent::Menu(at) => this.open_pane_menu(&tab, *at, window, cx),
                },
            );
        Ok((pane, subscription))
    }

    /// After anything that changed the workspace: read it back, give the keyboard to the active tab's focused pane,
    /// and close the window when no pane is left.
    fn after_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(message) = self.sync(window, cx) {
            self.notice = Some(message);
        }
        self.rehome_editors();
        if let Err(message) = self.save_layout() {
            self.notice = Some(message);
        }
        if self.workspace.groups.is_empty() && self.notice.is_none() {
            cx.quit();
            return;
        }
        // A tab whose focused pane is new is titled now, not at the next look, so its title does not jump.
        for id in self.tab_focuses() {
            if !self.status.folders.contains_key(&id)
                && let Some(folder) = folder_of(&self.client, &id)
            {
                self.status.folders.insert(id, folder);
            }
        }
        // While Settings is open it keeps the keyboard; it gives it back to the pane when it closes.
        if self.settings.is_open() {
            cx.notify();
            return;
        }
        if let Some(focus) = self
            .workspace
            .groups
            .get(self.workspace.active)
            .and_then(|group| self.panes.get(&group.focus))
            .map(|pane| pane.0.read(cx).focus_handle().clone())
        {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Runs an action, keeping its failure for the status line.
    fn act(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> Result<(), String>,
    ) {
        self.notice = action(self, window, cx).err();
        self.after_change(window, cx);
    }

    /// The pane that has the keyboard, else the active tab's focused pane.
    fn focused_pane(&self, window: &Window, cx: &App) -> Option<String> {
        let group = self.workspace.groups.get(self.workspace.active)?;
        group
            .layout
            .leaves()
            .into_iter()
            .find(|id| {
                self.panes
                    .get(id)
                    .is_some_and(|pane| pane.0.read(cx).focus_handle().is_focused(window))
            })
            .or_else(|| Some(group.focus.clone()))
    }

    /// Keys while a control has the keyboard: Tab and Shift+Tab walk the controls in reading order and wrap;
    /// Escape gives the keyboard back to the pane it came from. True when the key was one of these.
    fn controls_key(&mut self, chord: Chord, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if chord.mods.ctrl || chord.mods.alt || !self.kit.has_focus(window) {
            return false;
        }
        match chord.key {
            Key::Tab => self.kit.focus_step(!chord.mods.shift, window, cx),
            Key::Escape => {
                let pane = self
                    .controls_from
                    .take()
                    .or_else(|| self.focused_pane(window, cx))
                    .and_then(|id| self.panes.get(&id))
                    .map(|pane| pane.0.read(cx).focus_handle().clone());
                if let Some(handle) = pane {
                    window.focus(&handle, cx);
                }
                true
            }
            _ => false,
        }
    }

    fn pane_size(&self, id: Option<&str>, cx: &App) -> (usize, usize) {
        id.and_then(|id| self.panes.get(id))
            .map(|pane| pane.0.read(cx).size_sent())
            .filter(|&(rows, cols)| rows > 0 && cols > 0)
            .unwrap_or((24, 80))
    }

    /// With restore_layout on, keeps the saved layout up to date with the workspace (its tabs, splits and each
    /// pane's folder), as today's window does after every change, so the next fresh session opens it again.
    fn save_layout(&self) -> Result<(), String> {
        let Some((file, socket)) = &self.layout_file else {
            return Ok(());
        };
        let _guard = clanky_core::platform::StartupLock::acquire(socket)
            .map_err(|e| format!("locking the session to save the layout: {e}"))?;
        clanky::layout_store::save(&self.client, file)
            .map_err(|e| format!("saving the layout to {}: {e}", file.display()))
    }

    /// Writes the workspace to the server, which answers with its new revision.
    fn save(&mut self) -> Result<(), String> {
        self.workspace = self
            .client
            .workspace_set(&self.workspace)
            .map_err(|e| format!("saving the workspace: {}", e.message))?;
        Ok(())
    }

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let source = self.focused_pane(window, cx);
        let (rows, cols) = self.pane_size(source.as_deref(), cx);
        let id = self.spawn_terminal(source.as_deref(), rows, cols)?;
        self.sync(window, cx)?;
        self.workspace.active = self
            .workspace
            .groups
            .iter()
            .position(|group| group.layout.contains(&id))
            .ok_or("the new tab is missing from the workspace")?;
        self.save()
    }

    /// Splits the focused pane, the new pane to the right (`Axis::Columns`) or below (`Axis::Rows`), starting in
    /// the focused pane's folder. The server opens every new terminal as a tab of its own, so the new pane is then
    /// moved into the split; if the workspace changed meanwhile, it stays a tab, as in today's window.
    fn split(
        &mut self,
        axis: Axis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let target = self
            .focused_pane(window, cx)
            .ok_or("there is no pane to split")?;
        let area = self.grid_area(&target, window, cx);
        let group = self
            .workspace
            .groups
            .iter()
            .find(|group| group.layout.contains(&target))
            .ok_or("the pane to split has closed")?;
        // Refuse a split that cannot leave both panes their minimum before starting a terminal for it.
        group
            .layout
            .clone()
            .split(&target, "\u{0}new".into(), axis, area.clone())?;
        let (rows, cols) = self.pane_size(Some(&target), cx);
        let (rows, cols) = match axis {
            Axis::Columns => (rows, (cols / 2).max(1)),
            Axis::Rows => ((rows / 2).max(1), cols),
        };
        let id = self.spawn_terminal(Some(&target), rows, cols)?;
        self.sync(window, cx)?;
        self.workspace
            .groups
            .retain(|group| group.layout.leaves() != [id.clone()]);
        let index = self
            .workspace
            .groups
            .iter()
            .position(|group| group.layout.contains(&target))
            .ok_or("the pane to split has closed; the new terminal is a tab of its own")?;
        let group = &mut self.workspace.groups[index];
        group.layout.split(&target, id.clone(), axis, area)?;
        group.focus = id;
        self.workspace.active = index;
        self.save()
    }

    /// The panes' area in cells, for the split rules' minimum sizes.
    fn grid_area(&self, sample: &str, window: &Window, cx: &App) -> Rect {
        let (cell_width, line_height) = self
            .panes
            .get(sample)
            .and_then(|pane| pane.0.read(cx).cell_size())
            .unwrap_or(FALLBACK_CELL);
        let viewport = window.viewport_size();
        // The hairlines under the title bar and beside the tools panel and the side tabs take their own device pixel.
        let hairline = t::hairline(window.scale_factor());
        let mut beside = 0.0;
        if self.tools.open {
            beside += t::SIDE_COLUMN_WIDTH + hairline;
        }
        if self.vertical() {
            beside += self.side_width() + hairline;
        }
        let width = f32::from(viewport.width) - beside - 2.0 * PANE_PADDING;
        let height = f32::from(viewport.height)
            - t::TITLE_BAR_HEIGHT
            - hairline
            - t::STATUS_HEIGHT
            - 2.0 * PANE_PADDING;
        Rect {
            col: 0,
            row: 0,
            cols: (width / cell_width).max(0.0) as usize,
            rows: (height / line_height).max(0.0) as usize,
        }
    }

    fn close_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let target = self
            .focused_pane(window, cx)
            .ok_or("there is no pane to close")?;
        self.close_terminals(&[target])
    }

    /// Closes terminals on the server, which takes them out of the workspace.
    fn close_terminals(&mut self, ids: &[String]) -> Result<(), String> {
        for id in ids {
            self.client
                .request("close_tab", &json!({"tab_id": id}), true)
                .map_err(|e| format!("closing the pane: {}", e.message))?;
        }
        Ok(())
    }

    fn select_tab(&mut self, index: usize) -> Result<(), String> {
        if index >= self.workspace.groups.len() || index == self.workspace.active {
            return Ok(());
        }
        self.workspace.active = index;
        self.save()
    }

    fn pane_finished(&mut self, tab: &str, window: &mut Window, cx: &mut Context<Self>) {
        // Closing the last terminal is what lets the session server exit after it, as it does behind today's window.
        let tab = tab.to_owned();
        self.act(window, cx, move |this, _, _| this.close_terminals(&[tab]));
    }

    /// A window-wide shortcut from clanky-state's table, if this window handles it; the rest go to the pane.
    fn shortcut(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Settings' keys first while it is open: it is a window of its own over this one.
        if self.settings_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        // Typing in the side tabs' filter, before the keys the table names (Space has no chord of its own).
        if self.side_filter_key(event, window, cx) {
            cx.stop_propagation();
            return;
        }
        let Some(chord) = chord_of(&event.keystroke) else {
            return;
        };
        if self.floating_key(chord, window, cx) || self.controls_key(chord, window, cx) {
            cx.stop_propagation();
            return;
        }
        let Some(action) = action_for(chord) else {
            return;
        };
        // The window's own commands wait while Settings is open over it, but Ctrl+F6 still reaches its controls;
        // its other keys go to its controls.
        if self.settings.is_open() && !self.popover.is_open() && action != Action::EnterControls {
            return;
        }
        match action {
            Action::EnterControls => {
                let from = self.focused_pane(window, cx);
                if self.kit.focus_first_control(window, cx) {
                    self.controls_from = from;
                }
            }
            Action::NewTab => self.act(window, cx, Self::new_tab),
            Action::NextTab => {
                let next = (self.workspace.active + 1) % self.workspace.groups.len().max(1);
                self.act(window, cx, move |this, _, _| this.select_tab(next));
            }
            Action::SplitRight => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Columns, window, cx)
            }),
            Action::SplitDown => self.act(window, cx, |this, window, cx| {
                this.split(Axis::Rows, window, cx)
            }),
            Action::ClosePane => {
                if !self.close_focused_editor(window, cx) {
                    self.act(window, cx, Self::close_pane);
                }
            }
            Action::ToggleToolsPanel => self.toggle_tools(cx),
            Action::ToggleSettings => self.toggle_settings(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// A tab's title: its focused pane's folder, else its place. A folder's name may hold anything, so it is
    /// escaped before it is drawn or recorded.
    fn tab_title(&self, index: usize) -> String {
        self.workspace
            .groups
            .get(index)
            .and_then(|group| self.status.folders.get(&group.focus))
            .map_or_else(|| format!("Session {}", index + 1), |folder| escape(folder))
    }

    /// Shows a pane: its tab becomes the active one and the pane its focused pane.
    fn show_pane(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .workspace
            .groups
            .iter()
            .position(|group| group.layout.contains(id))
            .ok_or("that pane has closed")?;
        id.clone_into(&mut self.workspace.groups[index].focus);
        self.workspace.active = index;
        self.save()
    }

    fn title_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(t::SPACE_6))
            .h(px(t::TITLE_BAR_HEIGHT))
            .px(px(t::SPACE_8))
            .overflow_hidden();
        let this = cx.entity();
        let toggler = this.clone();
        // With side tabs the first button folds and unfolds their column; the tools panel keeps its shortcut and
        // its palette command.
        let toggle = if self.vertical() {
            Control::new(Kind::IconButton, TOOLS_TOGGLE_ID, "Side tabs")
                .icon(Icon::Sidebar)
                .checked(!self.side.folded)
                .on_click(move |_, cx| toggler.update(cx, Self::toggle_side_fold))
        } else {
            Control::new(Kind::IconButton, TOOLS_TOGGLE_ID, "Tools panel")
                .icon(Icon::Sidebar)
                .checked(self.tools.open)
                .on_click(move |_, cx| toggler.update(cx, Self::toggle_tools))
        };
        bar = bar.child(self.kit.build(SCREEN, toggle, window, cx));
        if self.vertical() {
            for index in 0..self.workspace.groups.len() {
                let record = self.kit.record();
                record.landmark_absent("tab-dot", index as u32);
                let mut fields = serde_json::Map::new();
                fields.insert("state".into(), json!(""));
                record.set_landmark_fields("tab-dot", index as u32, fields);
            }
            return bar
                .child(div().flex_1())
                .child(self.search_box(window, cx))
                .child(self.settings_button(window, cx))
                .child(self.kit.record().landmark("title-bar", 0))
                .into_any_element();
        }
        for (index, group) in self.workspace.groups.iter().enumerate() {
            let label = self.tab_title(index);
            let id = FIRST_TAB_ID + 2 * index as u32;
            let leaves = group.layout.leaves();
            let closer = this.clone();
            // Inside the tab: its press is its own, and an inactive tab keeps the close's room but Tab skips it.
            let close = Control::new(Kind::IconButtonInline, id + 1, format!("Close {label}"))
                .icon(Icon::Close)
                .inside(index == self.workspace.active)
                .on_click(move |window, cx| {
                    // The tab under the button must not take this click as its own.
                    cx.stop_propagation();
                    let leaves = leaves.clone();
                    closer.update(cx, |this, cx| {
                        this.act(window, cx, move |this, _, _| this.close_terminals(&leaves));
                    });
                });
            let selecter = this.clone();
            let mut tab = Control::new(Kind::Tab, id, label)
                .checked(index == self.workspace.active)
                .trailing(self.kit.build(SCREEN, close, window, cx));
            if let Some(dot) = self.tab_dot(index, cx) {
                tab = tab.leading(dot);
            }
            let tab = tab.on_click(move |window, cx| {
                selecter.update(cx, |this, cx| {
                    this.act(window, cx, move |this, _, _| this.select_tab(index));
                });
            });
            // The tab inside a box of its own that starts a drag and measures where the tab is.
            bar = bar.child(
                div()
                    .relative()
                    .flex_none()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.press_tab(index, event.position, cx);
                        }),
                    )
                    .child(self.kit.build(SCREEN, tab, window, cx))
                    .child(self.places.tab(index)),
            );
        }
        let adder = this.clone();
        let new_tab = Control::new(Kind::IconButton, NEW_TAB_ID, "New tab")
            .icon(Icon::Plus)
            .on_click(move |window, cx| {
                adder.update(cx, |this, cx| this.act(window, cx, Self::new_tab));
            });
        bar.child(self.kit.build(SCREEN, new_tab, window, cx))
            .child(div().flex_1())
            .child(self.search_box(window, cx))
            .child(self.settings_button(window, cx))
            .child(self.kit.record().landmark("title-bar", 0))
            .into_any_element()
    }

    /// The active tab's split tree: panes and nested splits sharing their parent's room by the split's ratio, a
    /// hairline divider between them.
    fn panes_area(&self, scale: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let area = div().flex().flex_1().min_w_0().min_h_0();
        let Some(group) = self.workspace.groups.get(self.workspace.active) else {
            return area.into_any_element();
        };
        let mut index = 0;
        area.child(self.layout_element(
            &group.layout,
            1.0,
            (Vec::new(), &mut index),
            scale,
            window,
            cx,
        ))
        .into_any_element()
    }

    fn layout_element(
        &self,
        layout: &Layout,
        share: f32,
        place: (Vec<bool>, &mut u32),
        scale: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (path, index) = place;
        let room = div()
            .flex()
            .flex_basis(px(0.0))
            .flex_grow(share)
            .min_w_0()
            .min_h_0();
        match layout {
            Layout::Leaf(id) => {
                // The landmark measures the pane's box; flush panes have no border of their own, the dividers do.
                let pane = room
                    .relative()
                    .child(
                        div()
                            .flex()
                            .size_full()
                            .overflow_hidden()
                            .bg(crate::grid_element::colour(self.palette.background))
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_h_0()
                                    .children(self.panes.get(id).map(|pane| pane.0.clone())),
                            )
                            .children(self.command_bar(id, *index, window, cx)),
                    )
                    .child(self.kit.record().landmark("pane", *index));
                *index += 1;
                self.editors_beside(id, share, pane.into_any_element(), scale)
            }
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let room = match axis {
                    Axis::Columns => room.flex_row(),
                    Axis::Rows => room.flex_col(),
                };
                let ratio = *ratio as f32;
                let mut first_path = path.clone();
                first_path.push(false);
                let mut second_path = path.clone();
                second_path.push(true);
                let first =
                    self.layout_element(first, ratio, (first_path, index), scale, window, cx);
                let second = self.layout_element(
                    second,
                    1.0 - ratio,
                    (second_path, index),
                    scale,
                    window,
                    cx,
                );
                room.relative()
                    .child(first)
                    .child(self.divider(path.clone(), *axis, scale, cx))
                    .child(second)
                    .child(self.places.split(path))
                    .into_any_element()
            }
        }
    }

    fn status_line(&self, scale: f32) -> AnyElement {
        let colors = &self.colors;
        let tabs = self.workspace.groups.len();
        let word = |text: String| div().flex_none().child(text);
        let count = div()
            .relative()
            .flex_none()
            .child(tabs.to_string())
            .child(self.kit.record().landmark("status-count", 0));
        let mut left = div()
            .flex()
            .flex_1()
            .min_w_0()
            .gap(px(t::SPACE_16))
            .child(word("Session 1".into()))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(t::SPACE_4))
                    .child(count)
                    .child(if tabs == 1 { "tab" } else { "tabs" }),
            )
            .child(word(self.status.uptime.clone()));
        // The record's `notice` landmark carries the words, so a gate can read what the line says.
        if let Some(notice) = &self.notice {
            left = left.child(
                div()
                    .relative()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(paint(colors.err, 1.0))
                    .child(notice.clone())
                    .child(
                        self.kit
                            .record()
                            .landmark_with_text("notice", 0, notice.clone()),
                    ),
            );
        } else {
            self.kit.record().landmark_absent("notice", 0);
        }
        let mut right = div().flex().flex_none().gap(px(t::SPACE_16)).child(word(
            if self.status.masking_ready {
                "masking on"
            } else {
                "checking masking"
            }
            .into(),
        ));
        if !self.post_ai {
            let agents = self.status.agents;
            right = right.child(word(format!(
                "{agents} {}",
                if agents == 1 { "agent" } else { "agents" }
            )));
        }
        // The landmark measures the whole line, its top line included, so the line is the inner element's border.
        div()
            .relative()
            .flex_none()
            .h(px(t::STATUS_HEIGHT))
            .child(
                div()
                    .flex()
                    .size_full()
                    .items_center()
                    .gap(px(t::SPACE_16))
                    .px(px(t::SPACE_12))
                    .border_t(px(t::hairline(scale)))
                    .border_color(paint(colors.border, 1.0))
                    .text_size(px(t::TEXT_STATUS))
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(left)
                    .child(right),
            )
            .child(self.kit.record().landmark("status-line", 0))
            .into_any_element()
    }

    /// Placed after everything else: logs the frame, and with a trace how long it took from the start of render to
    /// the end of this paint.
    fn frame_end(&mut self) -> AnyElement {
        let started = self.draw_started.take();
        let trace = Rc::clone(&self.trace);
        canvas(
            |_, _, _| (),
            move |_, (), _, _| {
                let mut trace = trace.borrow_mut();
                if let Some(started) = started {
                    trace.drawn(started.elapsed().as_micros());
                }
                trace.presented();
            },
        )
        .into_any_element()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.trace.borrow().enabled() {
            self.draw_started = Some(Instant::now());
        }
        self.kit.begin_frame();
        self.places.clear();
        self.side_tabs_frame(cx);
        let scale = window.scale_factor();
        let title_bar = self.title_bar(window, cx);
        let record = self.kit.record().clone();
        let vertical = self.vertical();
        let mut body = div().flex().flex_1().min_h_0();
        let mut animate = false;
        if vertical {
            let (column, moving) = self.side_column(window, cx);
            animate = moving;
            body = body
                .child(column)
                .child(self.hairline(Axis::Columns, scale));
        } else {
            record.landmark_absent("side-tabs", 0);
        }
        // The tools panel: on the left with top tabs, on the right while the side tabs hold the left.
        let tools = self.tools_panel(window, cx).map(|panel| {
            div()
                .relative()
                .flex()
                .flex_none()
                .child(panel)
                .child(record.landmark("tools-panel", 0))
        });
        if tools.is_none() {
            record.landmark_absent("tools-panel", 0);
        }
        body = match tools {
            Some(tools) if vertical => body
                .child(self.panes_area(scale, window, cx))
                .child(self.hairline(Axis::Columns, scale))
                .child(tools),
            Some(tools) => body
                .child(tools)
                .child(self.hairline(Axis::Columns, scale))
                .child(self.panes_area(scale, window, cx)),
            None => body.child(self.panes_area(scale, window, cx)),
        };
        self.side_schedule(animate, window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(paint(t::frame(&self.colors), 1.0))
            .font_family(INTERFACE_FONT_FAMILY)
            .text_color(paint(self.colors.foreground, 1.0))
            .capture_key_down(cx.listener(Self::shortcut))
            .child(title_bar)
            .child(self.hairline(Axis::Rows, scale))
            .child(body)
            .child(self.status_line(scale))
            .children(self.settings_layer(window, cx))
            .children(self.floating_layer(window, cx))
            .child(crate::popover::shadow_warmup())
            .child(Self::drag_listener(cx))
            .child(self.frame_end())
            .child(self.kit.record().flush())
    }
}

/// The chord a gpui keystroke stands for, in clanky-state's terms; None for a key the table never names.
pub fn chord_of(keystroke: &Keystroke) -> Option<Chord> {
    let modifiers = &keystroke.modifiers;
    if modifiers.platform || modifiers.function {
        return None;
    }
    let key = match keystroke.key.as_str() {
        "enter" => Key::Enter,
        "escape" => Key::Escape,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        name => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(only), None) => Key::Char(only.to_ascii_lowercase()),
                (Some('f'), Some(_)) => Key::F(name[1..].parse().ok()?),
                _ => return None,
            }
        }
    };
    Some(Chord::new(
        Mods {
            ctrl: modifiers.control,
            shift: modifiers.shift,
            alt: modifiers.alt,
        },
        key,
    ))
}

/// The window's theme, for the interface and the terminal alike: the settings file's light theme when it asks for
/// light, its dark theme otherwise (the window does not yet follow the desktop's mode), and Graphite, the rulebook's
/// default, where the file names none.
pub fn interface_theme(config: &Config, is_set: &dyn Fn(&str) -> bool) -> Result<Theme, String> {
    let (key, saved, default) = if config.theme == ThemePreference::Light {
        ("theme_light", config.theme_light.as_str(), "graphite-light")
    } else {
        ("theme_dark", config.theme_dark.as_str(), "graphite-dark")
    };
    let id = if is_set(key) { saved } else { default };
    clanky_state::themes::builtin(id)
        .or_else(|_| clanky_state::themes::builtin(default))
        .map_err(|e| format!("loading the interface theme: {e}"))
}

/// One look at the server for the status line and the tab titles. A failed request leaves its part empty: the
/// window keeps working, and the status line says only what it knows.
fn read_status(
    client: &Client,
    focuses: &[String],
    inputs: &[String],
    query: &ToolsQuery,
) -> Status {
    let look = request_log::exchange(client, 3, "look_state", &json!({})).unwrap_or(Value::Null);
    request_log::note("workspace_get");
    let revision = client
        .workspace_get()
        .map_or(0, |workspace| workspace.revision);
    // One terminal_info per pane, shared by the tab titles, the chips and the side tabs.
    let mut infos: BTreeMap<String, Value> = BTreeMap::new();
    for id in focuses.iter().chain(inputs) {
        if !infos.contains_key(id)
            && let Ok(info) =
                request_log::exchange(client, 3, "terminal_info", &json!({"tab_id": id}))
        {
            infos.insert(id.clone(), info);
        }
    }
    let folders = focuses
        .iter()
        .filter_map(|id| {
            let folder = match infos.get(id) {
                Some(info) => folder_name(info["cwd"].as_str()?),
                None => folder_of(client, id),
            };
            Some((id.clone(), folder?))
        })
        .collect();
    let home = clanky_core::platform::home_dir().and_then(|home| home.to_str().map(str::to_owned));
    let side = focuses
        .iter()
        .map(|id| {
            (
                id.clone(),
                read_pane_look(client, id, infos.get(id), home.as_deref()),
            )
        })
        .collect();
    Status {
        revision,
        folders,
        uptime: clanky::calm::uptime_label(look["uptime_ms"].as_u64().unwrap_or(0) / 1000),
        masking_ready: look["masking_ready"].as_bool().unwrap_or(false),
        agents: look["agents"].as_array().map_or(0, Vec::len),
        tools: read_tools(client, &look, query),
        chips: read_chips(client, inputs, &infos),
        side,
    }
}

/// The last name of a folder's path.
fn folder_name(cwd: &str) -> Option<String> {
    let name = std::path::Path::new(cwd)
        .file_name()?
        .to_string_lossy()
        .into_owned();
    Some(name)
}

/// The name of the folder a pane's shell is in, or None when the server cannot say.
fn folder_of(client: &Client, id: &str) -> Option<String> {
    let reply = request_log::exchange(client, 3, "terminal_cwd", &json!({"tab_id": id})).ok()?;
    folder_name(reply["cwd"].as_str()?)
}

/// The window's frame trace, shared with its panes.
pub fn shared_trace(trace: clanky::frame_trace::FrameTrace) -> SharedTrace {
    Rc::new(RefCell::new(trace))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(text: &str) -> Keystroke {
        Keystroke::parse(text).expect("a valid keystroke")
    }

    #[test]
    fn the_interface_theme_follows_the_saved_preference() {
        let named = |config: &Config| {
            interface_theme(config, &|_| true)
                .expect("a built-in theme")
                .name
        };
        let unset = Config::window_from_json("{}").expect("an empty config is valid");
        assert_eq!(
            interface_theme(&unset, &|_| false).expect("graphite").name,
            "Graphite"
        );
        let light = Config::window_from_json(
            r#"{"theme":"light","theme_light":"zinc-light","theme_dark":"neutral-dark"}"#,
        )
        .expect("the config is valid");
        assert_eq!(named(&light), "Zinc");
        let dark = Config::window_from_json(
            r#"{"theme":"dark","theme_light":"zinc-light","theme_dark":"neutral-dark"}"#,
        )
        .expect("the config is valid");
        assert_eq!(named(&dark), "Neutral");
        let system = Config::window_from_json(r#"{"theme":"system","theme_dark":"zinc-dark"}"#)
            .expect("the config is valid");
        assert_eq!(named(&system), "Zinc");
        let light_default =
            Config::window_from_json(r#"{"theme":"light"}"#).expect("the config is valid");
        let theme = interface_theme(&light_default, &|_| false).expect("graphite light");
        assert_eq!(
            (theme.name.as_str(), theme.appearance),
            ("Graphite", clanky_state::themes::Appearance::Light)
        );
    }

    #[test]
    fn keystrokes_become_the_tables_chords() {
        let chord = |text| chord_of(&stroke(text)).map(|chord| chord.label());
        assert_eq!(chord("ctrl-shift-t").as_deref(), Some("Ctrl+Shift+T"));
        assert_eq!(chord("ctrl-tab").as_deref(), Some("Ctrl+Tab"));
        assert_eq!(chord("ctrl-shift-f6").as_deref(), Some("Ctrl+Shift+F6"));
        assert_eq!(chord("ctrl-=").as_deref(), Some("Ctrl+="));
        assert_eq!(chord("a").as_deref(), Some("A"));
        // The Super key is the desktop's: no chord of the table uses it.
        assert_eq!(chord("super-t"), None);
        assert_eq!(
            chord_of(&stroke("ctrl-shift-d")).and_then(action_for),
            Some(Action::SplitRight)
        );
        assert_eq!(chord_of(&stroke("ctrl-d")).and_then(action_for), None);
    }
}
