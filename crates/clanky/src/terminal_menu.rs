//! The right-click menu in the window: what it is opened over, and what its rows do.
//! Every row uses the code path behind its shortcut or block action.
use super::*;
use clanky::calm::context_menu::{Context, Effect, Item};
use clanky::mouse::{self, Kind, Pos, Selection};

impl App {
    /// Open the menu over a pane at the pointer. The pane has focus already.
    pub(super) fn open_menu(&mut self, id: &str) -> Result<(), String> {
        let layout = self.pixel_layout()?;
        let pane = layout.pane(id).ok_or("That pane was closed")?;
        let editor = self.files.panes.contains_key(id);
        let mut context = Context { editor, post_ai: self.applied_post_ai, ..Context::default() };
        if editor {
            let editor = &self.files.panes[id];
            context.selection = editor.selection().anchor != editor.selection().cursor;
        } else {
            context.access = self.studio.permissions.get(id).copied().unwrap_or(false);
            context.selection = self.term_mouse.selection.as_ref()
                .is_some_and(|held| held.pane == id && self.studio.snapshots.get(id).is_some_and(|s| !held.sel.text(s).is_empty()));
            if let (Some((col, row)), Some(snapshot)) = (studio::point(pane, self.mouse, layout.cell_px, false), self.studio.snapshots.get(id)) {
                context.link = window_tools::url_at(snapshot, row, col).map(|hit| hit.url).filter(|url| window_tools::valid_url(url));
                if let Some(view) = self.studio.blocks.get(id) {
                    context.block = studio::matched_blocks(snapshot, view).into_iter()
                        .find(|card| !card.running && card.first_row <= row && row <= card.last_row).map(|card| card.id);
                }
            }
        }
        self.context_menu.show(id.to_owned(), context, self.mouse);
        self.reset_ime();
        self.frame_clock.invalidate();
        Ok(())
    }

    pub(super) fn menu_effect(&mut self, effect: Effect) -> Result<(), String> {
        let item = match effect {
            Effect::None => return Ok(()),
            Effect::Close => { self.context_menu.close(); self.frame_clock.invalidate(); return Ok(()); }
            Effect::Run(item) => item,
        };
        let id = self.context_menu.pane.clone();
        let context = std::mem::take(&mut self.context_menu.context);
        self.context_menu.close();
        self.frame_clock.invalidate();
        if self.pixel_layout()?.pane(&id).is_none() { return Err("That pane was closed".into()); }
        if context.editor { self.files.focus_on(id.clone()); } else if self.focused() != Some(id.as_str()) { self.focus_pane(&id)?; }
        if context.editor { return self.editor_menu_item(&id, item); }
        match item {
            Item::OpenLink => self.url_opener.open(context.link.as_deref().unwrap_or_default()).map_err(|_| "Could not open URL".to_owned()),
            Item::CopyLink => self.copy_text(context.link.as_deref().unwrap_or_default()),
            Item::Copy => self.copy_selection(),
            Item::Paste => self.paste_selection(&id, false),
            Item::SelectAll => self.select_all(&id),
            Item::CopyCommand => {
                let block = context.block.ok_or("No command here")?;
                let command = self.studio.blocks.get(&id).and_then(|view| view["blocks"].as_array())
                    .and_then(|blocks| blocks.iter().find(|b| b["block_id"].as_str() == Some(block.as_str())))
                    .and_then(|b| b["command"].as_str()).ok_or("This command is no longer on screen")?.trim().to_owned();
                self.copy_text(&command)
            }
            Item::CopyOutput => self.card_action(&id, context.block.as_deref().ok_or("No command here")?, "copy"),
            Item::RunAgain => self.card_action(&id, context.block.as_deref().ok_or("No command here")?, "rerun"),
            Item::Find => { self.search.open(&self.engine, &id)?; self.reset_ime(); Ok(()) }
            Item::SplitRight => self.split(Axis::Columns),
            Item::SplitBelow => self.split(Axis::Rows),
            Item::StartAgent => { self.open_agent_picker(); Ok(()) }
            Item::AgentAccess => {
                let old = self.engine.agent_permission(&id).map_err(|e| e.message)?;
                self.engine.set_agent_permission(&id, !old).map_err(|e| e.message)?;
                self.studio.permissions.insert(id, !old);
                Ok(())
            }
            Item::Reset | Item::ResetClear => {
                let clear = item == Item::ResetClear;
                self.engine.exchange(3, "reset", &json!({"tab_id": id, "clear": clear})).map_err(|e| e.message)?;
                if clear && self.term_mouse.selection.as_ref().is_some_and(|held| held.pane == id) { self.term_mouse.selection = None; }
                self.terminal_dirty = true;
                Ok(())
            }
            Item::ClosePane => self.close_pane(),
            Item::Cut => Ok(()),
        }
    }

    fn focus_pane(&mut self, id: &str) -> Result<(), String> {
        self.files.focus_on(TERMINALS.into());
        if let Some(group) = self.groups.get_mut(self.active) { group.focus = id.to_owned(); }
        self.save_workspace()?;
        self.reset_ime();
        Ok(())
    }

    /// Select all: the whole screen, to PRIMARY, as a drag over it would.
    fn select_all(&mut self, id: &str) -> Result<(), String> {
        let Some(snapshot) = self.studio.snapshots.get(id) else { return Ok(()); };
        if snapshot.rows == 0 || snapshot.cols == 0 { return Ok(()); }
        let top = mouse::top_line(snapshot);
        let mut sel = Selection::new(Kind::Char, false, Pos { line: top, col: 0, right: false });
        sel.head = Pos { line: top + snapshot.rows as i64 - 1, col: snapshot.cols - 1, right: true };
        self.term_mouse.selection = Some(terminal_mouse::Held { pane: id.to_owned(), sel, dragging: true, lines: Vec::new() });
        self.end_selection()
    }

    fn editor_menu_item(&mut self, id: &str, item: Item) -> Result<(), String> {
        let editor = self.files.panes.get_mut(id).ok_or("That editor was closed")?;
        let selection = editor.selection();
        let (start, end) = (selection.anchor.min(selection.cursor), selection.anchor.max(selection.cursor));
        let selected = editor.text()[start..end].to_owned();
        match item {
            Item::Cut | Item::Copy => {
                if selected.is_empty() { return Ok(()); }
                if item == Item::Cut { editor.command("insert", &json!({"text": ""}))?; }
                self.copy_text(&selected)
            }
            Item::Paste => {
                let bytes = platform::clipboard_read().map_err(|_| "Nothing to paste".to_owned())?;
                editor.insert_text(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())
            }
            Item::SelectAll => editor.command("select", &json!({"anchor": 0, "cursor": editor.text().len()})).map(|_| ()),
            Item::Find => editor.key(&Key::Character("f".into()), None, ModifiersState::CONTROL, false),
            Item::SplitRight => self.split(Axis::Columns),
            Item::SplitBelow => self.split(Axis::Rows),
            Item::ClosePane => self.close_pane(),
            _ => Ok(()),
        }
    }
}

impl App {
    /// A right press: over an editor pane it opens the editor's menu; over a terminal pane
    /// it opens the menu unless the program takes the mouse (Shift+right-click still opens it).
    pub(super) fn secondary_press(&mut self) -> Result<(), String> {
        if self.mouse_overlay() { return Ok(()); }
        let layout = self.pixel_layout()?;
        if let Some(pane) = layout.pane_at(self.mouse.0, self.mouse.1).filter(|p| self.files.panes.contains_key(&p.id) && p.grid_clip.contains(self.mouse.0, self.mouse.1)) {
            let id = pane.id.clone();
            self.files.focus_on(id.clone());
            self.reset_ime();
            return self.open_menu(&id);
        }
        self.terminal_press(3)
    }
}
