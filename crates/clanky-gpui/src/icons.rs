//! The interface icons: one stroke family, drawn on a 16-unit grid with the rulebook's 1.75 stroke and round ends, so
//! they read the same at the row size (16 px) and the bar size (14 px). gpui-ce draws an SVG as a mask in the
//! element's text colour, so the stroke colour written here does not matter.
//!
//! Each shape is symmetric about the box's centre on the axis it is centred on, so its ink centres where its box does.

/// An interface icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Plus,
    Close,
    ChevronRight,
    ChevronDown,
    Folder,
    File,
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
        }
    }
}

const PLUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v10M3 8h10"/></svg>"##;
const CLOSE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4l8 8M12 4l-8 8"/></svg>"##;
const CHEVRON_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M6.5 4l4 4-4 4"/></svg>"##;
const CHEVRON_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6.5l4 4 4-4"/></svg>"##;
const FOLDER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4.5c0-.8.7-1.5 1.5-1.5h2.8l1.5 1.5h4.7c.8 0 1.5.7 1.5 1.5v5.5c0 .8-.7 1.5-1.5 1.5h-9c-.8 0-1.5-.7-1.5-1.5z"/></svg>"##;
const FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M9 2H5c-.8 0-1.5.7-1.5 1.5v9c0 .8.7 1.5 1.5 1.5h6c.8 0 1.5-.7 1.5-1.5V5.5zM9 2v3.5h3.5"/></svg>"##;
