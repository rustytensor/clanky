# M11 app scene contract

The app's new code uses `clanky_render::scene`; renderer code is owned elsewhere.
`look.rs` already owns Layout/PixelRect/easing. New `look/scene_builder.rs` exports
all types below through `look` (lead adds the module declaration).

## State and presentation

- `LookState` fields: `opacity:f32`, `animations:bool`, `elapsed_ms:f32`,
  `focused:Option<String>`, `panes:Vec<PaneState>`, `sessions:Vec<SessionItem>`,
  `agents:Vec<AgentItem>`, `files:Vec<FileItem>`, `timeline:Vec<TimelineItem>`,
  `activity:Vec<f32>`, `uptime_seconds:u64`, `masking_ready:bool`,
  `pane_radius:f32`, `panels:Vec<PanelState>`. Default opacity .94, animations true,
  radius 11 logical px; otherwise empty/zero/false. Radius override is for the
  independent harness fault, never an environment switch in production.
- `PaneState::new(id:impl Into<String>, snapshot:TerminalSnapshot)->Self`.
  Public fields: `id:String`, `title:String`, `driver:String`, `allow_agent:bool`,
  `agent_running:bool`, `snapshot:TerminalSnapshot`, `underlines:Vec<Underline>`,
  `blocks:Vec<BlockCard>`, `cursor:Option<CursorMotion>`, `scroll_px:f32`.
- `BlockCard` public fields: `id:String`, `first_row:usize`, `last_row:usize`
  (inclusive local grid rows), `command:String`, `exit_code:Option<i32>`,
  `duration_ms:Option<u64>`, `running:bool`. Draw grid text once; do not duplicate
  or replace actual terminal text with a guessed command label.
- `CursorMotion {from:[f32;2], to:[f32;2], elapsed_ms:f32}` uses physical window
  coordinates. Focused pane uses ease_out_cubic over 80 ms; without motion state,
  use the snapshot cursor at its grid origin. Always clear GridLayer cursor
  when the app supplies the cursor shape; unfocused panes show no cursor.
- `SessionItem {id:String,title:String,active:bool}`: id is a stable terminal
  anchor for its group, not a mutable group index.
- `AgentItem {connection_id:u64,peer_pid:u32,tab_id:Option<String>,running:bool}`.
- `FileItem {id:String,title:String,pending:usize,active:bool}`.
- `TimelineItem {label:String,detail:String}`: metadata only, never output payloads.
- `PanelState {id:String,rect:PixelRect,title:String,lines:Vec<PanelLine>,
  snapshot:Option<TerminalSnapshot>,underlines:Vec<Underline>,opacity:f32,rise_px:f32}`.
  UI lines use Inter; an optional grid is editable content (mono). Panel line
  height is 20 * scale, padding 12 * scale, header 24 * scale. Keep that geometry
  for hit conversion. `PanelLine {text:String,selected:bool,action:Option<HitAction>}`.
  Text is clipped/truncated to its box; panels have rounded corners/shadows.
- `HitAction`: Palette, ToggleRails, Session(String), File(String),
  Agent {connection_id:u64,tab_id:Option<String>}, Access(String),
  Block {tab_id:String,block_id:String,action:String}, PaletteChoice(usize).
- `Hotspot {rect:PixelRect,action:HitAction}`; `Presentation {scene:Scene,hits:Vec<Hotspot>}`.
- `presentation(&Layout,&LookState)->Presentation`; `scene(&Layout,&LookState)->Scene`
  wraps the same builder. No filesystem, RPC, clock or environment reads in either.

Theme and fixed pixels are exactly docs/design-m11-look.md. Card is white 3%
preblended over panel, exit pill ok/err 18% over card; inset4, chip right6 and
horizontal text padding8. The fixed fixture's card x228..946 and exit right940
must leave sample x936 in the pill's right padding. Scene grids already treat
black backgrounds as transparent: do not recolour program output. Nonfocused,
nonagent panes have no shadow. Top bar/rails do not cast shadows into y44.
Agent glow must meet the design hue check, not merely be a faint desaturated glow.

## Window integration

Window worker owns main.rs/panels.rs/editor_workspace.rs plus new
`clanky/src/studio.rs` if helpful; no renderer or look builder writes. Main uses
pixel pane origins for terminal/editor input hit testing and PTY dimensions.
The current server terminal group is expanded into the local editor workspace
leaf named terminal-workspace. Keep local editor IDs out of workspace_set.
Map divider paths back to the server subtree or local editor tree and cancel
captured drags on workspace revision changes. Core ratios/layout store already work.

Restyle palette/file/search prompts as PanelState with UI text. Compose docks at
bottom of its captured pane, reserves terminal rows, and maps input to the actual
compose content rectangle. Editor source remains a GridLayer; review controls can
be UI cards. Keep old actions/IME/permissions/revocation semantics and the stop key
first. Use ease_out_cubic for 80 ms cursor and 120 ms panels/smooth scrolling;
animations=false must remove interpolation/pulsing. Never paint two cursors.

Create window with with_transparent(true). After Renderer::new call
set_font_families(config.font_family, config.ui_font_family); a failure is a config
error with its message. If !supports_transparency(), show a one-time status line.
Call draw_scene, not the legacy full-window grid. Config.rails controls initial
rails; Ctrl+Shift+B toggles both and immediately recomputes PTY geometry.

## Owner metadata

Metadata worker may add owner-only v3 `look_state {}` and document it. It returns
`uptime_ms:u64`, `masking_ready:bool`, `agents:[{connection_id,peer_pid,tab_id,running}]`,
`drivers:[{tab_id,command_id,connection_id,peer_pid,running}]`,
`events:[{seq,kind,tab_id,command_id}]` (last20, no raw text/data).
Connected peers must use real connection lifecycle/owner-role tracking, not audit
recency. Driver metadata binds an agent-issued run to the actual command and
checks whether it is still running. Never retain raw arguments or clipboard data.
Poll metadata at most four times per second, cache for drawing, and show unknown
rather than guess if unavailable. Existing v1-v4 operations and reply fields stay.

Live block cards need honest viewport mapping: use existing owner block text only
where its visible rows can be matched without inventing coordinates; fullscreen
rewrites or ambiguous mappings must not paint cards over unrelated text. Report
missing stable block-position metadata to the lead; do not add another VT parser
or change the fixed gate's explicit BlockCard row fixture.
