//! "Start an agent": the agent picker, opened by Ctrl+Shift+G or the Agent segment of a pane's Shell | Agent switch.
//! It lists the configured agent tools (one not installed is greyed, never hidden), narrowed by what is typed; the
//! arrows move, Enter starts the selected tool in a new pane with its agent access as configured, and Left or Right
//! switches between a split to the right of the focused pane and a new tab. A window started with --post-ai opens
//! no picker. What is listed and the filtering are clanky-state's `agent_picker`; this draws it in the popover.

use clanky::config::AgentTool;
use clanky_core::layout::Axis;
use clanky_state::{
    agent_picker::{AgentChoice, AgentPicker},
    keys::{Action, Chord, Key, action_for},
    list_selection::ListKey,
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Styled, Window, div, point, px,
};
use serde_json::json;

use super::{MainWindow, SCREEN, TOOLS_TOGGLE_ID, chord_of};
use crate::{
    control_look::paint,
    controls::{Control, Group, INTERFACE_FONT_FAMILY, Kind},
    icons::Icon,
    popover::{Opener, Placement},
    tokens as t,
};

/// The picker's screen name in the control record.
const SCREEN_NAME: &str = "agent-picker";
/// Control ids of the tool rows, one per shown tool in order.
const ROW_IDS: u32 = 1;
const ROWS: Group = Group::vertical("agent-picker-tools");
/// What the empty query line says.
const PROMPT: &str = "Start an agent";

/// What the window keeps for the picker: the config's agent tools and their access, the open picker's list, and the
/// tool a new terminal is being started for.
pub(super) struct AgentTools {
    tools: Vec<AgentTool>,
    /// Whether a pane started for an agent gets agent access (read + run in that pane only).
    access: bool,
    picker: AgentPicker,
    /// The focused pane's folder when the picker opened, as the footer shows it.
    folder: String,
    /// Set while `new_tab` or `split` starts the terminal for an agent.
    pending: Option<AgentTool>,
    /// The terminal `spawn_terminal` opened for that agent.
    opened: Option<String>,
}

impl AgentTools {
    pub(super) fn new(tools: Vec<AgentTool>, access: bool) -> Self {
        AgentTools {
            tools,
            access,
            picker: AgentPicker::default(),
            folder: String::new(),
            pending: None,
            opened: None,
        }
    }
}

impl MainWindow {
    /// Ctrl+Shift+G opens the picker, or closes it when it is open. True when the key was that one.
    pub(super) fn agent_picker_key(
        &mut self,
        chord: Chord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if action_for(chord) != Some(Action::ToggleAgentPicker) {
            return false;
        }
        if self.popover.screen() == Some(SCREEN_NAME) {
            self.close_popover(window, cx);
            return true;
        }
        let pane = self.focused_pane(window, cx);
        let opener = Opener::focused_control(&self.kit, window)
            .or_else(|| {
                Opener::control(&self.kit, SCREEN, TOOLS_TOGGLE_ID).map(|opener| Opener {
                    pane: pane.clone(),
                    control: None,
                    ..opener
                })
            })
            .or_else(|| pane.map(|pane| Opener::point(point(px(0.0), px(0.0)), pane)));
        if let Some(opener) = opener {
            self.open_agent_picker(opener, window, cx);
        }
        true
    }

    /// The Agent segment of pane `pane`'s Shell | Agent switch, control `segment`, opens the picker beside it.
    pub(super) fn agent_segment_chosen(
        &mut self,
        segment: u32,
        pane: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(opener) = Opener::control(&self.kit, SCREEN, segment) {
            self.open_agent_picker(
                Opener {
                    pane: Some(pane),
                    ..opener
                },
                window,
                cx,
            );
        }
    }

    /// Opens the picker with every configured tool, the first installed one selected; nothing with --post-ai.
    fn open_agent_picker(&mut self, opener: Opener, window: &mut Window, cx: &mut Context<Self>) {
        if self.post_ai {
            return;
        }
        let path = std::env::var_os("PATH");
        let choices = self
            .agent_tools
            .tools
            .iter()
            .map(|tool| AgentChoice {
                name: tool.name.clone(),
                command: tool.command.clone(),
                installed: clanky::agent_launch::installed(&tool.command, path.as_deref()),
            })
            .collect();
        self.agent_tools.picker = AgentPicker::new(choices);
        self.agent_tools.folder = opener
            .pane
            .as_deref()
            .and_then(|pane| self.pane_folder(pane))
            .unwrap_or_else(|| "the focused pane's folder".into());
        self.popover.open(
            SCREEN_NAME,
            opener,
            Placement::Centred {
                top: t::TITLE_BAR_HEIGHT + t::SPACE_12,
            },
            window,
            cx,
        );
        self.restart_agent_list();
        cx.notify();
    }

    /// The pane's folder as the picker's footer shows it, home written as ~.
    fn pane_folder(&self, pane: &str) -> Option<String> {
        let reply = self
            .client
            .exchange(3, "terminal_cwd", &json!({"tab_id": pane}))
            .ok()?;
        let home =
            clanky_core::platform::home_dir().and_then(|home| home.to_str().map(str::to_owned));
        Some(clanky_state::blocks::home_short(
            reply["cwd"].as_str()?,
            home.as_deref(),
        ))
    }

    /// After the list changed: its rows counted again, the first installed one selected.
    fn restart_agent_list(&mut self) {
        self.popover
            .restart_list(self.agent_tools.picker.shown().len());
        if let Some(row) = self.agent_tools.picker.first_row() {
            self.popover.select_row(row);
        }
    }

    /// A key while the picker itself has the keyboard (no row has it): the list's keys, Left and Right, typing and
    /// Backspace. True when the picker took it.
    fn agent_picker_typed(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(chord) = chord_of(&event.keystroke) else {
            return false;
        };
        let modified = chord.mods.ctrl || chord.mods.alt;
        match self.popover.list_key(chord) {
            ListKey::Moved => return true,
            ListKey::Pick(row) => {
                self.start_agent_row(row, window, cx);
                return true;
            }
            ListKey::Ignored => {}
        }
        match chord.key {
            Key::Left | Key::Right if !modified => self.agent_tools.picker.toggle_new_tab(),
            Key::Backspace if !modified => {
                if !self.agent_tools.picker.backspace() {
                    return true;
                }
                self.restart_agent_list();
            }
            _ => {
                let typed = event.keystroke.key_char.as_deref().filter(|_| !modified);
                let Some(text) = typed.filter(|text| !text.is_empty()) else {
                    return false;
                };
                self.agent_tools.picker.type_text(text);
                self.restart_agent_list();
            }
        }
        true
    }

    /// Starts the tool on shown row `row`, if it is installed, and closes the picker.
    fn start_agent_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(choice) = self.agent_tools.picker.pick(row) else {
            return;
        };
        let tool = AgentTool {
            name: choice.name.clone(),
            command: choice.command.clone(),
        };
        let new_tab = self.agent_tools.picker.new_tab();
        self.close_popover(window, cx);
        self.act(window, cx, |this, window, cx| {
            this.start_agent(tool, new_tab, window, cx)
        });
    }

    /// Starts an agent tool in a split to the right of the focused pane, or in a new tab.
    fn start_agent(
        &mut self,
        tool: AgentTool,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let path = std::env::var_os("PATH");
        if !clanky::agent_launch::installed(&tool.command, path.as_deref()) {
            return Err(format!(
                "{} is not installed: {} is not on PATH",
                tool.name,
                clanky::agent_launch::program(&tool.command)
            ));
        }
        self.agent_tools.pending = Some(tool.clone());
        let result = if new_tab {
            self.new_tab(window, cx)
        } else {
            self.split(Axis::Columns, window, cx)
        };
        self.agent_tools.pending = None;
        if let Some(id) = self.agent_tools.opened.take() {
            self.type_agent_command(id, tool, window, cx);
        }
        result
    }

    /// Types the tool's command into its new terminal once the shell has drawn its prompt, off the window's thread,
    /// so the window draws the new pane meanwhile. A failure closes that terminal and shows in the status line.
    fn type_agent_command(
        &mut self,
        id: String,
        tool: AgentTool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let client = self.client.clone();
        cx.spawn_in(window, async move |this, cx| {
            let typed = cx
                .background_executor()
                .spawn(async move { clanky::agent_launch::type_command(&client, &id, &tool) })
                .await;
            if let Err(message) = typed {
                // A window closed meanwhile has no status line to show the failure in.
                let _ = this.update(cx, |this, cx| {
                    this.notice = Some(message);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// A new terminal for `new_tab` or `split`: for the tool being started, when one is (its command is typed once the
    /// pane is in the workspace), else the shell.
    pub(super) fn spawn_terminal(
        &mut self,
        focused: Option<&str>,
        rows: usize,
        cols: usize,
    ) -> Result<String, String> {
        let Some(tool) = &self.agent_tools.pending else {
            return clanky::window_tools::new_terminal(&self.client, focused, rows, cols);
        };
        let id = clanky::agent_launch::open(
            &self.client,
            tool,
            focused,
            None,
            self.agent_tools.access,
            rows,
            cols,
        )?;
        self.agent_tools.opened = Some(id.clone());
        Ok(id)
    }

    /// The picker: the query line, the tools, and a footer saying where the agent opens, in which folder and with
    /// what access.
    pub(super) fn agent_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let colors = self.colors.clone();
        let hair = t::hairline(window.scale_factor());
        let picker = &self.agent_tools.picker;
        let (query, ink) = if picker.query().is_empty() {
            (PROMPT.to_owned(), colors.muted_foreground)
        } else {
            (picker.query().to_owned(), colors.foreground)
        };
        // The card's top edge and the line under the query are inside the input's height.
        let input = div()
            .flex()
            .items_center()
            .h(px(t::PALETTE_INPUT_HEIGHT - hair))
            .px(px(t::SPACE_12))
            .border_b(px(hair))
            .border_color(paint(colors.border, 1.0))
            .text_size(px(t::TEXT_TITLE))
            .font_weight(FontWeight(f32::from(t::WEIGHT_MEDIUM)))
            .text_color(paint(ink, 1.0))
            .child(query);
        let mut rows = div().flex().flex_col().p(px(t::POPOVER_PADDING));
        let selected = self
            .popover
            .selected_row()
            .filter(|_| self.popover.holds_keyboard(window));
        let shown: Vec<AgentChoice> = picker.shown().into_iter().cloned().collect();
        if shown.is_empty() {
            let empty = if self.agent_tools.tools.is_empty() {
                "No agent tools in the config (\"agents\")."
            } else {
                "No agent tool matches."
            };
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(t::PALETTE_ROW_HEIGHT))
                    .px(px(t::SPACE_8))
                    .text_size(px(t::TEXT_TITLE))
                    .text_color(paint(colors.muted_foreground, 1.0))
                    .child(empty),
            );
        }
        for (index, choice) in shown.iter().enumerate() {
            let this = cx.entity();
            let detail = if choice.installed {
                choice.command.clone()
            } else {
                "not installed".to_owned()
            };
            let row = Control::new(
                Kind::PaletteRow,
                ROW_IDS + index as u32,
                choice.name.clone(),
            )
            .group(ROWS)
            .icon(Icon::Agent)
            .keys(detail)
            .enabled(choice.installed)
            .checked(selected == Some(index))
            .on_click(move |window, cx| {
                this.update(cx, |this, cx| this.start_agent_row(index, window, cx));
            });
            // The pointer selects the installed tool under it, as the keyboard does, so one row is lit at a time.
            let hover = cx.entity();
            let installed = choice.installed;
            let row = div()
                .w_full()
                .on_mouse_move(move |_, _, cx| {
                    hover.update(cx, |this, cx| {
                        if installed && this.popover.selected_row() != Some(index) {
                            this.popover.select_row(index);
                            cx.notify();
                        }
                    });
                })
                .child(self.kit.build(SCREEN_NAME, row, window, cx));
            rows = rows.child(row);
        }
        let opens_in = if picker.new_tab() {
            "Opens in a new tab"
        } else {
            "Opens in a split to the right"
        };
        let access = if self.agent_tools.access {
            "read + run, this pane only"
        } else {
            "off"
        };
        let footer = div()
            .flex()
            .items_center()
            .h(px(t::STATUS_HEIGHT))
            .px(px(t::SPACE_12))
            .border_t(px(hair))
            .border_color(paint(colors.border, 1.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_size(px(t::TEXT_STATUS))
            .text_color(paint(colors.muted_foreground, 1.0))
            .child(format!(
                "{opens_in} (\u{2190} \u{2192}) \u{b7} {} \u{b7} Agent access: {access}",
                self.agent_tools.folder
            ));
        let content = div()
            .flex()
            .flex_col()
            .w(px(t::PALETTE_WIDTH))
            .font_family(INTERFACE_FONT_FAMILY)
            .child(input)
            .child(rows)
            .child(footer);
        let keys = cx.entity();
        let closer = cx.entity();
        self.popover.element(
            &self.kit,
            &colors,
            content.into_any_element(),
            move |event, window, cx: &mut App| {
                keys.update(cx, |this, cx| {
                    if this.popover.holds_keyboard(window)
                        && this.agent_picker_typed(event, window, cx)
                    {
                        cx.stop_propagation();
                        cx.notify();
                    }
                });
            },
            move |window, cx| closer.update(cx, |this, cx| this.close_popover(window, cx)),
            window,
        )
    }
}
