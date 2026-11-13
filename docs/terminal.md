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

## Mouse

When a program asks for the mouse (mc, vim with `mouse=a`, htop), clicks, drags and the
wheel go to it as xterm reports them: modes 1000 (buttons), 1002 (motion with a button held),
1003 (all motion), with the 1006 (SGR) or 1005 (UTF-8) encodings or the original byte form.
Reports carry Shift, Alt and Ctrl, as VTE sends them; each wheel notch is one report. Nothing is
reported while the view is scrolled back into the history. The pointer is an arrow while a program
has the mouse and an I-beam over terminal text otherwise. Hold Shift to select or middle-click
paste anyway. Programs that ask for focus reports (1004) get `ESC [I` and `ESC [O` when the
window gains and loses focus.

Otherwise the mouse selects text, as in Ptyxis: drag for characters (Ctrl+drag for a rectangle),
double-click for a word, triple-click for the line, Shift+click to extend. The selection is drawn
in the theme's selection colour, works over the history, and goes to the PRIMARY selection, so
a middle click pastes it. It stays put as output scrolls and goes away when the text it covers
changes. Ctrl+Shift+C copies it to the clipboard; Ctrl+Shift+V pastes the clipboard.

Pasting follows VTE: control characters become visible symbols (U+2400 and up) instead of
running, line feeds become carriage returns, and the text is wrapped in `ESC [200~` …
`ESC [201~` when the program turned on bracketed paste.

A right-click opens the pane's menu: copy and paste, links, the command under the pointer, find,
splits, the agent, agent access, reset and close (see [shortcuts](shortcuts.md#mouse-in-a-terminal-pane)).
While a program has the mouse, the pane's top bar says so ("mc · has the mouse"): a plain
right-click goes to the program, and Shift+right-click opens the menu. **Reset terminal** turns off
what a crashed program can leave on (mouse reporting, bracketed paste, application keys, the
alternate screen, a scroll region, colours and character sets) and keeps the text, as VTE's reset
does. **Reset and clear** is a full reset: the screen and the history go too.

On a full-screen program's screen (the alternate screen) with no mouse mode, a wheel notch sends
max(1, ⌈lines ÷ 10⌉) Up or Down keys, as VTE does (mode 1007, on by default). On the normal
screen the wheel scrolls the history. Ctrl+wheel zooms.

## Keys

`TERM` is `xterm-256color`, and keys are sent as xterm sends them; see
[keyboard shortcuts](shortcuts.md). Programs such as mc get F1–F12 and application cursor keys.
