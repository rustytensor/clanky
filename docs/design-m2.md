# M2: splits, images, readable fonts and Wayland

M1 is accepted after the window check. M2 keeps the socket protocol and existing
terminal IDs unchanged. The implementation proceeds after this design commit.

## Gate first

Command: `scripts/gate-m2.sh`. Missing binaries, adapters, input files, timeouts
or unexpected outputs are FAIL with a nonzero exit; no skips and no downloads.
The harness uses Python's standard library, real core/parser/renderer code and
bounded subprocesses. Expected values below are fixed before implementation.

1. **Images through APC and GPU readback.** Python creates a 2x2 opaque PNG:
   red, green / blue, white. It wraps it in Kitty direct-transfer bytes after
   `ESC[3;3H`, with `a=T,f=100,t=d,i=1,c=2,r=2,q=2`. The Rust harness receives
   that file, feeds it through the patched vte parser and core image decoder,
   and draws an 80x24 snapshot through the shared renderer at fixed 16x32 cells.
   Pixel samples (40,80), (56,80), (40,112), (56,112) must be respectively
   (255,0,0), (0,255,0), (0,0,255), (255,255,255), alpha 255, tolerance 8 for RGB.
   Feed the same bytes in two-byte chunks as well; both paths must give those
   four colours. The PNG/base64 and protocol inputs are produced by the gate,
   not inferred from the decoder's output.
2. **Splits with independent terminals.** The core layout partitions an 80x24
   cell rectangle into left/right panes separated by one column: left is
   (col=0,row=0,cols=39,rows=24), right is (40,0,40,24). Splitting the right pane
   top/bottom gives (40,0,40,11) and (40,12,40,12). Hit-testing cell (41,13)
   selects the bottom-right terminal; (39,10) hits no terminal (the divider).
   Run `printf A`, `printf B`, `printf C` in the three real PTYs. Require exact
   independent raw outputs A/B/C and exit 0. Closing the top-right leaf expands
   its sibling to (40,0,40,24), without changing the surviving terminal IDs.
3. **Config, DPI and zoom.** Load a real JSON file containing
   `{"font_size_pt":12,"columns":80,"rows":24}`. At scale 1 the font is 16 physical
   pixels and cells are 10x20; zoom +1 pt gives 11x22; reset gives 10x20. At
   scale 1.5 the reset font is 24 pixels and cells are 15x30. Invalid font size
   0 must return a configuration error, not silently fall back. These values
   come from the formula in the font section, not measured output expectations.
4. **M1 regression.** Run `scripts/gate-m1.sh` once from the clean M2 gate with
   fault injection removed. It still uses 16x32 text cells and its original
   exact outputs; no expected values are relaxed.
5. **Planted fault.** `CLANKY_GATE_FAULT=image scripts/gate-m2.sh` changes only
   the observed first image sample to black before the usual comparison.
   It must fail specifically at the red quadrant assertion (expected red,
   observed black). A crash or unrelated failure does not count. Run this mode
   once before the clean mode. The clean mode must finish with PASS.

Each harness subprocess has a 30-second deadline; image rendering may use a
software adapter. The existing M1 deadlines stay unchanged. Temporary inputs
are private and removed; app/PTY cleanup uses saved process IDs. Gate binaries
return observations; Python owns comparisons. The image harness is
`clanky-image-gate`; the layout/config harness is `clanky-state-gate`.

Manual Wayland acceptance remains separate: move the window between monitors
with different scaling, minimize/restore, resize split panes, use an IME, zoom,
and display an image with `kitten icat`. A headless gate cannot certify these.

## Splits without changing the socket API

Each leaf keeps an existing core terminal ID (`tab_id` in protocol v1), with its
own PTY, screen, output and permission. A window tab is a UI group of these leaves;
its initial terminal remains its group identity. Splitting creates another normal
core terminal and inserts it into that group's layout. `list_tabs` continues to
list terminal IDs and `run` still targets exactly that ID, never whichever pane
happens to have focus. The socket does not acquire layout-changing operations in
M2. M3 can add explicit view operations without changing command routing.

A binary layout tree has terminal leaves and side-by-side or stacked branches.
Partitions divide the available cell rectangle after reserving one divider cell;
the first child gets floor((extent-1)/2), the other gets the remainder. Refuse
splits that cannot leave at least one cell per child. Closing a leaf collapses
its parent into the sibling. Resize each PTY to its pane's content dimensions.
Layout and hit-testing share the same rectangles used to compose the frame.

Ctrl+Shift+D splits side-by-side; Ctrl+Shift+E splits top/bottom. Click a pane to
focus it. Ctrl+Shift+W closes the focused pane; closing the last pane removes its
window tab. Ctrl+Shift+T opens a separate window tab; Ctrl+Tab cycles groups.
Existing independently opened socket terminals appear as separate window tabs.
The top strip shows groups; the focused pane is visually marked. Group layout
is session-only in M2. Agent permissions stay per terminal and default denied
for GUI-created panes. Ctrl+Shift+A affects only the focused terminal.

## Config and font size

Read JSON from `--config PATH`, otherwise
`$XDG_CONFIG_HOME/clanky/config.json` or `$HOME/.config/clanky/config.json`.
A missing default file uses defaults; a missing explicit file or malformed,
unknown or out-of-range fields is an error. No new config-parser dependency.
Defaults: `font_size_pt:12`, `columns:80`, `rows:24`. Font points must be finite
and 6–48; dimensions use the core's existing limits. Config is read at startup;
no watcher, automatic writes or live reload in M2.

Physical font size = points * (96/72) * window scale factor.
Physical cell width = ceil(font_pixels * 0.625), height = ceil(font_pixels * 1.25).
The renderer shapes at that physical size and uses the cell height for baseline
layout. Ctrl+= increases by 1 pt, Ctrl+- decreases by 1 pt, Ctrl+0 restores the
configured size; clamp zoom to 6–48 pt. Zoom is session-only. DPI/zoom changes
invalidate size-dependent glyph cache entries and recalculate pane grids/PTY sizes.
The offscreen M1 and image fixtures explicitly retain 26px glyphs and 16x32 cells.

## Kitty images and the upstreamable parser patch

Vendor the fetched vte 0.15.0 source and licences under `vendor/vte`. The root
manifest uses `[patch.crates-io]`; no hand-written scanner sits before the parser.
Keep a patch note with the upstream version, callback semantics and changed files.

Add `Perform::apc_dispatch(&mut self, bytes: &[u8])` with a default no-op. The
parser collects APC bytes after ESC underscore and dispatches on ST. Chunking
must not change the payload. CAN/SUB cancel the string; non-terminating ESC
aborts it and resumes normal escape parsing. Limit a pending APC to 16 MiB;
overflow discards that string through its terminator without dispatching a prefix.
Normal VT processing remains unchanged; the default performer ignores APC.
The terminal's existing observer receives this callback; cursor placement is
captured at that point in parsing, not at the end of a later output chunk.

Core owns Kitty state, chunk assembly, bounds and PNG/RGB/RGBA decoding. Restore
approved `png =0.18.1` in core; reuse base64. Support direct transmission (`t=d`),
`a=t` / `a=T`, `a=p` for a stored ID, deletion `a=d`, `m` chunks, image/placement
IDs, explicit c/r cell dimensions and standard reply suppression. Reply with
success or a protocol error through the PTY as appropriate. Reject file/shared
memory transport without opening paths. Unsupported controls must not silently
claim success. Compression other than PNG itself can be rejected in M2.

Limit encoded transfer to 16 MiB, decoded dimensions to 4096 per axis, and total
retained decoded pixels to 64 MiB per terminal. Avoid copying pixel buffers on
every snapshot: use shared immutable RGBA storage. Image placement coordinates
are terminal-local cells. Deletion frees retained pixels when no placement uses
them. The renderer clips placements to their pane so images cannot cover another
pane or the tab strip. It uses nearest sampling for deterministic test colours.
The protocol implementation is a documented subset, not a claim of every Kitty
extension. Parser changes and licence notices are reviewed before committing.

## Wayland polish and integration

Use winit's display handle when creating wgpu's window instance. Apply scale
factor changes to font metrics and pane grids, never to the fixed test fixture.
Suspend presentation for zero-size windows and restore after a nonzero resize.
Handle surface lost/outdated without exiting; report unrecoverable GPU errors.
Enable IME, route commits to the focused pane, do not send preedit text to its
PTY, and set the IME cursor rectangle from that pane's cursor and physical metrics.
Clear composition state on focus loss. These use winit abstractions, not Wayland
protocol calls or platform-specific code outside the platform boundary.

Five bounded workers may own: (1) vte/core images, (2) renderer image/font support,
(3) layout geometry, (4) config/zoom/IME helpers, (5) gate harness. The lead owns
app integration and manifests shared across tasks, reads every changed line,
and runs whole builds at -j8. Workers check only their crate at -j2, serialized
with the shared build lock. No worker commits. No extra third-party dependency
other than the approved png restoration. Licence policy includes Unicode-3.0
and Unlicense in addition to MIT, Apache-2.0, BSD, Zlib and ISC.

Every commit is annotated and pushed after the secret scan. Patch tags precede
the gate; v0.2.0 records the passing gate. Stop on the first real failed check,
a disallowed licence, an API-changing design question or 70% context.

## M3 outline: the graphical layer

M3 adds actions on command blocks, an agent-task panel and a command palette.
Keep execution and output in core; UI views refer to stable terminal/command/block
IDs. Closing or rearranging a view must not silently retarget an agent command.

- **Command blocks:** select, copy output, rerun explicitly, collapse and expand.
  Rerun uses captured argv only when known; OSC 133 text is not sufficient to
  infer safe argv. Unknown commands ask for an explicit command, never guess.
- **Task panel:** show agent-provided task labels/state and associated commands.
  Distinguish process exit from an agent's claimed task completion. No remote
  service is needed; scope task records to the local socket instance.
- **Palette:** discover the same explicit actions used by mouse and keyboard.
  Each action declares its target IDs and required permission. Destructive
  actions ask for confirmation in the UI; socket permissions remain explicit.
- **Agent parity:** add versioned, documented view/task/action operations rather
  than altering v1 `run` semantics. M2's UI layout tree is reusable, but a view ID
  is distinct from a terminal ID. Subscriptions need bounded queues and replay
  cursors so a slow agent cannot stall PTY processing.

Before M3 implementation, write its gate using a real two-command transcript:
select/copy exact bytes, collapse without changing retained output, rerun explicit
argv into a new command ID, and compare palette/socket actions against the same
state changes. Planted faults must break one observed result, not weaken the
expected values. UI colours and screenshots alone do not prove command routing.
