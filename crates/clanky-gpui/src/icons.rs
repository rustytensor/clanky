//! The interface icons: Lucide (lucide-static 1.70.0, ISC licence in assets/icons/LICENSE-lucide.txt), drawn on
//! Lucide's 24-unit grid with the rulebook's stroke of 1.75 and round ends, at 16 px in rows and 14 px in bars.
//! gpui-ce draws an SVG as a mask in the element's text colour, so the stroke colour written here does not matter.

/// An interface icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// New tab.
    Plus,
    /// Close a tab or a dialog.
    Close,
    /// A closed folder row.
    ChevronRight,
    /// An open folder row.
    ChevronDown,
    /// A folder row.
    Folder,
    /// A file row.
    File,
    /// The tools panel's toggle.
    Sidebar,
    /// Split the pane right.
    SplitRight,
    /// Split the pane down.
    SplitDown,
    /// Find.
    Search,
    /// Agents.
    Agent,
    /// Settings.
    Settings,
    /// Copy.
    Copy,
    /// Done, or a checked menu item.
    Check,
    /// Masked text.
    Hidden,
    /// Recent.
    Clock,
    /// A terminal pane.
    Terminal,
}

impl Icon {
    /// The icon as SVG source.
    pub fn svg(self) -> &'static [u8] {
        match self {
            Icon::Plus => PLUS,
            Icon::Close => CLOSE,
            Icon::ChevronRight => CHEVRON_RIGHT,
            Icon::ChevronDown => CHEVRON_DOWN,
            Icon::Folder => FOLDER,
            Icon::File => FILE,
            Icon::Sidebar => SIDEBAR,
            Icon::SplitRight => SPLIT_RIGHT,
            Icon::SplitDown => SPLIT_DOWN,
            Icon::Search => SEARCH,
            Icon::Agent => AGENT,
            Icon::Settings => SETTINGS,
            Icon::Copy => COPY,
            Icon::Check => CHECK,
            Icon::Hidden => HIDDEN,
            Icon::Clock => CLOCK,
            Icon::Terminal => TERMINAL,
        }
    }
}

/// Lucide `plus`.
const PLUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14" /><path d="M12 5v14" /></svg>"##;
/// Lucide `x`.
const CLOSE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18" /><path d="m6 6 12 12" /></svg>"##;
/// Lucide `chevron-right`.
const CHEVRON_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6" /></svg>"##;
/// Lucide `chevron-down`.
const CHEVRON_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6" /></svg>"##;
/// Lucide `folder`.
const FOLDER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" /></svg>"##;
/// Lucide `file`.
const FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z" /><path d="M14 2v5a1 1 0 0 0 1 1h5" /></svg>"##;
/// Lucide `panel-left`.
const SIDEBAR: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M9 3v18" /></svg>"##;
/// Lucide `columns-2`.
const SPLIT_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M12 3v18" /></svg>"##;
/// Lucide `rows-2`.
const SPLIT_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M3 12h18" /></svg>"##;
/// Lucide `search`.
const SEARCH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m21 21-4.34-4.34" /><circle cx="11" cy="11" r="8" /></svg>"##;
/// Lucide `bot`.
const AGENT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" /></svg>"##;
/// Lucide `settings`.
const SETTINGS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915" /><circle cx="12" cy="12" r="3" /></svg>"##;
/// Lucide `copy`.
const COPY: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2" /><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" /></svg>"##;
/// Lucide `check`.
const CHECK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5" /></svg>"##;
/// Lucide `eye-off`.
const HIDDEN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" /><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" /><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" /><path d="m2 2 20 20" /></svg>"##;
/// Lucide `clock`.
const CLOCK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10" /><path d="M12 6v6l4 2" /></svg>"##;
/// Lucide `square-terminal`.
const TERMINAL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="m7 11 2-2-2-2" /><path d="M11 13h4" /><rect width="18" height="18" x="3" y="3" rx="2" ry="2" /></svg>"##;
