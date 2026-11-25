//! The Settings pages, one module each, in the switcher's order. Each exports `rows`, which lists the page's
//! settings for the config now; the Settings window lays them out and writes their changes.

pub mod agents;
pub mod appearance;
pub mod editor;
pub mod general;
pub mod history;
pub mod keys;
pub mod terminal;
