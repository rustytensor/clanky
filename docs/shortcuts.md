# Keyboard shortcuts

These are Clanky controls, not shell bindings. Prefixes below name the input
context. **Ctrl+Shift+Escape is handled before every window modal**: it revokes
agent access and cancels waits, but does not kill running commands.

During the [startup greeting](robot.md), any key or click skips the robot **and still acts
normally**. In the standalone `clanky +hello` window, a key or click closes the replay.
[Post-AI mode](post-ai.md) blocks agent requests at the server. Agent controls may remain visible
until the window's plain-terminal view is connected.

## Window

| Key | Action |
| --- | --- |
| `Ctrl+Shift+Escape` | Revoke all agents, including while a panel, search or compose prompt is open |
| `Ctrl+Shift+P` | Toggle palette |
| `Ctrl+,` | Open/close Settings; the sidebar Settings item does the same |
| `Ctrl+Shift+B` | Show/hide the sidebar and activity column |
| `Ctrl+Shift+T` | New terminal tab in the focused terminal folder, or server home when unknown |
| `Ctrl+Shift+D` | Split right; with editor focus, prompt for another file |
| `Ctrl+Shift+E` | Split below; with editor focus, prompt for another file |
| `Ctrl+Shift+W` | Close focused pane; dirty editor refuses; last terminal closes its server |
| `Ctrl+Shift+A` | Toggle agent access to the focused terminal |
| `Ctrl+Shift+G` | Open the [agent picker](agent-pane.md); Enter starts the highlighted tool |
| `Ctrl+Shift+O` | Open local file prompt |
| `Ctrl+Shift+F` | Search retained terminal text, including soft wraps |
| `Ctrl+Shift+C` | Copy the terminal selection to the clipboard |
| `Ctrl+Shift+V` | Paste the clipboard into the focused terminal (bracketed when the program asks) |
| `Ctrl+Shift+Enter` | Open compose docked to the focused terminal; does not send a draft |
| `Ctrl+=` | Zoom in |
| `Ctrl++` | Zoom in |
| `Ctrl+-` | Zoom out |
| `Ctrl+0` | Reset zoom |
| `Ctrl+Tab` | Next terminal tab group |
| `Ctrl+Shift+F6` | Focus/cycle visible command cards; reveal block actions (plain F6 goes to the program) |
| `Card:ArrowLeft` | Choose the previous card action |
| `Card:ArrowRight` | Choose the next card action |
| `Card:Tab` | Choose the next card action |
| `Card:Enter` | Activate the chosen block action |
| `Card:Escape` | Return keyboard input to the pane |

The top-bar New tab, Split, Agent and search controls are mouse alternatives to these shortcuts.
In the agent picker, Up and Down choose a tool, Tab or Left/Right switch between a split and a new tab, and Escape closes it.
Drag a pane divider to resize it; click anywhere on an agent-access row to toggle it.

## Mouse in a terminal pane

The mouse works as in Ptyxis (VTE). See [the terminal](terminal.md#mouse) for what programs receive.

| Mouse | Action |
| --- | --- |
| Drag | Select text; selecting copies it to the PRIMARY selection |
| Ctrl+drag | Select a rectangle |
| Double-click | Select a word: letters, digits and `-#%&+,./=?@\_~·`, so a path is one word |
| Triple-click | Select the whole line |
| Shift+click | Extend the selection |
| Click | Clear the selection |
| Middle click | Paste the PRIMARY selection |
| Wheel | Scroll the history; on a full-screen program's screen, send Up/Down keys |
| Ctrl+wheel | Zoom in and out |
| Right-click | Open the pane's menu; the selection stays |
| Shift+right-click | Open the menu while the program takes the mouse (a plain right-click goes to the program) |
| Shift + any of these | Select and paste even while the program takes the mouse (mc, vim, htop) |

The menu opens at the pointer, flipped up or left to stay in the window. Its rows, top to bottom:
Open link and Copy link (over a link); Copy, Paste, Select all; Copy command, Copy output and Run
again (over a finished command); Find…, Split right, Split below, Start an agent here…, Agent access;
Reset terminal, Reset and clear; Close pane. Post-AI mode leaves out the two agent rows. Rows that
can't act are greyed (Copy with nothing selected). Up and Down move the highlight, Enter runs it,
Escape or a click outside closes the menu, and that click does nothing else. In an editor pane the
menu has Cut, Copy, Paste, Select all, Find, Split right, Split below and Close pane.

Block actions appear only while the card is hovered or keyboard-focused. They
fade in over 120 ms; animations off makes them immediate. Card focus consumes
ordinary typing and IME input rather than sending it to the shell underneath.


Keys a program in a pane receives follow xterm (`TERM=xterm-256color`): cursor keys send `ESC [ x`, or
`ESC O x` after the program turns on application cursor keys (`ESC [?1h`, as mc and vim do); modified
keys use xterm's forms (Shift+Up `ESC [1;2A`, Ctrl+Right `ESC [1;5C`, Ctrl+F5 `ESC [15;5~`);
Shift+Tab sends `ESC [Z`; Alt prefixes `ESC`. F1–F12 all reach the program.
## Settings

| Key | Action |
| --- | --- |
| `Settings:Enter` | Apply the edited value or chosen import/export path |
| `Settings:Escape` | Cancel the current editor/picker, or close Settings |
| `Settings:Backspace` | Delete the final character, or selected field text |
| `Settings:Ctrl+A` | Select the whole field |
| `Settings:Ctrl+V` | Paste clipboard text into the field |
| `Settings:ArrowUp` | Scroll the section up when not editing |
| `Settings:ArrowDown` | Scroll the section down when not editing |

## Palette and file prompts

| Key | Action |
| --- | --- |
| `Palette:Escape` | Close palette/prompt |
| `Palette:ArrowUp` | Previous action |
| `Palette:ArrowDown` | Next action |
| `Palette:Enter` | Run action or confirm prompt |
| `Palette:Backspace` | Erase last prompt character |
| `File prompt:Escape` | Cancel Open file / Load changes |
| `File prompt:Enter` | Confirm pinned target/path |
| `File prompt:Backspace` | Erase last path character |

## Graphical editor

| Key | Action |
| --- | --- |
| `Editor:Ctrl+S` | Save atomically; refuse external-file conflict |
| `Editor:Ctrl+Z` | Undo |
| `Editor:Ctrl+Shift+Z` | Redo |
| `Editor:Ctrl+F` | Open local find prompt |
| `Editor:Ctrl+Alt+A` | Accept selected hunk in memory |
| `Editor:Ctrl+Alt+R` | Reject selected pending hunk |
| `Editor:Ctrl+Alt+Shift+A` | Accept all pending applicable hunks |
| `Editor:Ctrl+Alt+Shift+R` | Reject all pending hunks |
| `Editor:ArrowLeft` | Move to previous Unicode character |
| `Editor:ArrowRight` | Move to next Unicode character |
| `Editor:ArrowUp` | Move to line above |
| `Editor:ArrowDown` | Move to line below |
| `Editor:Home` | Move to line start |
| `Editor:End` | Move to line end |
| `Editor:PageUp` | Move to page above |
| `Editor:PageDown` | Move to page below |
| `Editor:Shift+ArrowLeft` | Extend selection to previous Unicode character |
| `Editor:Shift+ArrowRight` | Extend selection to next Unicode character |
| `Editor:Shift+ArrowUp` | Extend selection to line above |
| `Editor:Shift+ArrowDown` | Extend selection to line below |
| `Editor:Shift+Home` | Extend selection to line start |
| `Editor:Shift+End` | Extend selection to line end |
| `Editor:Shift+PageUp` | Extend selection to page above |
| `Editor:Shift+PageDown` | Extend selection to page below |
| `Editor:Enter` | Insert newline; in find prompt, find next with wrap |
| `Editor:Escape` | Close find prompt |
| `Editor:Backspace` | Delete selection/previous character; erase in find prompt |
| `Editor:Delete` | Delete selection/next character |
| `Editor:Tab` | Insert tab |
| `Editor:Space` | Insert space |

Typing replaces the selection. Click/drag selects; Shift+click extends. Wheel
scrolls text or the review preview under the pointer. Review buttons include
accept/reject/all and previous/next; stale hunks cannot be applied.

## Compose

| Key | Action |
| --- | --- |
| `Compose:Ctrl+Enter` | Send one bracketed paste to the captured terminal (no extra Enter by default) |
| `Compose:Escape` | Close suggestions/find first, otherwise close and retain draft |
| `Compose:F7` | Request suggestions at/after caret, wrapping to the first misspelling |
| `Compose:ArrowUp` | Previous suggestion; otherwise move caret up |
| `Compose:ArrowDown` | Next suggestion; otherwise move caret down |
| `Compose:Enter` | Apply selected suggestion; otherwise newline (or next find result) |
| `Compose:Ctrl+Alt+A` | Add current misspelling to personal dictionary |
| `Compose:Ctrl+S` | No file save: draft remains memory-only |
| `Compose:Tab` | Insert tab |
| `Compose:Space` | Insert space |

Compose also uses editor navigation, Shift selection, find and undo/redo. It
does not import review hunks. Drafts are at most 256 KiB UTF-8 and retained per
terminal after Escape or send; closing the window can lose them.

## Scrollback search

| Key | Action |
| --- | --- |
| `Search:Enter` | Next match |
| `Search:Shift+Enter` | Previous match |
| `Search:Escape` | Close search |
| `Search:Backspace` | Erase last query character |
| `Search:Space` | Insert query space |

Typing/IME changes the literal, case-sensitive query (up to 256 bytes). The
matched view stays fixed until a query/navigation action. Plain URL clicks are
inert; **Ctrl+click** opens only HTTP/HTTPS URLs. Ordinary path:line clicks open
the editor. Hover underlines a complete URL visible in one physical row.

## Text-mode editor: clanky edit

| Key | Action |
| --- | --- |
| `TUI:Ctrl+S` | Save |
| `TUI:Ctrl+Z` | Undo |
| `TUI:Ctrl+Y` | Redo |
| `TUI:Ctrl+F` | Find |
| `TUI:Ctrl+Q` | Quit; ask before discarding unsaved changes |
| `TUI:Ctrl+C` | Quit with the same unsaved-change confirmation |
| `TUI:Y` | Confirm discarding unsaved changes at quit prompt (y also accepted) |
| `TUI:N` | Cancel quit prompt (n also accepted) |
| `TUI:F1` | Toggle help |
| `TUI:F5` | Accept selected hunk, or next pending hunk when none selected |
| `TUI:F6` | Reject selected hunk, or next pending hunk when none selected |
| `TUI:F7` | Accept all pending hunks |
| `TUI:F8` | Reject all pending hunks |
| `TUI:Escape` | Cancel find/quit prompt; otherwise close help and clear selected hunk |
| `TUI:Enter` | Newline; in find prompt, next match with wrap |
| `TUI:Tab` | Next review hunk when pending/stale hunks exist; otherwise insert tab |
| `TUI:Shift+Tab` | Previous review hunk when pending/stale hunks exist |
| `TUI:Backspace` | Delete selection/previous character, or erase query |
| `TUI:Delete` | Delete selection/next character |
| `TUI:Left` | Move to previous Unicode character |
| `TUI:Shift+Left` | Extend selection to previous Unicode character |
| `TUI:Right` | Move to next Unicode character |
| `TUI:Shift+Right` | Extend selection to next Unicode character |
| `TUI:Up` | Move to line above |
| `TUI:Shift+Up` | Extend selection to line above |
| `TUI:Down` | Move to line below |
| `TUI:Shift+Down` | Extend selection to line below |
| `TUI:Home` | Move to line start |
| `TUI:Shift+Home` | Extend selection to line start |
| `TUI:End` | Move to line end |
| `TUI:Shift+End` | Extend selection to line end |
| `TUI:PageUp` | Move to 20 lines above |
| `TUI:Shift+PageUp` | Extend selection to 20 lines above |
| `TUI:PageDown` | Move to 20 lines below |
| `TUI:Shift+PageDown` | Extend selection to 20 lines below |

Accept changes memory only; Ctrl+S writes disk. Stale hunks and external-file
conflicts refuse application/save. The standalone text editor does not manage
server agent permissions. See [getting started](getting-started.md).
