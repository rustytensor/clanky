# Settings rebuild: frame

The replacement frame is in `crates/clanky/src/calm/settings_frame.rs`. It is not
connected to the live window yet. Its Terminal page draws the approved copy,
select, stepper, percentage slider and switches; input handling is unfinished.
Appearance, General, Agents and Keys still need their page contents.

The frame uses the approved HTML's measured 780×600 panel, 200-pixel sidebar,
56-pixel header/footer and 68.84375-pixel bordered Terminal rows at scale 1.
`settings_frame_geometry` passed after its planted +1-pixel width fault failed.
The `settings-frame` example resolves themes through `ThemeStore` and
`Config::resolved_document`, then renders a 2× comparison image.

Comparison files are under `/srv/devops/build/clanky-editor-m12-reference/`:
`settings-terminal-2x.png` and `settings-terminal-reference-2x.png`.
These are frame previews, not evidence that the Settings controls work.

Legacy Settings, palette, search, compose and editor opening routes are hidden
in the calm review build, including keyboard and terminal-path mouse routes.
A private-window check pressed Settings and palette shortcuts, then typed a
command; its exact output still reached the shell. This check preceded the lead's
robot integration. The combined tree passed a binary/library compile check.

Live-window defaults are Geist Mono 13 px, Geist UI and full opacity. Explicit
older font settings remain intact. The default-font check passed after its
planted fault failed. The ANSI provenance check passed with unchanged colour
expectations, sampling the new renderer's 8×20 cell centres.

Next: connect Settings controls to the shared settings/theme APIs, complete the
remaining pages, and enforce Post-AI at the server and MCP boundaries. Post-AI
config parsing alone is not enforcement; that part is not implemented yet.
