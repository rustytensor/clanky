//! clanky-gpui: one window with one terminal pane, drawn with gpui-ce, as a client of Clanky's session server.
//!
//! ```text
//!   clanky-gpui [--config FILE] [--size WxH] [--exec "CMD"] [--socket PATH] [--post-ai]
//!   clanky-gpui --server --socket PATH [--config FILE] [--post-ai]
//! ```
//!
//! The window starts the session server from its own executable when none is running, exactly as today's
//! window does. --size is the window's size in logical pixels; --exec runs CMD with bash -c instead of the
//! user's shell, and the pane stays open after it ends.

// Pixel arithmetic converts between float positions and cell counts all through this crate. The values are
// bounded (window sides at most 16384 px, grids at most 1000 x 1000 cells), so no cast here can wrap or lose
// a whole pixel.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]

mod blocks_view;
mod cells;
// Item by item, the window chrome uses the rest of the control set and the icons.
#[allow(dead_code)]
mod control_look;
mod control_record;
#[allow(dead_code)]
mod controls;
mod editor_pane;
mod glyphs;
mod grid_element;
#[allow(dead_code)]
mod icons;
mod keys;
mod link;
mod main_window;
mod palette;
mod pane;
// The floating screens use the rest of the popover; until each is in, the test popover uses some of it.
#[allow(dead_code)]
mod popover;
mod request_log;
// The controls and the window chrome use the rest of the table.
#[allow(dead_code)]
mod tokens;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clanky::config::{Config, StartupOverrides};
use clanky_api::Client;
use clanky_state::cursor_blink::Blink;
use gpui::{
    App, AppContext, Bounds, TextRenderingMode, TitlebarOptions, WindowBounds, WindowOptions,
    point, px, size,
};
use serde_json::{Value, json};

use crate::{
    control_record::ControlRecord,
    controls::Kit,
    grid_element::{PANE_PADDING, TextSettings},
    main_window::{MainWindow, WindowSetup, interface_theme, shared_trace},
    palette::Palette,
};

const TITLE: &str = "clanky-gpui";
/// The window's application id: xdg_toplevel app_id on Wayland, WM_CLASS on X11. It matches the desktop file's
/// StartupWMClass and the icon name, so a panel or taskbar finds the entry and the icon for the window.
const APP_ID: &str = "clanky";
const DEFAULT_SIZE: (f32, f32) = (1240.0, 760.0);

/// The bundled fonts (SIL Open Font License 1.1, assets/fonts/OFL-JetBrainsMono.txt, OFL-Geist.txt and OFL-Inter.txt):
/// the terminal fonts a config may name, and Inter for the interface, so neither falls back to whatever the system
/// has.
const FONTS: [&[u8]; 10] = [
    include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
];

struct Arguments {
    server: bool,
    socket: Option<PathBuf>,
    config: Option<PathBuf>,
    size: (f32, f32),
    exec: Option<String>,
    overrides: StartupOverrides,
}

fn parse_size(text: &str) -> Result<(f32, f32), String> {
    let (w, h) = text
        .split_once('x')
        .ok_or_else(|| format!("--size {text}: expected WIDTHxHEIGHT, such as 1240x760"))?;
    let number = |part: &str| {
        part.parse::<u32>()
            .ok()
            .filter(|n| (200..=16384).contains(n))
            .ok_or_else(|| {
                format!("--size {text}: each side must be a whole number from 200 to 16384")
            })
    };
    Ok((number(w)? as f32, number(h)? as f32))
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut args = Arguments {
        server: false,
        socket: None,
        config: None,
        size: DEFAULT_SIZE,
        exec: None,
        overrides: StartupOverrides::default(),
    };
    let mut items = std::env::args().skip(1);
    while let Some(item) = items.next() {
        let mut value = |name: &str| items.next().ok_or_else(|| format!("{name} needs a value"));
        match item.as_str() {
            "--server" => args.server = true,
            "--socket" => args.socket = Some(PathBuf::from(value("--socket")?)),
            "--config" => args.config = Some(PathBuf::from(value("--config")?)),
            "--size" => args.size = parse_size(&value("--size")?)?,
            "--exec" => args.exec = Some(value("--exec")?),
            "--version" => {
                println!("{TITLE} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            flag if args.overrides.parse_flag(flag) => {}
            other => {
                return Err(format!(
                    "unknown argument {other}; see the usage at the top of main.rs"
                ));
            }
        }
    }
    Ok(args)
}

/// The pane's tab: with --exec a new tab running `bash -c CMD`; otherwise the server's first tab when it already
/// has one (as today's window shows the session it joins), or a new tab running the server's default shell. The
/// flag says the window opened that shell tab itself (a fresh session, no --exec): such a window is a new window,
/// which `new_window_panes` may split.
fn open_tab(
    client: &Client,
    exec: Option<&str>,
    rows: usize,
    cols: usize,
) -> Result<(String, bool), String> {
    let Some(command) = exec else {
        let existing = client
            .tab_ids()
            .map_err(|e| format!("listing the session's tabs: {}", e.message))?;
        return match existing.into_iter().next() {
            Some(tab) => Ok((tab, false)),
            None => {
                clanky::window_tools::new_terminal(client, None, rows, cols).map(|tab| (tab, true))
            }
        };
    };
    let defaults = client
        .exchange(3, "shell_defaults", &json!({}))
        .map_err(|e| format!("asking for the shell defaults: {}", e.message))?;
    let home = defaults
        .get("home")
        .and_then(Value::as_str)
        .ok_or("the server reported no home directory")?
        .to_owned();
    let tab = client
        .request(
            "open_tab",
            &json!({"rows": rows, "cols": cols, "allow_agent": false}),
            true,
        )
        .map_err(|e| format!("opening the pane: {}", e.message))?;
    let id = tab["tab_id"]
        .as_str()
        .ok_or("the server sent no tab id")?
        .to_owned();
    client
        .request(
            "run",
            &json!({"tab_id": id, "argv": ["bash", "-c", command], "cwd": home}),
            true,
        )
        .map_err(|e| format!("running --exec: {}", e.message))?;
    // The window shows the session's active tab: the command's own tab is the one asked for.
    let mut workspace = client
        .workspace_get()
        .map_err(|e| format!("reading the workspace: {}", e.message))?;
    if let Some(index) = workspace
        .groups
        .iter()
        .position(|group| group.layout.leaves().contains(&id))
    {
        workspace.active = index;
        client
            .workspace_set(&workspace)
            .map_err(|e| format!("showing the --exec tab: {}", e.message))?;
    }
    Ok((id, false))
}

/// The settings the settings file sets itself, by name, so a value it leaves out takes clanky-gpui's own default
/// rather than the old window's. An unreadable or broken file sets nothing: `Config::load` has already reported it
/// and used the defaults. A bad value of a setting that falls back on its own (`tab_groups`, `new_window_panes`)
/// leaves the others set, as `Config::load` does.
fn settings_names(explicit: Option<&Path>) -> Vec<String> {
    let Some(path) = explicit
        .map(Path::to_owned)
        .or_else(clanky_state::platform::settings_path)
    else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let document = if clanky_state::config::is_json_path(&path) {
        serde_json::from_str::<serde_json::Value>(&text).ok()
    } else {
        clanky_state::config_xml::load_soft(&text)
            .ok()
            .map(|(_, document, _)| document)
    };
    document
        .and_then(|document| {
            document
                .as_object()
                .map(|object| object.keys().cloned().collect())
        })
        .unwrap_or_default()
}

/// The terminal's text: the config's where it sets it, else the design rulebook's terminal font (`JetBrains Mono` 16,
/// line height 1.2).
fn text_settings(config: &Config, is_set: &dyn Fn(&str) -> bool) -> TextSettings {
    let family = if is_set("font_family") {
        config.font_family.clone()
    } else {
        tokens::TERMINAL_FONT_FAMILY.to_owned()
    };
    let size = if is_set("font_size") || is_set("font_size_pt") {
        config.font_size
    } else {
        tokens::TERMINAL_FONT_SIZE
    };
    let line_height = if is_set("cell_height_scale") && config.cell_height_scale > 0.0 {
        config.cell_height_scale
    } else {
        tokens::TERMINAL_LINE_HEIGHT
    };
    TextSettings {
        family: family.into(),
        size,
        line_height,
    }
}

fn start_window(args: Arguments, cx: &mut App) -> Result<(), String> {
    let socket = match args.socket {
        Some(path) => path,
        None => clanky_core::platform::default_socket()
            .map_err(|e| format!("finding the session socket: {e}"))?,
    };
    let config = Config::load(args.config.as_deref())?;
    // With restore_layout on, a fresh session reopens the saved tabs and splits, and the window keeps the file current.
    let layout_file = clanky::layout_store::default_path().filter(|_| config.restore_layout);
    let client = clanky::session::prepare_window_configured(
        &socket,
        layout_file.as_deref(),
        args.config.as_deref(),
        args.overrides,
    )?;
    // Post-AI comes from the config or --post-ai, and the session server has the last word, as behind today's window:
    // a server already in Post-AI keeps every window it serves there.
    let policy = client
        .exchange(
            4,
            "post_ai",
            &json!({"enabled": config.post_ai || args.overrides.post_ai, "force": args.overrides.post_ai}),
        )
        .map_err(|e| format!("asking the session server for its Post-AI mode: {}", e.message))?;
    let post_ai = policy["post_ai"]
        .as_bool()
        .ok_or("the session server's Post-AI reply has no post_ai flag")?;
    // Grayscale anti-aliasing, as today's window draws text: sub-pixel colour fringes would tint every glyph edge.
    cx.set_text_rendering_mode(TextRenderingMode::Grayscale);
    cx.text_system()
        .add_fonts(FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect())
        .map_err(|e| format!("loading the bundled fonts: {e}"))?;
    let named = settings_names(args.config.as_deref());
    let text = text_settings(&config, &|name| named.iter().any(|set| set == name));
    let line_height = text.line_height;
    // A first guess at the grid in logical pixels; the pane sends the exact size once the window is laid out.
    let cols = (((args.size.0 - 2.0 * PANE_PADDING) / (text.size * 0.6)) as usize).clamp(2, 1000);
    let rows =
        (((args.size.1 - 2.0 * PANE_PADDING) / (text.size * line_height)) as usize).clamp(1, 1000);
    // The workspace needs a tab to show: the session's own when it has one, else a new one.
    let (tab, opened) = open_tab(&client, args.exec.as_deref(), rows, cols)?;
    let trace = clanky::frame_trace::FrameTrace::from_env()
        .map_err(|e| format!("opening CLANKY_FRAME_TRACE: {e}"))?;
    let record = ControlRecord::from_environment().map_err(|e| e.to_string())?;
    let theme = interface_theme(&config, &|name| named.iter().any(|set| set == name))?;
    let colors = theme.colors.clone();
    let setup = WindowSetup {
        client,
        palette: Palette::from_config(&config, &theme),
        text,
        trace: shared_trace(trace),
        blink: Blink::from_config(&config, clanky_state::platform::gnome_cursor_blink()),
        kit: Kit::new(record, colors.clone(), config.animations),
        colors,
        post_ai,
        // A pane started with --exec stays open after its command ends; a shell's pane closes when its shell exits.
        exec_tab: args.exec.is_some().then_some(tab),
        tools_open: config.tools_panel,
        tools: config.tools.clone(),
        agent_programs: config.agents.clone(),
        agent_pane_access: config.agent_pane_access,
        command_input: args.overrides.editor_mode(&config),
        // Today's new window: one tab split in two, one pane above the other, unless the config says one pane.
        opens_with: if opened { config.new_window_panes } else { 1 },
        layout_file: layout_file.map(|file| (file, socket.clone())),
        syntax: editor_pane::syntax_colors(&theme),
        tab_style: config.tab_style,
        tab_groups: config.tab_groups,
        config_file: args
            .config
            .clone()
            .or_else(clanky_state::platform::settings_path),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(args.size.0), px(args.size.1)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(TITLE.into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some(APP_ID.into()),
        ..WindowOptions::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| MainWindow::new(setup, window, cx))
    })
    .map(|_| ())
    .map_err(|e| format!("opening the window: {e}"))
}

fn main() -> ExitCode {
    let args = match parse_arguments() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{TITLE}: {message}");
            return ExitCode::from(2);
        }
    };
    if args.server {
        let Some(socket) = &args.socket else {
            eprintln!("{TITLE}: --server needs --socket PATH");
            return ExitCode::from(2);
        };
        return match clanky::session::run_server_configured(
            socket,
            args.config.as_deref(),
            args.overrides,
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{TITLE}: {message}");
                ExitCode::FAILURE
            }
        };
    }
    gpui_platform::application().run(move |cx| {
        if let Err(message) = start_window(args, cx) {
            eprintln!("{TITLE}: {message}");
            // No window was opened, and quitting from inside the start callback does not end gpui-ce's event loop
            // on X11 (the process stayed with no window), so it ends here.
            std::process::exit(1);
        }
    });
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_terminal_text_defaults_to_the_rulebooks_font() {
        // The old window's defaults (Geist Mono 11 pt, 1.25) do not apply to a file that sets nothing.
        let config = Config::window_from_json("{}").expect("an empty config is valid");
        let unset = text_settings(&config, &|_| false);
        assert_eq!(
            (unset.family.as_ref(), unset.size, unset.line_height),
            ("JetBrains Mono", 16.0, 1.2)
        );
        let set = Config::window_from_json(
            r#"{"font_family":"IBM Plex Mono","font_size_pt":12,"cell_height_scale":1.3}"#,
        )
        .expect("the config is valid");
        let all = text_settings(&set, &|_| true);
        assert_eq!(
            (all.family.as_ref(), all.size, all.line_height),
            ("IBM Plex Mono", 16.0, 1.3)
        );
    }
}
