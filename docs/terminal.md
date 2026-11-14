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
window theme; the title bar, tools panel and status line keep the theme. Unset entries follow the theme, and
**Use theme colours** in Settings removes them all. Code in editor panes keeps the window theme's
colours too (see [editor colours](config.md#editor-colours)).

To use a Ptyxis palette, choose **Import Ptyxis palette…** in Settings > Terminal (palettes live in
`~/.local/share/org.gnome.Ptyxis/palettes/`), or print its colours as JSON:

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

When the shell in a pane exits (`exit`, Ctrl+D, or killed by a signal), the pane closes, as in
Ptyxis: its tab goes with it when it was the tab's last pane, and after the last pane the window
and its server exit, as Ctrl+Shift+W does. A command an agent started in a tab of its own leaves
the tab in place, so the agent can still read its output and exit code.

On a full-screen program's screen (the alternate screen) with no mouse mode, a wheel notch sends
max(1, ⌈lines ÷ 10⌉) Up or Down keys, as VTE does (mode 1007, on by default). On the normal
screen the wheel scrolls the history; typing to the shell brings the view back to the newest
rows, as VTE does. Ctrl+wheel zooms.

## Keys

`TERM` is `xterm-256color`, and keys are sent as xterm sends them; see
[keyboard shortcuts](shortcuts.md). Programs such as mc get F1–F12 and application cursor keys.

## Command input

With `command_input` `editor` (the default) each terminal pane has an input at its bottom, under a
hairline: chips for the pane's folder, its git branch, the shell and agent access (a switch; a click
toggles it), then the line you write, with a Shell | Agent switch at the right (Agent opens the
[agent picker](agent-pane.md); Clanky itself makes no AI calls).

- Write as in an editor: click to place the caret, drag or Shift+arrows to select, Ctrl+Z and
  Ctrl+Shift+Z to undo and redo, Shift+Enter for another line (up to 8 show; more scroll).
- bash colours come from tree-sitter-bash's own highlight query, in the theme's terminal colours:
  commands blue, strings green, variables cyan, flags violet, comments in the muted colour.
- Grey ghost text offers the newest command from history that starts with what you wrote; Right or
  End at the end of the line takes it, Enter never does. History is this window's commands, the
  commands the panes ran, and the saved [history](history.md) (rows the server masked are left out).
- Up recalls older commands that start with what you wrote before the first Up; Down goes back.
- Tab completes the word at the caret: commands from PATH and bash builtins for the first word, git
  branches after `git checkout` or `git switch`, otherwise files and folders relative to the pane's
  folder (a folder gets its `/`, other completions a space). One match goes in at once; more open a
  list above the input: Tab or Enter takes the highlighted item (Enter then runs the line), Up and
  Down move, Escape closes it.
- Enter sends the text to the shell as one bracketed paste and a carriage return, so a multi-line
  text is one submission and one block. Ctrl+C clears the text, or on an empty line goes to the
  shell; Ctrl+D on an empty line, and Ctrl+L, go to the shell too.

The input steps aside, dimmed, and keys go straight to the program while a command runs (it says
"A command is running: keys go to it. Ctrl+C stops it."), while a program uses the alternate screen
(vim, less, mc), and in a shell that sends no OSC 133 marks. It comes back at the shell's next prompt.
Its rows stay reserved meanwhile, so the terminal is not resized at each command.

Clanky starts bash as `bash --init-file <clanky.bash> -i` and zsh with its own `ZDOTDIR` that reads
your `.zshenv` and `.zshrc` first, then adds the marks and OSC 7 folder reports. The scripts live in
`$XDG_RUNTIME_DIR/clanky/shell-<version>-<uid>/`. In editor mode the prompt prints only the marks
(the block's meta line shows the folder instead); in classic mode your own `PS1` (or zsh `PROMPT`)
prints between them. `VTE_VERSION` is not passed to programs, so Fedora's `vte.sh` does not add its
own marks. Agents' commands run as given, without the integration.

## Command blocks

With `block_style` `quiet` (the default), commands that the shell marks with OSC 133 are drawn
as quiet blocks. With the command input on (`command_input` `editor`), the blocks breathe: each
has its own meta row above the bold command, with the folder and start time (HH:MM) on the left
and "exit N" (in the error colour, when the command failed) and the duration on the right; there
are 9 px of space above the meta row and 11 px under the block before it, with a 1 px hairline in
that space (red after a failure, none above the first block). The space is drawing only: the
program's screen and its rows do not change, and clicks, selection, links, the right-click menu
and mouse reports land on the cell under the pointer. The newest row stays in view; at the top of
the history the wheel shows the rows the space pushed above the pane. A full-screen program (the
alternate screen) and output without OSC 133 draw the plain grid. In `classic` mode a 1 px
hairline sits above each command row (none above the first row; red, with "exit N" in the error
colour, when the command failed), the command row is bold, and the folder, start time and duration
are right-aligned on the command row. Hovering a block
shows one floating bar over the meta text: **Copy output**, **Copy command**, **Run again**, **Ask agent** (not in
Post-AI mode) and **⋯** (the right-click menu). When a block's command row has scrolled off the
top while its output still shows, a sticky line at the top of the pane names the command; while
it runs it counts "running N s", redrawing once a second and no more. A running block whose
command row is on screen counts the same way in its meta text. `block_style` `cards`
keeps the older card around each command.
