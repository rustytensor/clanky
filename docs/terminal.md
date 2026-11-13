# The terminal

The panes aim to feel like Ptyxis (VTE): the same cell sizes, keys, colours and cursor.

## Fonts and line height

`font_family` names the bundled Geist Mono, JetBrains Mono or DejaVu Sans Mono, or any installed
monospace family; Settings > Terminal lists them. Cells are sized from the font the way VTE does:
the width is the widest ASCII character's advance rounded up, the height the font's ascent and
descent each rounded up, and each is then multiplied by `cell_width_scale` / `cell_height_scale`
(1.0–2.0, Settings' **Line height**) and truncated. IBM Plex Mono 11 pt with a line height of
1.25 gives 9 × 26 px cells, as in Ptyxis. Bold text (SGR 1) uses the font's bold face in the same
colour; faint text (SGR 2) is drawn two thirds of the way from the background to its colour.

## Cursor

A block in the cursor colour. With `cursor_blink` `system` (the default) it follows GNOME's
cursor-blink settings; `on` uses `cursor_blink_time_ms` and `cursor_blink_timeout_s`. As in VTE,
the cursor is shown for half the blink time and hidden for half, only while the window has focus;
each key press restarts it, and it stops, shown, once the timeout has passed since the last key.
Nothing is redrawn once it stops, or with `cursor_blink` `off`.

## Colours

`terminal_colors` sets the terminal's background, text, cursor and 16 ANSI colours apart from the
window theme; the sidebar, cards and top bar keep the theme. Unset entries follow the theme, and
**Use theme colours** in Settings removes them all.

To use a Ptyxis palette, choose **Import Ptyxis palette…** in Settings > Terminal (palettes live in
`~/.local/share/org.gnome.Ptyxis/palettes/`), or print it as JSON for the config file:

```sh
clanky terminal-colors import ~/.local/share/org.gnome.Ptyxis/palettes/riley.palette
```

Both use the palette's `[Dark]` section: Background, Foreground, Cursor and Color0–Color15.

## Keys

`TERM` is `xterm-256color`, and keys are sent as xterm sends them; see
[keyboard shortcuts](shortcuts.md). Programs such as mc get F1–F12 and application cursor keys.
