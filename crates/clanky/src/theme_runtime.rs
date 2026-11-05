//! Timer-owned theme state, independent of Settings controls and scene layout.
use crate::{config::Config, theme_store::ThemeStore, themes::Theme};

pub struct ThemeRuntime {
    pub store: ThemeStore,
    active: Option<Theme>,
    error: Option<String>,
}
impl ThemeRuntime {
    pub fn open() -> Result<Self, String> {
        Self::from_store(ThemeStore::open_default()?)
    }
    pub fn from_store(store: ThemeStore) -> Result<Self, String> {
        Ok(Self {
            store,
            active: None,
            error: None,
        })
    }
    pub fn active(&self) -> Option<&Theme> {
        self.active.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Reload on the existing 500 ms settings timer, not every frame.
    /// Keep the last-good selected theme on errors; return whether anything visible changed.
    pub fn poll(&mut self, config: &Config, system_light: bool) -> bool {
        let old_error = self.error.clone();
        let reloaded = self.store.reload();
        let mut changed = reloaded.as_ref().copied().unwrap_or(false);
        let result = reloaded.and_then(|_| config.resolved_document(&self.store, system_light));
        match result {
            Ok(theme) => {
                changed |= self.active.as_ref() != Some(&theme);
                self.active = Some(theme);
                self.error = self
                    .store
                    .issues()
                    .first()
                    .map(|i| format!("{}: {}", i.path.display(), i.message));
            }
            Err(error) => self.error = Some(error),
        }
        changed || self.error != old_error
    }
}
