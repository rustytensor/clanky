//! The side tabs (config `tab_style` vertical): a column on the window's left, under the title bar, listing every tab
//! as a row with its program's mark, a badge for its state, its title and its git branch or folder, grouped under the
//! name of the repository each tab's folder is in (`tab_groups` project). A filter field and a new-tab button head
//! the column; the title bar's first button folds it into a rail of icons. In the horizontal style the same states
//! show as a dot before each top tab's title.
//!
//! What a row shows comes from the window's once-a-second look at the server ([`read_pane_look`]: one
//! `terminal_info` and one `input_state` per tab's focused pane) and from the pane's own command blocks. Titles,
//! branches, folders and program names come from the file system and from programs, so every piece of them is
//! escaped ([`escape`]) before it is drawn or recorded.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    time::{Duration, Instant},
};

use clanky::config::{AgentTool, TabGroups};
use clanky_api::Client;
use clanky_state::{blocks::home_short, pane_input};
use gpui::{
    AnyElement, App, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled, Task, Window, div, px, svg,
};
use serde_json::{Map, Value, json};

use super::{MainWindow, NEW_TAB_ID, SCREEN};
use crate::{
    control_look::{paint, with_alpha},
    controls::{Control, Group, INTERFACE_FONT_FAMILY, Kind},
    icons::Icon,
    request_log, tokens as t,
};

/// Control ids: the filter field, then a row per tab and a close button per tab, by the tab's index.
const FILTER_ID: u32 = 6990;
const FIRST_ROW_ID: u32 = 7000;
const FIRST_CLOSE_ID: u32 = 8000;
const ROWS: Group = Group::vertical("side-tabs");
const FILTER_PLACEHOLDER: &str = "Filter tabs";
/// The group of tabs whose folder is in no repository; it always comes last.
const OTHER_GROUP: &str = "Other";
/// The mark of the tool named pi.
const PI_MARK: &str = "\u{03c0}";
/// A row's mark when its pane runs no agent.
const SHELL_MARK: &str = "shell";
/// Shells a foreground program may be named by when it is not the pane's own executable's name (`sh` for bash).
const SHELLS: [&str; 10] = [
    "bash", "sh", "zsh", "fish", "dash", "ksh", "mksh", "tcsh", "csh", "nu",
];

/// What the window keeps for the side column between frames.
pub(super) struct SideTabs {
    /// Folded into the rail of icons.
    pub(super) folded: bool,
    filter: String,
    field: FocusHandle,
    /// Each pane's count of output screens when its tab was last the active one.
    seen: HashMap<String, u64>,
    /// The tabs drawn with the unread mark, to tell when the next look changes them.
    pub(super) unread_shown: BTreeSet<usize>,
    /// The most row and group landmarks a frame has drawn: the rest are written empty when fewer are drawn.
    most_rows: u32,
    most_groups: u32,
    /// When the badges' animation started, and the timer that draws its next step.
    epoch: Instant,
    step: Option<Task<()>>,
}

impl SideTabs {
    pub(super) fn new(field: FocusHandle) -> Self {
        SideTabs {
            folded: false,
            filter: String::new(),
            field,
            seen: HashMap::new(),
            unread_shown: BTreeSet::new(),
            most_rows: 0,
            most_groups: 0,
            epoch: Instant::now(),
            step: None,
        }
    }
}

/// What the look at the server found for one tab's focused pane.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PaneLook {
    /// The git branch of the pane's folder, or the folder shortened with `~`; escaped.
    sub: String,
    /// Whether `sub` is a branch.
    branch: bool,
    /// The name of the repository the folder is in; None outside any repository. Escaped.
    repository: Option<String>,
    /// The foreground program, when the server could tell.
    program: Option<String>,
    /// The foreground program is the pane's shell (or unknown).
    at_shell: bool,
    /// The server's input state (`running`, `waiting`, `exited`), when it answered.
    input: Option<String>,
    waiting: bool,
}

/// Replaces every control character, and every bidirectional embedding, override or isolate, by U+FFFD: text from a
/// folder name or a program must not move the cursor, ring the bell or reorder what is drawn around it.
pub(super) fn escape(text: &str) -> String {
    text.chars()
        .map(|c| {
            if matches!(c,
                '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}

/// The name of the repository holding `dir`: the folder whose `.git` (a folder, or a worktree's file) is found first
/// going up, as `git rev-parse --show-toplevel` finds it.
fn repository_name(dir: &Path) -> Option<String> {
    dir.ancestors()
        .take(64)
        .find(|folder| folder.join(".git").exists())
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
}

/// The pane's look for the side column, from its `terminal_info` reply (asked once for the chips and the column)
/// and one `input_state` request.
pub(super) fn read_pane_look(
    client: &Client,
    id: &str,
    info: Option<&Value>,
    home: Option<&str>,
) -> PaneLook {
    let info = info.cloned().unwrap_or_default();
    let cwd = info["cwd"].as_str().unwrap_or_default();
    let branch = (!cwd.is_empty())
        .then(|| pane_input::git_branch(Path::new(cwd)))
        .flatten();
    let sub = match &branch {
        Some(branch) => branch.clone(),
        None if cwd.is_empty() => String::new(),
        None => home_short(cwd, home),
    };
    let repository = (!cwd.is_empty())
        .then(|| repository_name(Path::new(cwd)))
        .flatten();
    let program = info["program"].as_str().map(str::to_owned);
    let shell_name = info["exe"]
        .as_str()
        .and_then(|exe| Path::new(exe).file_name())
        .map(|name| name.to_string_lossy().into_owned());
    let at_shell = program
        .as_deref()
        .is_none_or(|program| Some(program) == shell_name.as_deref() || SHELLS.contains(&program));
    let input = request_log::exchange(client, 4, "input_state", &json!({"tab_id": id})).ok();
    PaneLook {
        sub: escape(&sub),
        branch: branch.is_some(),
        repository: repository.as_deref().map(escape),
        program,
        at_shell,
        input: input
            .as_ref()
            .and_then(|reply| reply["state"].as_str().map(str::to_owned)),
        waiting: input.is_some_and(|reply| reply["waiting"] == true),
    }
}

/// A tab's state, as its badge and its words show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RowState {
    Quiet,
    Working,
    Waiting,
    Done,
    Failed(i64),
}

impl RowState {
    /// The state's name in the record.
    pub(super) fn name(self) -> &'static str {
        match self {
            RowState::Quiet => "",
            RowState::Working => "working",
            RowState::Waiting => "waiting",
            RowState::Done => "done",
            RowState::Failed(_) => "failed",
        }
    }

    /// The words at the row's right end.
    fn label(self) -> String {
        match self {
            RowState::Quiet => String::new(),
            RowState::Working => "working".into(),
            RowState::Waiting => "needs you".into(),
            RowState::Done => "done".into(),
            RowState::Failed(code) => format!("exit {code}"),
        }
    }

    /// Working and waiting are the agent colour, done is `ok`, failed `err`.
    pub(super) fn colour(self, colors: &clanky_state::themes::Colors) -> Option<[u8; 4]> {
        match self {
            RowState::Quiet => None,
            RowState::Working | RowState::Waiting => Some(colors.agent),
            RowState::Done => Some(colors.ok),
            RowState::Failed(_) => Some(colors.err),
        }
    }

    /// Whether the badge moves: working steps round, waiting pulses.
    fn moves(self) -> bool {
        matches!(self, RowState::Working | RowState::Waiting)
    }
}

/// The file name of a command line's program.
fn program_of(command: &str) -> Option<&str> {
    let first = command.split_whitespace().next()?;
    Path::new(first).file_name()?.to_str()
}

/// The agent tool a program name stands for: the tool of that name, or whose command runs a program of that name.
fn agent_tool<'a>(program: &str, tools: &'a [AgentTool]) -> Option<&'a AgentTool> {
    tools.iter().find(|tool| {
        tool.name.eq_ignore_ascii_case(program) || program_of(&tool.command) == Some(program)
    })
}

/// A tab's state from its pane's look and its newest finished command block.
pub(super) fn state_of(
    look: &PaneLook,
    last: Option<&(String, i64)>,
    tools: &[AgentTool],
) -> RowState {
    if look.input.as_deref() == Some("exited") {
        return RowState::Quiet;
    }
    if !look.at_shell {
        return if look.waiting {
            RowState::Waiting
        } else {
            RowState::Working
        };
    }
    match last {
        Some((_, code)) if *code != 0 => RowState::Failed(*code),
        Some((command, _))
            if program_of(command).is_some_and(|program| agent_tool(program, tools).is_some()) =>
        {
            RowState::Done
        }
        _ => RowState::Quiet,
    }
}

/// The program's mark and whether it is an agent: π for the tool named pi, else the tool name's first letter in
/// capitals; `shell` for anything that is not an agent tool.
fn mark_of(program: Option<&str>, tools: &[AgentTool]) -> (String, bool) {
    let Some(tool) = program.and_then(|program| agent_tool(program, tools)) else {
        return (SHELL_MARK.into(), false);
    };
    if tool.name.eq_ignore_ascii_case("pi") {
        return (PI_MARK.into(), true);
    }
    let first: String = tool
        .name
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect();
    (escape(&first), true)
}

/// One row as the column draws it.
struct Row {
    tab: usize,
    title: String,
    sub: String,
    branch: bool,
    /// The group name shown over the row, `Other` outside a repository; None with tab_groups none.
    group: Option<String>,
    mark: String,
    agent: bool,
    state: RowState,
    unread: bool,
}

impl Row {
    fn matches(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let filter = filter.to_lowercase();
        [Some(&self.title), Some(&self.sub), self.group.as_ref()]
            .into_iter()
            .flatten()
            .any(|text| text.to_lowercase().contains(&filter))
    }
}

/// The rows in the order drawn, each with the group it opens when it is its group's first: groups in the order of
/// their first tab, `Other` last, tabs in tab order inside a group.
fn arrange(rows: Vec<Row>, grouped: bool) -> Vec<(Option<String>, Row)> {
    if !grouped {
        return rows.into_iter().map(|row| (None, row)).collect();
    }
    let mut order: Vec<String> = Vec::new();
    for row in &rows {
        let name = row.group.clone().unwrap_or_default();
        if name != OTHER_GROUP && !order.contains(&name) {
            order.push(name);
        }
    }
    order.push(OTHER_GROUP.into());
    let mut by_group: BTreeMap<usize, Vec<Row>> = BTreeMap::new();
    for row in rows {
        let name = row.group.clone().unwrap_or_default();
        let place = order
            .iter()
            .position(|group| *group == name)
            .unwrap_or(order.len() - 1);
        by_group.entry(place).or_default().push(row);
    }
    let mut out = Vec::new();
    for (place, rows) in by_group {
        for (index, row) in rows.into_iter().enumerate() {
            out.push(((index == 0).then(|| order[place].clone()), row));
        }
    }
    out
}

impl MainWindow {
    /// Whether the window shows side tabs (`tab_style` vertical).
    pub(super) fn vertical(&self) -> bool {
        self.tab_style == clanky::config::TabStyle::Vertical
    }

    /// The side column's width as drawn: folded or not, and wider under the `side-width` planted fault.
    pub(super) fn side_width(&self) -> f32 {
        if self.side.folded {
            t::SIDE_COLUMN_FOLDED_WIDTH
        } else if self.kit.record().faults().side_width() {
            t::SIDE_COLUMN_WIDTH + t::SIDE_COLUMN_FAULT_EXTRA
        } else {
            t::SIDE_COLUMN_WIDTH
        }
    }

    /// The palette's tab switch: top tabs become side tabs or the other way round at once, and `tab_style` is saved
    /// to the settings file. A file that cannot take it (the old JSON format, one that does not parse, a link) is left
    /// as it is, and the status line says the choice was not saved.
    pub(super) fn toggle_side_tabs(&mut self, cx: &mut Context<Self>) {
        use clanky::config::TabStyle;
        let (style, value) = if self.vertical() {
            (TabStyle::Horizontal, "horizontal")
        } else {
            (TabStyle::Vertical, "vertical")
        };
        self.tab_style = style;
        self.side.folded = false;
        self.notice = match self.config_file.as_deref() {
            Some(path) if clanky_state::config::is_json_path(path) => Some(format!(
                "Tabs switched, not saved: {} is the old JSON settings file",
                path.display()
            )),
            Some(_) if self.kit.record().faults().tab_switch_unsaved() => None,
            Some(path) => clanky_state::config_xml::set_scalar(path, "tab_style", value)
                .err()
                .map(|error| format!("Tabs switched, not saved: {error}")),
            None => Some("Tabs switched, not saved: there is no settings file".to_owned()),
        };
        cx.notify();
    }

    pub(super) fn toggle_side_fold(&mut self, cx: &mut Context<Self>) {
        self.side.folded = !self.side.folded;
        cx.notify();
    }

    /// The state of tab `index`, from the last look and its focused pane's blocks.
    pub(super) fn tab_state(&self, index: usize, cx: &App) -> RowState {
        let Some(group) = self.workspace.groups.get(index) else {
            return RowState::Quiet;
        };
        let Some(look) = self.status.side.get(&group.focus) else {
            return RowState::Quiet;
        };
        let last = self
            .panes
            .get(&group.focus)
            .and_then(|pane| pane.0.read(cx).last_finished());
        state_of(look, last.as_ref(), &self.agent_programs)
    }

    /// The tabs with output the user has not seen: not the active one, and a pane of it has shown new screens since
    /// its tab was last active.
    pub(super) fn unread_tabs(&self, cx: &App) -> BTreeSet<usize> {
        self.workspace
            .groups
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != self.workspace.active)
            .filter(|(_, group)| {
                group.layout.leaves().iter().any(|id| {
                    self.panes.get(id).is_some_and(|pane| {
                        pane.0.read(cx).outputs() > self.side.seen.get(id).copied().unwrap_or(0)
                    })
                })
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Called at the start of every frame: the active tab's output counts as seen.
    pub(super) fn side_tabs_frame(&mut self, cx: &App) {
        if let Some(group) = self.workspace.groups.get(self.workspace.active) {
            for id in group.layout.leaves() {
                if let Some(pane) = self.panes.get(&id) {
                    self.side.seen.insert(id, pane.0.read(cx).outputs());
                }
            }
        }
        self.side.unread_shown = self.unread_tabs(cx);
    }

    /// Keys while the filter field has the keyboard: typed text, Backspace, and Escape, which clears the field (and
    /// once it is empty gives the keyboard back to the pane, as Escape does from any control). True when taken.
    pub(super) fn side_filter_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.vertical() || !self.side.field.is_focused(window) {
            return false;
        }
        let keystroke = &event.keystroke;
        let modifiers = &keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        match keystroke.key.as_str() {
            "escape" if !self.side.filter.is_empty() => self.side.filter.clear(),
            "backspace" => {
                self.side.filter.pop();
            }
            "escape" | "tab" | "enter" => return false,
            _ => match keystroke.key_char.as_deref() {
                Some(text) if !text.is_empty() => {
                    self.side.filter.push_str(&escape(text));
                }
                _ => return false,
            },
        }
        cx.notify();
        true
    }

    /// Whether any drawn badge moves, and the window lets it: focused, with motion on.
    fn side_animates(&self, rows: &[(Option<String>, Row)], window: &Window) -> bool {
        self.kit.motion()
            && window.is_window_active()
            && rows.iter().any(|(_, row)| row.state.moves())
    }

    /// After a frame that drew a moving badge: the next spinner step is drawn a step's time later. Nothing is
    /// scheduled while no badge moves, so an idle window draws no frames.
    pub(super) fn side_schedule(
        &mut self,
        animate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !animate {
            self.side.step = None;
            return;
        }
        if self.side.step.is_some() {
            return;
        }
        let step = Duration::from_millis(u64::from(1000 / t::SPINNER_STEPS_PER_SECOND));
        self.side.step = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(step).await;
            let _ = this.update(cx, |this, cx| {
                this.side.step = None;
                cx.notify();
            });
        }));
    }

    /// The rows for the tabs, filtered, in the order drawn.
    fn side_rows(&self, cx: &App) -> Vec<(Option<String>, Row)> {
        let grouped = self.tab_groups == TabGroups::Project;
        let rows = (0..self.workspace.groups.len())
            .map(|index| {
                let focus = &self.workspace.groups[index].focus;
                let look = self.status.side.get(focus).cloned().unwrap_or_default();
                let (mark, agent) = mark_of(look.program.as_deref(), &self.agent_programs);
                Row {
                    tab: index,
                    title: self.tab_title(index),
                    group: grouped.then(|| {
                        look.repository
                            .clone()
                            .unwrap_or_else(|| OTHER_GROUP.to_owned())
                    }),
                    sub: look.sub.clone(),
                    branch: look.branch,
                    mark,
                    agent,
                    state: self.tab_state(index, cx),
                    unread: self.side.unread_shown.contains(&index),
                }
            })
            .filter(|row| row.matches(&self.side.filter))
            .collect();
        arrange(rows, grouped)
    }

    /// The side column, and whether one of its badges moves (so the window draws its next step).
    pub(super) fn side_column(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let rows = self.side_rows(cx);
        let animate = self.side_animates(&rows, window);
        let folded = self.side.folded;
        let width = self.side_width();
        let pad_x = if folded {
            t::SPACE_6
        } else {
            t::SIDE_COLUMN_PADDING
        };
        let inner = width - 2.0 * pad_x;
        let record = self.kit.record().clone();
        let mut list = div()
            .id("side-tabs-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(t::SPACE_2))
            // One focus ring's width of room inside the clip, so a focused row's ring is drawn whole.
            .m(px(-t::FOCUS_RING_WIDTH))
            .p(px(t::FOCUS_RING_WIDTH))
            .overflow_y_scroll();
        let (mut row_count, mut group_count) = (0_u32, 0_u32);
        for (group, row) in &rows {
            if let Some(name) = group.as_ref().filter(|_| !folded) {
                list = list.child(
                    div()
                        .relative()
                        .flex_none()
                        .px(px(t::SPACE_8))
                        .pt(px(t::SPACE_8))
                        .pb(px(t::SPACE_4))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(t::SIDE_GROUP_TEXT))
                        .text_color(paint(self.colors.muted_foreground, 1.0))
                        .child(name.clone())
                        .child(record.landmark_with_text(
                            "side-tab-group",
                            group_count,
                            name.clone(),
                        )),
                );
                group_count += 1;
            }
            list = list.child(self.side_row(row, row_count, inner, animate, window, cx));
            row_count += 1;
        }
        for index in row_count..self.side.most_rows {
            record.landmark_absent("side-tab-row", index);
        }
        for index in group_count..self.side.most_groups {
            record.landmark_absent("side-tab-group", index);
        }
        self.side.most_rows = self.side.most_rows.max(row_count);
        self.side.most_groups = self.side.most_groups.max(group_count);
        let column = div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .min_h_0()
            .px(px(pad_x))
            .py(px(t::SIDE_COLUMN_PADDING))
            .gap(px(t::SPACE_8))
            .overflow_hidden()
            .child(self.side_head(inner, window, cx))
            .child(list)
            .child(record.landmark("side-tabs", 0));
        (column.into_any_element(), animate)
    }

    /// The filter field and the new-tab button; folded, the button alone.
    fn side_head(&self, inner: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        let new_tab = Control::new(Kind::IconButton, NEW_TAB_ID, "New tab")
            .icon(Icon::Plus)
            .on_click(move |window, cx| {
                this.update(cx, |this, cx| this.act(window, cx, Self::new_tab));
            });
        let mut head = div().flex().flex_none().items_center().gap(px(t::SPACE_4));
        if self.side.folded {
            head = head.justify_center();
        } else {
            let filter = self.side.filter.clone();
            let typing = self.side.field.is_focused(window);
            let colors = &self.colors;
            let text = if filter.is_empty() {
                div()
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(FILTER_PLACEHOLDER)
            } else {
                div()
                    .text_color(paint(colors.foreground, 1.0))
                    .child(filter.clone())
            }
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis();
            let caret = typing.then(|| {
                div()
                    .flex_none()
                    .w(px(t::hairline(window.scale_factor())))
                    .h(px(t::CARET_HEIGHT))
                    .bg(paint(colors.foreground, 1.0))
            });
            let mut line = div()
                .flex()
                .items_center()
                .w_full()
                .text_size(px(t::TEXT_UI));
            line = if filter.is_empty() {
                line.children(caret).child(text)
            } else {
                line.child(text).children(caret)
            };
            let field = Control::new(Kind::Field, FILTER_ID, FILTER_PLACEHOLDER)
                .icon(Icon::Search)
                .focus(self.side.field.clone())
                .height(t::SIDE_FILTER_HEIGHT)
                .width(inner - t::SPACE_4 - t::ICON_BUTTON_HEIGHT_BAR)
                .field_text(filter)
                .content(line);
            head = head.child(self.kit.build(SCREEN, field, window, cx));
        }
        head.child(self.kit.build(SCREEN, new_tab, window, cx))
            .into_any_element()
    }

    /// One tab's row, `place`-th on screen, `width` wide.
    fn side_row(
        &self,
        row: &Row,
        place: u32,
        width: f32,
        animate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = row.tab;
        let active = index == self.workspace.active;
        let folded = self.side.folded;
        let id = FIRST_ROW_ID + index as u32;
        let colors = &self.colors;
        let icon = self.session_icon(row, active, animate);
        let mut content = div()
            .flex()
            .items_center()
            .w_full()
            .h_full()
            .gap(px(t::ROW_GAP))
            .child(icon);
        if folded {
            content = content.justify_center();
        } else {
            let title = div()
                .relative()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_family(INTERFACE_FONT_FAMILY)
                .text_size(px(t::TEXT_TITLE))
                .font_weight(FontWeight(f32::from(t::WEIGHT_MEDIUM)))
                .text_color(paint(colors.foreground, 1.0))
                .child(row.title.clone())
                .child(
                    self.kit
                        .record()
                        .part(SCREEN, id, crate::control_record::Part::Label),
                );
            let sub_icon = if row.branch {
                Icon::Branch
            } else {
                Icon::Folder
            };
            let mut sub = div()
                .flex()
                .items_center()
                .gap(px(t::SPACE_4))
                .min_w_0()
                .font_family(t::TERMINAL_FONT_FAMILY)
                .text_size(px(t::SIDE_SUB_TEXT))
                .text_color(paint(colors.muted_foreground, 1.0));
            if !row.sub.is_empty() {
                sub = sub
                    .child(
                        svg()
                            .flex_none()
                            .data(sub_icon.svg())
                            .size(px(t::SIDE_SUB_ICON_SIZE))
                            .text_color(paint(colors.muted_foreground, 1.0)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(row.sub.clone()),
                    );
            }
            if let Some(ink) = row.state.colour(colors) {
                sub = sub.child(
                    div()
                        .ml_auto()
                        .flex_none()
                        .whitespace_nowrap()
                        .font_family(INTERFACE_FONT_FAMILY)
                        .text_size(px(t::SIDE_STATE_TEXT))
                        .text_color(paint(ink, 1.0))
                        .child(row.state.label()),
                );
            }
            content = content.px(px(t::SPACE_8)).child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(t::SPACE_2))
                    .child(title)
                    .child(sub),
            );
        }
        let this = cx.entity();
        let mut control = Control::new(Kind::ListRow, id, row.title.clone())
            .tab_row()
            .group(ROWS)
            .checked(active)
            .height(t::SIDE_TAB_ROW_HEIGHT)
            .width(width)
            .content(content)
            .on_click(move |window, cx| {
                this.update(cx, |this, cx| {
                    this.act(window, cx, move |this, _, _| this.select_tab(index));
                });
            });
        if !folded {
            let leaves = self.workspace.groups[index].layout.leaves();
            let closer = cx.entity();
            let close = Control::new(
                Kind::IconButtonInline,
                FIRST_CLOSE_ID + index as u32,
                format!("Close {}", row.title),
            )
            .icon(Icon::Close)
            .inside(false)
            .on_click(move |window, cx| {
                // The row under the button must not take this click as its own.
                cx.stop_propagation();
                let leaves = leaves.clone();
                closer.update(cx, |this, cx| {
                    this.act(window, cx, move |this, _, _| this.close_terminals(&leaves));
                });
            });
            control = control.trailing(self.kit.build(SCREEN, close, window, cx));
        }
        let mut fields = Map::new();
        fields.insert("tab".into(), json!(index));
        fields.insert("text".into(), json!(row.title));
        fields.insert("sub".into(), json!(row.sub));
        fields.insert("mark".into(), json!(row.mark));
        fields.insert("agent".into(), json!(row.agent));
        fields.insert("state".into(), json!(row.state.name()));
        fields.insert("state_label".into(), json!(row.state.label()));
        fields.insert("unread".into(), json!(row.unread));
        fields.insert("group".into(), json!(row.group));
        div()
            .relative()
            .flex_none()
            .child(self.kit.build(SCREEN, control, window, cx))
            .child(
                self.kit
                    .record()
                    .landmark_with_fields("side-tab-row", place, fields),
            )
            .into_any_element()
    }

    /// The session icon: the program's mark in a rounded square, with the state badge on its bottom-right corner.
    fn session_icon(&self, row: &Row, active: bool, animate: bool) -> AnyElement {
        let colors = &self.colors;
        let (fill, ink) = if row.agent {
            (
                with_alpha(colors.agent, t::SESSION_AGENT_FILL),
                colors.agent,
            )
        } else {
            (colors.muted, colors.muted_foreground)
        };
        let mut icon = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(t::SESSION_ICON_SIZE))
            .rounded(px(t::RADIUS_MEDIUM))
            .bg(paint(fill, 1.0));
        icon = if row.agent {
            icon.child(
                div()
                    .font_family(t::TERMINAL_FONT_FAMILY)
                    .text_size(px(t::SESSION_MARK_TEXT))
                    .font_weight(FontWeight(f32::from(t::WEIGHT_MEDIUM)))
                    .text_color(paint(ink, 1.0))
                    .child(row.mark.clone()),
            )
        } else {
            icon.child(
                svg()
                    .data(Icon::Terminal.svg())
                    .size(px(t::ICON_SIZE_BAR))
                    .text_color(paint(ink, 1.0)),
            )
        };
        icon.children(self.badge(row, active, animate))
            .into_any_element()
    }

    /// The state badge: a circle edged in the frame colour (in the foreground while the tab has unread output), its
    /// dot in the state's colour. A working badge has a notch of the edge's colour stepping round it; a waiting one's
    /// dot pulses in size, so its centre always shows its colour.
    fn badge(&self, row: &Row, active: bool, animate: bool) -> Option<AnyElement> {
        let colors = &self.colors;
        let dot_colour = match row.state.colour(colors) {
            Some(colour) => colour,
            None if row.unread => colors.muted,
            None => return None,
        };
        let frame = t::frame(colors);
        let edge = if row.unread { colors.foreground } else { frame };
        let elapsed = if animate {
            self.side.epoch.elapsed().as_secs_f32() * 1000.0
        } else {
            0.0
        };
        let mut badge = div()
            .absolute()
            .right(px(-t::BADGE_OUTSET))
            .bottom(px(-t::BADGE_OUTSET))
            .size(px(t::BADGE_SIZE))
            .rounded_full()
            .bg(paint(edge, 1.0));
        if active && !row.unread {
            // On the active row the edge takes the row's pill colour, as the row's fill lies over the frame.
            badge = badge.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded_full()
                    .bg(paint(
                        with_alpha(colors.foreground, t::ACTIVE_TAB_FILL),
                        1.0,
                    )),
            );
        }
        let dot = t::BADGE_SIZE - 2.0 * t::BADGE_EDGE;
        let shrink = if row.state == RowState::Waiting {
            // Half a cosine from full size to the smallest and back, once a period.
            let phase = (elapsed % t::PULSE_PERIOD_MS) / t::PULSE_PERIOD_MS;
            t::PULSE_SHRINK * (1.0 - (phase * std::f32::consts::TAU).cos()) / 2.0
        } else {
            0.0
        };
        badge = badge.child(
            div()
                .absolute()
                .top(px(t::BADGE_EDGE + shrink))
                .left(px(t::BADGE_EDGE + shrink))
                .size(px(dot - 2.0 * shrink))
                .rounded_full()
                .bg(paint(dot_colour, 1.0)),
        );
        if row.state == RowState::Working {
            let steps = t::SPINNER_STEPS_PER_TURN as f32;
            let step = (elapsed / 1000.0 * t::SPINNER_STEPS_PER_SECOND as f32).floor() % steps;
            let angle = step / steps * std::f32::consts::TAU;
            // The notch runs round inside the dot's edge, clear of its centre.
            let radius = (dot - t::SPINNER_NOTCH_SIZE) / 2.0;
            let centre = t::BADGE_SIZE / 2.0;
            badge = badge.child(
                div()
                    .absolute()
                    .left(px(
                        centre + radius * angle.sin() - t::SPINNER_NOTCH_SIZE / 2.0
                    ))
                    .top(px(centre
                        - radius * angle.cos()
                        - t::SPINNER_NOTCH_SIZE / 2.0))
                    .size(px(t::SPINNER_NOTCH_SIZE))
                    .bg(paint(frame, 1.0)),
            );
        }
        Some(badge.into_any_element())
    }

    /// A top tab's state dot (horizontal tabs), and its landmark; None for a tab with no state.
    pub(super) fn tab_dot(&self, index: usize, cx: &App) -> Option<AnyElement> {
        let state = self.tab_state(index, cx);
        let record = self.kit.record();
        let mut fields = Map::new();
        fields.insert("state".into(), json!(state.name()));
        let Some(colour) = state.colour(&self.colors) else {
            record.landmark_absent("tab-dot", index as u32);
            record.set_landmark_fields("tab-dot", index as u32, fields);
            return None;
        };
        Some(
            div()
                .relative()
                .flex_none()
                .size(px(t::TAB_DOT_SIZE))
                .rounded_full()
                .bg(paint(colour, 1.0))
                .child(record.landmark_with_fields("tab-dot", index as u32, fields))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, command: &str) -> AgentTool {
        AgentTool {
            name: name.into(),
            command: command.into(),
        }
    }

    fn look(program: Option<&str>, at_shell: bool, input: &str, waiting: bool) -> PaneLook {
        PaneLook {
            program: program.map(str::to_owned),
            at_shell,
            input: Some(input.into()),
            waiting,
            ..PaneLook::default()
        }
    }

    #[test]
    fn escaping_replaces_controls_and_bidirectional_marks() {
        let text = "a\u{1b}b\u{7}c\u{202e}d\u{2066}e\u{9f}f\u{7f}g";
        assert_eq!(
            escape(text),
            "a\u{fffd}b\u{fffd}c\u{fffd}d\u{fffd}e\u{fffd}f\u{fffd}g"
        );
        assert_eq!(escape("plain ~/git/clanky"), "plain ~/git/clanky");
    }

    #[test]
    fn states_follow_the_program_and_the_last_block() {
        let tools = [tool("pi", "/tmp/fake/pi")];
        let at_prompt = look(Some("bash"), true, "waiting", true);
        assert_eq!(state_of(&at_prompt, None, &tools), RowState::Quiet);
        assert_eq!(
            state_of(&at_prompt, Some(&("false".into(), 1)), &tools),
            RowState::Failed(1)
        );
        assert_eq!(RowState::Failed(1).label(), "exit 1");
        assert_eq!(
            state_of(&at_prompt, Some(&("true".into(), 0)), &tools),
            RowState::Quiet
        );
        assert_eq!(
            state_of(&at_prompt, Some(&("pi --run x".into(), 0)), &tools),
            RowState::Done
        );
        let cat = look(Some("cat"), false, "waiting", true);
        assert_eq!(state_of(&cat, None, &tools), RowState::Waiting);
        assert_eq!(RowState::Waiting.label(), "needs you");
        let busy = look(Some("sleep"), false, "running", false);
        assert_eq!(state_of(&busy, None, &tools), RowState::Working);
        let gone = look(Some("sleep"), false, "exited", false);
        assert_eq!(state_of(&gone, None, &tools), RowState::Quiet);
    }

    #[test]
    fn marks_name_agent_tools_and_nothing_else() {
        let tools = [
            tool("pi", "/tmp/fake/pi"),
            tool("Claude Code", "claude --x"),
        ];
        assert_eq!(mark_of(Some("pi"), &tools), ("\u{03c0}".into(), true));
        assert_eq!(mark_of(Some("claude"), &tools), ("C".into(), true));
        assert_eq!(mark_of(Some("bash"), &tools), ("shell".into(), false));
        assert_eq!(mark_of(None, &tools), ("shell".into(), false));
    }

    fn row(tab: usize, group: &str) -> Row {
        Row {
            tab,
            title: format!("t{tab}"),
            sub: String::new(),
            branch: false,
            group: Some(group.into()),
            mark: SHELL_MARK.into(),
            agent: false,
            state: RowState::Quiet,
            unread: false,
        }
    }

    #[test]
    fn groups_come_in_order_of_their_first_tab_with_other_last() {
        let rows = vec![
            row(0, OTHER_GROUP),
            row(1, "alpha"),
            row(2, "beta"),
            row(3, "alpha"),
        ];
        let arranged = arrange(rows, true);
        let heads: Vec<_> = arranged.iter().filter_map(|(g, _)| g.clone()).collect();
        assert_eq!(heads, ["alpha", "beta", "Other"]);
        let tabs: Vec<_> = arranged.iter().map(|(_, row)| row.tab).collect();
        assert_eq!(tabs, [1, 3, 2, 0]);
    }

    #[test]
    fn the_filter_matches_title_sub_and_group_ignoring_case() {
        let mut beta = row(0, "Beta");
        beta.sub = "main".into();
        assert!(beta.matches("bEtA"));
        assert!(beta.matches("mai"));
        assert!(!beta.matches("alpha"));
        assert!(beta.matches(""));
    }
}
