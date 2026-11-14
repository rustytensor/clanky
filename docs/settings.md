# Settings

Open Settings from the gear at the right of the title bar or **Ctrl+,**. Escape closes it; inside a colour or
theme dialog, Escape cancels that edit first. Changes apply and save as you go.
The rebuilt panel uses the same [JSON config file](config.md) as hand editing.

## Live pages

- **General:** Post-AI mode. It applies server policy and hides agent UI immediately.
- **Appearance:** first the **Layout** group, then **Theme**, then the theme's colours.
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
  Scroll to reach every theme, the colours and the Save/Import/Export buttons.
- **Terminal:** font (bundled or any installed monospace family), size, opacity, the tools panel and
  animations; then line height, cursor blink (System, On, Off), the terminal's own background, text,
  cursor and selection colours, and the **Colour scheme** group: a search field, **Import…**,
  **Follow the window theme** and 40 built-in schemes (Down and Up preview one in the terminal, Enter
  or a click keeps it, Escape puts the saved colours back). The page scrolls to the rows below
  Animations. See [colour schemes](terminal.md#colour-schemes).
- **Agents:** masking status, stop/resume actions and named per-pane access switches.
  Resume permits new agents but does not restore revoked pane permissions.
- **Keys:** current shortcuts as right-aligned key chips; unavailable screens are omitted.

Post-AI hides the Agents page and agent shortcuts. A window started with `--post-ai`
shows a disabled mode switch and explains that the server must restart to turn it off.
The old Settings modal is never used as a fallback. Ctrl+Shift+Escape still revokes agents.

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
user-theme errors retain their last-good theme. Panel writes preserve key order,
but may reformat whitespace. A saved font-size change resets window-local zoom.
