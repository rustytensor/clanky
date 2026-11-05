# Settings

Open Settings from the sidebar or **Ctrl+,**. Escape closes it; inside a colour or
theme dialog, Escape cancels that edit first. Changes apply and save as you go.
The rebuilt panel uses the same [JSON config file](config.md) as hand editing.

## Live pages

- **General:** Post-AI mode. It applies server policy and hides agent UI immediately.
- **Appearance:** Light/Dark/System, theme cards, semantic colours and per-colour Reset.
  Scroll to reach all themes, colours and the Save/Import/Export buttons.
- **Terminal:** bundled monospace font, size, opacity, side columns and animations.
- **Agents:** masking status, stop/resume actions and named per-pane access switches.
  Resume permits new agents but does not restore revoked pane permissions.
- **Keys:** current shortcuts as right-aligned key chips; unavailable screens are omitted.

Post-AI hides the Agents page and agent shortcuts. A window started with `--post-ai`
shows a disabled mode switch and explains that the server must restart to turn it off.
The old Settings modal is never used as a fallback. Ctrl+Shift+Escape still revokes agents.

## Colours and themes

Click a swatch/colour row to choose an ANSI swatch or enter a hex colour. Reset
appears only for a changed value. Invalid values keep the previous file/display.

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
