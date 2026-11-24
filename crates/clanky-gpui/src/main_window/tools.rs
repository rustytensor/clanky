//! The tools panel at the window's left (at its right while the side tabs hold the left): Files (the focused pane's folder as a tree), Recent (what ran lately) and
//! Agents (agent activity, the connected agents, and each pane's agent access), with a switcher between them at its
//! foot. What it shows is read with the window's once-a-second look at the server and drawn only when it changed.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    rc::Rc,
};

use clanky::config::{AgentTool, Tool};
use clanky_api::Client;
use clanky_state::files_tree::{self, TreeRow};
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, Window, div, px};
use serde_json::Value;

use super::{MainWindow, SCREEN};
use crate::{
    control_look::paint,
    controls::{Control, Group, Kind, Switcher},
    icons::Icon,
    request_log, tokens as t,
};

/// Control ids: the switcher's segments, the tree's rows, the agents' rows, the access switches.
const SEGMENT_IDS: u32 = 50;
const TREE_IDS: u32 = 1000;
const AGENT_IDS: u32 = 3000;
const ACCESS_IDS: u32 = 4000;
/// How many recent events the Recent tool lists, newest first.
const RECENT_SHOWN: usize = 20;

/// A pane's id and a command typed in it.
type PaneCommand = (String, String);

/// What the panel shows, read from the server; compared without the recent events' ages, which alone never draw.
#[derive(Clone, Debug, Default)]
pub(super) struct ToolsData {
    /// The folder the Files tool lists, and its rows.
    folder: String,
    tree: Vec<TreeRow>,
    /// Each event, its age, and for a typed command its pane and command (the pane's blocks give its exit code).
    recent: Vec<(clanky::look::TimelineItem, String, Option<PaneCommand>)>,
    /// Agent commands in the last 30 minutes.
    activity: u64,
    /// Connected agents: id, label, running, the pane each works in.
    agents: Vec<(u64, String, bool, Option<String>)>,
    /// Each terminal pane's label and whether agents may use it.
    access: Vec<(String, String, bool)>,
}

impl PartialEq for ToolsData {
    fn eq(&self, other: &Self) -> bool {
        let events = |data: &Self| {
            data.recent
                .iter()
                .map(|(item, _, command)| (item.clone(), command.clone()))
                .collect::<Vec<_>>()
        };
        self.folder == other.folder
            && self.tree == other.tree
            && events(self) == events(other)
            && self.activity == other.activity
            && self.agents == other.agents
            && self.access == other.access
    }
}

/// The panel's own state, kept by the window.
pub(super) struct Tools {
    pub(super) open: bool,
    tool: Tool,
    /// The tools the config lists, in its order.
    listed: Vec<Tool>,
    /// The folders opened in the Files tree, by path.
    tree_open: BTreeSet<String>,
    agent_programs: Vec<AgentTool>,
}

impl Tools {
    pub(super) fn new(open: bool, listed: Vec<Tool>, agent_programs: Vec<AgentTool>) -> Self {
        Tools {
            open,
            tool: listed.first().copied().unwrap_or(Tool::Files),
            listed,
            tree_open: BTreeSet::new(),
            agent_programs,
        }
    }
}

/// What the background look needs to read the panel's data.
pub(super) struct ToolsQuery {
    shown: Option<Tool>,
    focused: Option<String>,
    tree_open: BTreeSet<String>,
    /// Each pane's label ("home · pane 2"), by id, in tab order.
    labels: Vec<(String, String)>,
    agent_programs: Vec<AgentTool>,
    post_ai: bool,
}

/// The panel's data for one look; `look` is the server's `look_state` reply.
pub(super) fn read_tools(client: &Client, look: &Value, query: &ToolsQuery) -> ToolsData {
    let mut data = ToolsData::default();
    let Some(shown) = query.shown else {
        return data;
    };
    match shown {
        Tool::Files => {
            if let Some(cwd) = query
                .focused
                .as_deref()
                .and_then(|id| {
                    request_log::exchange(
                        client,
                        3,
                        "terminal_cwd",
                        &serde_json::json!({"tab_id": id}),
                    )
                    .ok()
                })
                .and_then(|reply| reply["cwd"].as_str().map(str::to_owned))
            {
                data.tree = files_tree::rows(Path::new(&cwd), &query.tree_open);
                data.folder = cwd;
            }
        }
        Tool::Recent => {
            let labels: BTreeMap<String, String> = query.labels.iter().cloned().collect();
            let mut recent = clanky::calm::human_recent(
                &look["recent"],
                &labels,
                &look["agents"],
                &query.agent_programs,
            );
            recent.sort_by_key(|(age, _, _)| std::cmp::Reverse(*age));
            data.recent = recent
                .into_iter()
                .rev()
                .take(RECENT_SHOWN)
                .map(|(age, item, command)| (item, age_text(age), command))
                .collect();
        }
        Tool::Agents => {
            data.activity = look["activity"].as_array().map_or(0, |values| {
                values.iter().take(30).filter_map(Value::as_u64).sum()
            });
            data.agents = agents(look);
            if !query.post_ai {
                data.access = query
                    .labels
                    .iter()
                    .map(|(id, label)| {
                        request_log::note("permission_get");
                        (
                            id.clone(),
                            label.clone(),
                            client.agent_permission(id).unwrap_or(false),
                        )
                    })
                    .collect();
            }
        }
    }
    data
}

fn agents(look: &Value) -> Vec<(u64, String, bool, Option<String>)> {
    look["agents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|agent| {
            let id = agent["connection_id"].as_u64()?;
            let name = agent["client_name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .unwrap_or("Agent");
            let label = agent["model"]
                .as_str()
                .filter(|model| !model.is_empty())
                .map_or_else(|| name.to_owned(), |model| format!("{name} · {model}"));
            Some((
                id,
                label,
                agent["running"].as_bool().unwrap_or(false),
                agent["tab_id"].as_str().map(str::to_owned),
            ))
        })
        .collect()
}

/// An event's age as the Recent tool words it: seconds, then minutes, then hours.
fn age_text(age_ms: u64) -> String {
    let seconds = age_ms / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h", seconds / 3600)
    }
}

impl MainWindow {
    /// The tools this window offers: the config's list, without Agents in a window started with --post-ai.
    fn offered_tools(&self) -> Vec<Tool> {
        self.tools
            .listed
            .iter()
            .copied()
            .filter(|tool| !(self.post_ai && *tool == Tool::Agents))
            .collect()
    }

    fn shown_tool(&self) -> Option<Tool> {
        let offered = self.offered_tools();
        offered
            .contains(&self.tools.tool)
            .then_some(self.tools.tool)
            .or_else(|| offered.first().copied())
    }

    /// The question for the next look; the panel shown or not decides whether anything is read for it.
    pub(super) fn tools_query(&self, focused: Option<String>) -> ToolsQuery {
        let mut labels = Vec::new();
        for (index, group) in self.workspace.groups.iter().enumerate() {
            let title = self.tab_title(index);
            for (pane, id) in group.layout.leaves().into_iter().enumerate() {
                labels.push((id, clanky::calm::pane_label(&title, pane)));
            }
        }
        ToolsQuery {
            shown: self.shown_tool().filter(|_| self.tools.open),
            focused,
            tree_open: self.tools.tree_open.clone(),
            labels,
            agent_programs: self.tools.agent_programs.clone(),
            post_ai: self.post_ai,
        }
    }

    pub(super) fn toggle_tools(&mut self, cx: &mut Context<Self>) {
        self.tools.open = !self.tools.open;
        self.status.tools = ToolsData::default();
        self.read_now(cx);
    }

    fn choose_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tools.tool = tool;
        self.read_now(cx);
    }

    fn toggle_folder(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.tools.tree_open.remove(&path) {
            self.tools.tree_open.insert(path);
        }
        self.read_now(cx);
    }

    /// Reads the panel's data at once, so a choice shows in the next frame rather than at the next look.
    fn read_now(&mut self, cx: &mut Context<Self>) {
        let focused = self
            .workspace
            .groups
            .get(self.workspace.active)
            .map(|group| group.focus.clone());
        let query = self.tools_query(focused);
        let look = request_log::exchange(&self.client, 3, "look_state", &serde_json::json!({}))
            .unwrap_or(Value::Null);
        self.status.tools = read_tools(&self.client, &look, &query);
        cx.notify();
    }

    pub(super) fn set_access(&mut self, id: &str, allowed: bool, cx: &mut Context<Self>) {
        self.notice = self
            .client
            .set_agent_permission(id, allowed)
            .err()
            .map(|e| format!("setting agent access: {}", e.message));
        // The server took it: show it now rather than at the next status read.
        if self.notice.is_none() {
            self.status
                .chips
                .entry(id.to_owned())
                .or_default()
                .set_access(allowed);
            for row in &mut self.status.tools.access {
                if row.0 == id {
                    row.2 = allowed;
                }
            }
            cx.notify();
        }
        self.read_now(cx);
    }

    /// The tools panel, or None while it is closed.
    pub(super) fn tools_panel(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.tools.open {
            return None;
        }
        let offered = self.offered_tools();
        let shown = self.shown_tool()?;
        let data = &self.status.tools;
        let title = match shown {
            Tool::Files if data.folder.is_empty() => "Files".to_owned(),
            Tool::Files => {
                let home = clanky_core::platform::home_dir()
                    .and_then(|home| home.to_str().map(str::to_owned));
                files_tree::folder_name(&data.folder, home.as_deref())
            }
            Tool::Recent => "Recent".to_owned(),
            Tool::Agents => "Agents".to_owned(),
        };
        let body = match shown {
            Tool::Files => self.files_body(window, cx),
            Tool::Recent => self.recent_body(cx),
            Tool::Agents => self.agents_body(window, cx),
        };
        let this = cx.entity();
        let choices = offered.clone();
        let labels: Vec<(u32, &str)> = offered
            .iter()
            .enumerate()
            .map(|(index, tool)| (SEGMENT_IDS + index as u32, tool.label()))
            .collect();
        let switcher = Switcher {
            group: Group::horizontal("tools"),
            segments: &labels,
            selected: offered.iter().position(|tool| *tool == shown).unwrap_or(0),
            choose: Rc::new(move |index, _, cx: &mut App| {
                if let Some(tool) = choices.get(index).copied() {
                    this.update(cx, |this, cx| this.choose_tool(tool, cx));
                }
            }),
        };
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(t::SIDE_COLUMN_WIDTH))
                .min_h_0()
                .p(px(t::SIDE_COLUMN_PADDING))
                .gap(px(t::SPACE_8))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(px(t::BUTTON_HEIGHT_DEFAULT))
                        .px(px(t::SPACE_8))
                        .text_size(px(t::TEXT_TITLE))
                        .font_weight(gpui::FontWeight(f32::from(t::WEIGHT_MEDIUM)))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(title),
                )
                .child(
                    // The body clips what scrolls out of it, one focus ring's width outside its rows, so a focused
                    // row's ring is drawn whole while the rows stay where they were.
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .m(px(-t::FOCUS_RING_WIDTH))
                        .p(px(t::FOCUS_RING_WIDTH))
                        .overflow_hidden()
                        .child(body),
                )
                .child(
                    div()
                        .flex_none()
                        .child(self.kit.segmented(SCREEN, &switcher, window, cx)),
                )
                .into_any_element(),
        )
    }

    fn quiet_line(&self, text: &str) -> AnyElement {
        div()
            .px(px(t::SPACE_8))
            .text_size(px(t::TEXT_TREE))
            .text_color(paint(self.colors.muted_foreground, 1.0))
            .child(text.to_owned())
            .into_any_element()
    }

    fn files_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let data = &self.status.tools;
        if data.tree.is_empty() {
            return self.quiet_line(if data.folder.is_empty() {
                "No folder to show"
            } else {
                "This folder is empty"
            });
        }
        let mut list = div().flex().flex_col();
        let this = cx.entity();
        for (index, row) in data.tree.iter().enumerate() {
            let mut control =
                Control::new(Kind::ListRow, TREE_IDS + index as u32, row.name.clone())
                    .icon(if row.dir { Icon::Folder } else { Icon::File })
                    .tree(row.depth, row.dir.then_some(row.open))
                    .group(Group::vertical("files"));
            // A folder opens and closes; a file opens in an editor pane beside the focused pane.
            let (this, path) = (this.clone(), row.path.clone());
            control = if row.dir {
                control.on_click(move |_, cx| {
                    let path = path.clone();
                    this.update(cx, |this, cx| this.toggle_folder(path, cx));
                })
            } else {
                control.on_click(move |window, cx| {
                    this.update(cx, |this, cx| this.open_editor(&path, window, cx));
                })
            };
            list = list.child(self.kit.build(SCREEN, control, window, cx));
        }
        list.into_any_element()
    }

    /// A finished command's exit code, from the newest finished block on its pane's screen with the same command.
    fn exit_of(&self, command: Option<&(String, String)>, cx: &App) -> Option<i32> {
        let (tab, command) = command?;
        self.panes.get(tab)?.0.read(cx).last_exit(command)
    }

    fn recent_body(&self, cx: &App) -> AnyElement {
        let data = &self.status.tools;
        if data.recent.is_empty() {
            return self.quiet_line("No recent events");
        }
        let colors = &self.colors;
        let mut list = div().flex().flex_col().gap(px(t::SPACE_4));
        for (item, age, command) in &data.recent {
            // Exit-code dots are status dots: `ok` for success, `err` for failure. Every row keeps the dot's room, so
            // the labels line up.
            let tone = self
                .exit_of(command.as_ref(), cx)
                .map_or(gpui::transparent_black(), |code| {
                    paint(if code == 0 { colors.ok } else { colors.err }, 1.0)
                });
            let dot = div()
                .flex_none()
                .size(px(t::SPACE_6))
                .rounded_full()
                .bg(tone);
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(t::SPACE_8))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(t::SPACE_8))
                            .text_size(px(t::TEXT_TREE))
                            .child(dot)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(item.label.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(t::TEXT_STATUS))
                                    .text_color(paint(colors.muted_foreground, 1.0))
                                    .child(age.clone()),
                            ),
                    )
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(t::TEXT_STATUS))
                            .text_color(paint(colors.muted_foreground, 1.0))
                            .child(item.detail.clone()),
                    ),
            );
        }
        list.into_any_element()
    }

    fn section(&self, text: &str) -> AnyElement {
        div()
            .px(px(t::SPACE_8))
            .pt(px(t::SPACE_8))
            .text_size(px(t::TEXT_STATUS))
            .font_weight(gpui::FontWeight(f32::from(t::WEIGHT_MEDIUM)))
            .text_color(paint(self.colors.muted_foreground, 1.0))
            .child(text.to_owned())
            .into_any_element()
    }

    fn agents_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let data = &self.status.tools;
        let this = cx.entity();
        let count = data.activity;
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(t::SPACE_2))
            .child(self.section("Agent activity"))
            .child(self.quiet_line(&format!(
                "{count} {} in 30 min",
                if count == 1 { "command" } else { "commands" }
            )));
        for (index, (_, label, running, tab)) in data.agents.iter().enumerate() {
            let text = format!("{label} · {}", if *running { "running" } else { "idle" });
            let mut control = Control::new(Kind::ListRow, AGENT_IDS + index as u32, text)
                .group(Group::vertical("agents"));
            if let Some(tab) = tab.clone() {
                let this = this.clone();
                control = control.on_click(move |window, cx| {
                    let tab = tab.clone();
                    this.update(cx, |this, cx| {
                        this.act(window, cx, move |this, _, _| this.show_pane(&tab));
                    });
                });
            }
            list = list.child(self.kit.build(SCREEN, control, window, cx));
        }
        if !self.post_ai {
            list = list.child(self.section("Agent access"));
            for (index, (id, label, allowed)) in data.access.iter().enumerate() {
                let (this, id, allowed) = (this.clone(), id.clone(), *allowed);
                let switch = Control::new(
                    Kind::Switch,
                    ACCESS_IDS + index as u32,
                    format!("Agent access to {label}"),
                )
                .checked(allowed)
                .on_click(move |_, cx| {
                    let id = id.clone();
                    this.update(cx, |this, cx| this.set_access(&id, !allowed, cx));
                });
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(t::SPACE_8))
                        .px(px(t::SPACE_8))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(t::TEXT_TREE))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(label.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(t::TEXT_STATUS))
                                        .text_color(paint(self.colors.muted_foreground, 1.0))
                                        .child(if allowed { "read + run" } else { "off" }),
                                ),
                        )
                        .child(self.kit.build(SCREEN, switch, window, cx)),
                );
            }
        }
        list.into_any_element()
    }
}
