# Settings

Open Settings from the gear at the right of the title bar or **Ctrl+,**. It opens on General. Escape closes it;
inside a field or a colour dialog, Escape cancels that edit first. Every setting of the
[config file](config.md) has a row here, and each change is saved to `config.xml` at once, so the file
never needs editing by hand. Rows say so when a change applies later ("Applies to new windows").

Controls: a **switch** flips a boolean (a click anywhere on its row works too); **segments** pick one
of a few words; a **field** shows the value as the file writes it: click it, type (Ctrl+A selects all),
Enter saves, Escape cancels, and a click elsewhere keeps what was typed. A value out of range is refused
with the reason shown above the page, and the file is not changed.

When `config.xml` is malformed or holds a bad value, the window still opens with the last good settings
and Settings shows the problem with its line above every page (`config.xml line 4: ...`). The file is
left alone until you change something here; that change writes a well-formed file.

## Pages

- **General:** Post-AI mode (`post_ai`; it applies server policy and hides agent UI immediately),
  Restore layout, Robot greeting (`startup_animation`), Animations, Notify after (seconds), and the
  Columns and Rows of a new window.
- **Appearance:** first the **Layout** group, then **Text**, **Window** with the window colours, and last
  **Theme** (the longest group).
  - Layout: **Tab style** Horizontal | Vertical (`tab_style`), **Tools panel** On | Off with the tool chips
    Files, Recent and Agents (`tools_panel`, `tools`), **Command input** Editor | Classic (`command_input`; Classic gives the pane its rows back and the shell its own prompt at its next prompt),
    **Blocks** Quiet | Cards (`block_style`) and **Status line** Show | Hide (`status_line`). A click saves at
    once and the window changes at once, without a restart. A default value is removed from the file.
  - Theme: **Follow the system** On | Off. On (`theme` `system`) shows two columns, In dark mode and In light
    mode, each choosing its own slot (`theme_dark`, `theme_light`). Off shows one list; choosing a theme sets
    `theme` to its appearance and its slot. Each row shows the theme's name and four dots of its ANSI red,
    green, blue and yellow (colours 1, 2, 4, 3) on a chip of its background. Palettes imported into
    `~/.local/share/org.gnome.Ptyxis/palettes` follow; choosing one sets `terminal_colors` from its [Dark]
    section and leaves the window theme alone.
  - Keys on this page: **Down** and **Up** move a highlight through the rows (the first press starts from the
    saved theme) and preview it in the window behind Settings without saving; **Enter** saves the highlighted
    row; **Escape** puts the saved theme back (a second Escape closes Settings). A click on a row saves it.
  - Text: **Font size** (`font_size`, pixels), **Terminal font** (type a family, or pick one of the
    installed monospace families from the menu beside it), **Interface font**, **Cell width** and
    **Line height** (1 to 2). Window: **Opacity** (a slider and its value, 0.5 to 1).
  - Window colours: one swatch per colour token of the theme, in two columns (`colors`); **Reset** takes the
    place of a changed colour's value. Save as new theme, Import and Export follow.
- **Terminal:** **Cursor blink** System | On | Off, **Blink cycle** (ms), **Stop blinking after** (s),
  **Clipboard writes** and **Clipboard reads** (OSC 52), then the terminal's own colours
  (`terminal_colors`): background, foreground, cursor and the 16 ANSI colours (normal 0–7, bright 8–15),
  **Import Ptyxis palette…** and **Use theme colours**. See [the terminal](terminal.md).
- **Editor:** the compose editor's spelling **Dictionary**, **Dictionary folder**, and **Enter after send**.
- **Agents:** **New agent panes** Read + run | Off (`agent_pane_access`); the agent tools (`agents`), one row
  each with **Remove**, and **Add agent** (type the name, Tab, the command, Enter); then masking status,
  stop/resume actions and named per-pane access switches. Resume permits new agents but does not restore
  revoked pane permissions.
- **History:** **Save history**, **Keep for** (days, 0 keeps everything) and **Output per command** (KiB).
- **Keys:** current shortcuts as right-aligned key chips; unavailable screens are omitted.

Every page scrolls with the wheel or Page Up/Down. In Post-AI mode the Agents page keeps the agent settings (they apply once it is off) without the access
rows, and the agent shortcuts are hidden. A window
started with `--post-ai` shows the mode switch without effect and explains that the server must restart to
turn it off. The old Settings modal is never used as a fallback. Ctrl+Shift+Escape still revokes agents.

## Colours and themes

Click a swatch/colour row to choose an ANSI swatch or enter a hex colour. Reset
appears only for a changed value. Invalid values keep the previous file/display.
The theme's ANSI 1–6, background and muted text colours also colour code in editor panes
(see [editor colours](config.md#editor-colours)); the Terminal page's colours do not.

**Save as new theme** asks for a display name and a safe file-name stem. It copies
the edited theme through `ThemeStore`, selects that copy and clears the baked-in
semantic config overrides. Built-ins are never overwritten.

**Export** writes the selected theme to a new file. Save modified colours as a new
theme first. **Import** asks for a source file and a new file-name stem, preserves
its bytes and selects it for its light/dark appearance. Existing destinations are
refused, not replaced. These buttons handle themes, not whole-settings imports.

Text fields accept typing, Backspace/Delete, arrows, Home/End, Ctrl+A and Ctrl+V.
Enter applies; Escape cancels. Tab moves focus. Sliders support left/right arrows.
Page Up/Down and the mouse wheel scroll long pages.
The footer's **Open settings file** uses the desktop's file association.

Hand edits are checked every 500 ms. Invalid files retain the last-good settings;
user-theme errors retain their last-good theme. Panel writes preserve the order of
the settings, but rewrite the file's whitespace and comments. A saved font-size change resets window-local zoom.
