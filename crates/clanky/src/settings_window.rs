//! Window integration for the shared settings file and form.
use super::*;
use clanky::calm::settings_panel::{Effect,ThemeAction};

impl App {
    pub(super) fn settings_open(&self) -> bool { self.settings.as_ref().is_some_and(|panel|panel.open) }
    pub(super) fn toggle_settings(&mut self) -> Result<(),String> {
        let Some(panel)=&mut self.settings else{self.status=self.settings_error.clone();return Ok(());};
        panel.toggle();
        self.clear_card_focus();self.studio.drag=None;self.files.dragging=None;self.compose_dragging=false;self.reset_ime();
        Ok(())
    }
    pub(super) fn settings_effect(&mut self,effect:Effect) -> Result<(),String> {
        let result=(|| -> Result<(),String> { match effect {
            Effect::None=>Ok(()),
            Effect::Changed=>self.apply_settings(),
            Effect::Theme(action)=>self.theme_action(action),
            Effect::Open(path)=>{
                if !path.exists() {
                    if let Some(panel)=&self.settings {panel.store.export(&path)?;}
                }
                clanky::platform::open_settings_file(&path)
            },
            Effect::Access(id)=>{
                let allowed=self.engine.agent_permission(&id).map_err(|e|e.message)?;
                self.engine.set_agent_permission(&id,!allowed).map_err(|e|e.message)
            }
        }})();
        if let Err(error)=result {
            self.status=Some(error.clone());
            if let Some(panel)=&mut self.settings {panel.error=Some(error);}
        }
        if !self.settings_open() {self.reset_ime();}
        Ok(())
    }
    fn theme_action(&mut self,action:ThemeAction)->Result<(),String>{
        let (selected,clear_overrides)={
            let runtime=self.theme_runtime.as_mut().ok_or("Theme storage is unavailable")?;
            match action{
                ThemeAction::Save{slug,theme}=>(Some(runtime.store.save_new(&slug,&theme)?),true),
                ThemeAction::Import{path,slug}=>(Some(runtime.store.import(&path,&slug)?),false),
                ThemeAction::Export{id,path}=>{runtime.store.export(&id,&path)?;(None,false)},
            }
        };
        if let Some(id)=selected{
            let appearance=self.theme_runtime.as_ref().unwrap().store.get(&id)?.appearance;
            let panel=self.settings.as_mut().ok_or("Settings is unavailable")?;
            panel.store.set(if appearance==clanky::themes::Appearance::Light{"theme_light"}else{"theme_dark"},json!(id))?;
            if clear_overrides{
                let retained:serde_json::Map<String,Value>=panel.store.config().colors.iter().filter(|(token,_)|!clanky::themes::COLOR_TOKENS.contains(&token.as_str())).map(|(token,color)|(token.clone(),json!(format!("#{:02x}{:02x}{:02x}",color[0],color[1],color[2])))).collect();
                panel.store.set("colors",Value::Object(retained))?;
            }
        }
        if let Some(panel)=&mut self.settings{panel.theme_complete();}
        self.apply_settings()
    }
    fn apply_settings(&mut self)->Result<(),String> {
        let Some(panel)=&self.settings else{return Ok(());};
        let mut config=panel.store.config().clone();
        self.startup_flags.apply(&mut config);
        self.frame_clock.invalidate();
        let font_changed=config.font_size_pt!=self.config.font_size_pt;
        let geometry_changed=config.columns!=self.config.columns || config.rows!=self.config.rows;
        if config.font_family!=self.config.font_family || config.ui_font_family!=self.config.ui_font_family {
            if let Some(renderer)=&mut self.renderer {
                renderer.set_font_families(&config.font_family,&config.ui_font_family)?;
            }
            self.invalidate_scene_fonts();
        }
        if config.compose_dictionary!=self.config.compose_dictionary || config.compose_dictionary_dir!=self.config.compose_dictionary_dir
            || config.compose_words!=self.config.compose_words || config.compose_send_enter!=self.config.compose_send_enter {
            self.compose.configure(&config);
        }
        if config.osc52_write!=self.config.osc52_write || config.osc52_read!=self.config.osc52_read {
            self.clipboard_bridge=ClipboardBridge::new(self.engine.clone(),config.osc52_write,config.osc52_read);
            self.clipboard_bridge.set_tabs(self.groups.iter().flat_map(|group|group.layout.leaves()).collect());
        }
        if config.rails!=self.config.rails {self.panels.visible=config.rails;}
        if font_changed {self.zoom=Zoom::new(config.font_size_pt);}
        self.studio.theme=config.resolved_theme(self.window.as_ref().is_some_and(|window|clanky::platform::system_light(window)));
        self.config=config;
        if let Some(runtime)=&mut self.theme_runtime{runtime.poll(&self.config,self.window.as_ref().is_some_and(|window|clanky::platform::system_light(window)));}
        if geometry_changed {
            if let Some(window)=&self.window {
                let base=self.zoom.metrics(1.0)?;
                let _=window.request_inner_size(LogicalSize::new(self.config.columns as f64*base.cell_width as f64,
                    (self.config.rows+1) as f64*base.cell_height as f64));
            }
        }
        if font_changed {self.metrics_changed()?;} else {self.resize_panes()?;}
        Ok(())
    }
    pub(super) fn poll_settings(&mut self) {
        let result=self.settings.as_mut().map(|panel|panel.store.reload());
        match result {
            Some(Ok(true))=>{
                self.frame_clock.invalidate();
                if let Some(panel)=&mut self.settings {
                    if panel.error.is_some() && self.status==panel.error {self.status=None;}
                    panel.error=None;
                }
                if let Err(error)=self.apply_settings(){self.status=Some(error.clone());if let Some(panel)=&mut self.settings{panel.error=Some(error);}}},
            Some(Err(error))=>{self.frame_clock.invalidate();self.status=Some(error.clone());if let Some(panel)=&mut self.settings{panel.error=Some(error);}},
            _=>(),
        }
    }
}
