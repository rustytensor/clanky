# Configuration

Every setting has a row in Settings (Ctrl+, or the gear in the title bar; see
[Settings](settings.md)), and each change is saved at once, so the file below never needs
editing by hand. It stays readable and may be edited: Clanky checks it every 500 ms and applies
a good edit live.

Clanky reads its settings from `$XDG_CONFIG_HOME/clanky/config.xml`, or
`$HOME/.config/clanky/config.xml` if XDG_CONFIG_HOME is unset or empty.
`clanky --config PATH` selects another file. A missing default file means the defaults; a
missing explicit file is an error.

The file is XML: one root element `<clanky version="1">` and one element per setting, named
exactly as the key in the table below, the value as its text. Booleans are `true` or `false`;
integers are written in decimal; numbers in decimal without an exponent (`0.85`, `1.25`, `18`);
strings as text (`&amp;`, `&lt;` and `&gt;` for `&`, `<` and `>`). Lists and objects have their
own elements, as in the example. Comments, blank lines and the XML declaration are allowed. A
setting that is left out has its default.

**Migration from config.json.** Clanky 1.6 and older kept the settings in JSON. On the first
start with that old file and no `config.xml` beside it, Clanky migrates it: it writes
`config.xml` with the same values and renames the old file to `config.json.migrated`, unchanged.
Migration turns the legacy `font_size_pt` P into `font_size` P × 4/3 and `rails` into
`tools_panel` (`on` / `off`), unless the modern key is also there (it wins). With both files
present nothing is migrated. `--config` never migrates; an explicit path ending in `.json` is
still read and written in the old JSON format (migration aside, Clanky never writes JSON).

**A bad file never stops Clanky.** When `config.xml` is not well-formed, or a setting holds a bad
value, the window opens with the last good settings (the defaults at a cold start) and Settings
shows the problem with its line (`config.xml line 4: ...`). The file is left as it is until
you change something in Settings; that change writes a well-formed file from the last good
settings plus the change.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<clanky version="1">
  <post_ai>false</post_ai>
  <restore_layout>true</restore_layout>
  <startup_animation>true</startup_animation>
  <animations>true</animations>
  <notify_after_seconds>10</notify_after_seconds>
  <columns>80</columns>
  <rows>24</rows>
  <theme>system</theme>
  <theme_dark>graphite-dark</theme_dark>
  <theme_light>graphite-light</theme_light>
  <tab_style>horizontal</tab_style>
  <tools_panel>on</tools_panel>
  <tools><tool>files</tool><tool>recent</tool><tool>agents</tool></tools>
  <command_input>editor</command_input>
  <block_style>quiet</block_style>
  <status_line>show</status_line>
  <font_size>13</font_size>
  <font_family>Geist Mono</font_family>
  <ui_font_family>Geist</ui_font_family>
  <cell_width_scale>1</cell_width_scale>
  <cell_height_scale>1.25</cell_height_scale>
  <opacity>1</opacity>
  <colors>
    <accent>#3a7bd5</accent>
  </colors>
  <cursor_blink>system</cursor_blink>
  <cursor_blink_time_ms>1200</cursor_blink_time_ms>
  <cursor_blink_timeout_s>10</cursor_blink_timeout_s>
  <osc52_write>true</osc52_write>
  <osc52_read>false</osc52_read>
  <terminal_colors>
    <background>#111111</background>
    <foreground>#fafafa</foreground>
    <cursor>#fafafa</cursor>
    <ansi>
      <color>#2b2b2b</color>
      <color>#f0716a</color>
      <color>#4ec27a</color>
      <color>#e2b64f</color>
      <color>#5e9cf0</color>
      <color>#c586e8</color>
      <color>#3fc4c0</color>
      <color>#cccccc</color>
      <color>#7a7a7a</color>
      <color>#ff8d86</color>
      <color>#6fd894</color>
      <color>#f0cd6e</color>
      <color>#82b4ff</color>
      <color>#d8a2f5</color>
      <color>#62d6d2</color>
      <color>#f5f5f5</color>
    </ansi>
  </terminal_colors>
  <compose_dictionary>en_US</compose_dictionary>
  <compose_dictionary_dir>/usr/share/hunspell</compose_dictionary_dir>
  <compose_send_enter>false</compose_send_enter>
  <agent_pane_access>read+run</agent_pane_access>
  <agents>
    <agent><name>Claude Code</name><command>claude</command></agent>
    <agent><name>Codex</name><command>codex</command></agent>
    <agent><name>Pi</name><command>pi</command></agent>
  </agents>
  <history>true</history>
  <history_days>90</history_days>
  <history_output_kib>256</history_output_kib>
</clanky>
```

| Key | Type | Default | Allowed values / effect |
| --- | --- | --- | --- |
| `font_size` | number | 13 | Finite, 8–32 logical pixels inclusive |
| `font_size_pt` | number | 9.75 | Legacy: finite, 6–48 points inclusive; font_size wins when both exist |
| `theme` | string | `system` | `dark`, `light` or `system`; system follows desktop changes live, falling back to dark |
| `theme_dark` | string | `stone-dark` | Dark built-in ID or safe `user:<slug>` ID |
| `theme_light` | string | `stone-light` | Light built-in ID or safe `user:<slug>` ID |
| `post_ai` | boolean | false | Plain terminal: agents refused, `clanky mcp` exits 2; see [Post-AI mode](post-ai.md) |
| `startup_animation` | boolean | true | Show the robot greeting; `animations: false` also disables it |
| `colors` | object | none | Overrides the active theme's named tokens (below) with `#rrggbb` or `#rrggbbaa` strings, one element per token |
| `columns` | integer | 80 | 1–1000 inclusive |
| `rows` | integer | 24 | 1–1000 inclusive; columns × rows must not exceed 250,000 |
| `compose_dictionary` | string | `en_US` | 1–64 ASCII letters, digits, `_` or `-`; Hunspell dictionary basename |
| `compose_dictionary_dir` | string | `/usr/share/hunspell` | Absolute directory containing the language's `.aff` and `.dic` files |
| `compose_send_enter` | boolean | false | Append one CR after the compose bracketed paste when true |
| `osc52_write` | boolean | true | Allow programs to write up to 1 MiB of UTF-8 clipboard text |
| `osc52_read` | boolean | false | Explicitly allow clipboard reads, capped at 64 KiB |
| `notify_after_seconds` | integer | 10 | Nonnegative u64 seconds; 0 disables command-finished notifications |
| `restore_layout` | boolean | true | Save/restore local terminal layout and directories; existing server state wins |
| `opacity` | number | 1.0 | Finite, 0.5–1.0; window background alpha |
| `animations` | boolean | true | Enable M11 cursor, panel and scroll motion |
| `rails` | boolean | true | Legacy: read as `tools_panel` (`true` on, `false` off) when `tools_panel` is absent; Settings removes it the first time it writes `tools_panel` |
| `tools_panel` | string | `on` | `on` or `off`: the tools panel on the left at start; Ctrl+Shift+B shows or hides it for the session |
| `tools` | list | files, recent, agents | Distinct ids from `files`, `recent`, `agents`: the tools the panel offers, written in that order |
| `tab_style` | string | `horizontal` | `horizontal`: tabs in the title bar; `vertical`: a tab list down the left with folder, branch and last command |
| `command_input` | string | `editor` | `editor`: a command input at the bottom of each terminal pane, and the shell prints no prompt of its own; `classic`: keys go straight to the shell and it shows its own prompt. Applies at once; the shell changes at its next prompt. See [the terminal](terminal.md#command-input) |
| `block_style` | string | `quiet` | `quiet`: space and a hairline between commands, a meta row above each bold command (with the command input on), one hover bar; `cards`: a card around each command |
| `status_line` | string | `show` | `show` or `hide`: the status line along the bottom of the window |
| `font_family` | string | `Geist Mono` | Bundled terminal default; otherwise name an installed family |
| `ui_font_family` | string | `Geist` | Bundled UI default; otherwise name an installed family |
| `history` | boolean | true | Save every command to the [history file](history.md); false creates no file and writes nothing |
| `history_days` | integer | 90 | 0–3650; saved commands older than this are deleted at server start and daily; 0 keeps everything |
| `history_output_kib` | integer | 256 | 1–65536; masked output stored per command, cut on a UTF-8 boundary |
| `cell_width_scale` | number | 1.0 | 1.0–2.0; multiplies the cell width (VTE's cell-width-scale) |
| `cell_height_scale` | number | 1.0 | 1.0–2.0; multiplies the cell height, as Ptyxis's line height (VTE's cell-height-scale) |
| `cursor_blink` | string | `system` | `system` follows GNOME's cursor-blink, cursor-blink-time and cursor-blink-timeout (on, 1200, 10 when unreadable); `on` or `off` |
| `cursor_blink_time_ms` | integer | 1200 | 100–10000; one full on-and-off cycle, for `on` (and `system` without GNOME) |
| `cursor_blink_timeout_s` | integer | 10 | Seconds after the last key that blinking stops, with the cursor shown |
| `terminal_colors` | object | `{}` | `background`, `foreground`, `cursor`, `selection` (`#rrggbb`) and `ansi` (16 colours); the terminal's colours only, unset ones follow the theme; a colour scheme or `clanky import-colors` writes all 20; see [the terminal](terminal.md#colour-schemes) |
| `agents` | list | Claude Code `claude`, Codex `codex`, Pi `pi` | Up to 32 `<agent>` elements, each with a `<name>` and a `<command>`: nonempty text of at most 256 bytes; the tools the [agent picker](agent-pane.md) and `clanky agent` can start |
| `agent_pane_access` | string | `read+run` | `read+run` or `off`: agent access of a pane started for an agent |

Settings load at startup and apply live when the file changes (checked every
500 ms). Invalid edits retain the last good settings and show the error, with its line.
Settings writing a Layout value that is the default removes its element. `columns` and `rows`
size new windows; every other setting applies at once in the open window. Zoom changes remain
window-local and do not rewrite the file. Headless terminals use their socket size defaults. Font family names
must be nonempty, at most 256 UTF-8 bytes, and contain no control characters.
The default scene fonts are Geist Mono and Geist. JetBrains Mono and Inter remain
bundled and explicit font settings are preserved. Legacy M1/M2 offscreen
fixtures retain DejaVu Sans Mono. Legacy font_size_pt remains accepted without
clamping it to the new key's narrower range; 12 points equals 16 logical pixels.

Colour tokens are the theme's (see [the theme format](theme-format.md)); Settings › Appearance ›
Window colours names each one after its key:

| In Settings | Token | In Settings | Token |
| --- | --- | --- | --- |
| Sidebar | `sidebar` | Ring | `ring` |
| Background | `background` | Primary | `primary` |
| Card | `card` | Primary foreground | `primary_foreground` |
| Muted | `muted` | Sidebar accent | `sidebar_accent` |
| Foreground | `foreground` | Ok | `ok` |
| Muted foreground | `muted_foreground` | Err | `err` |
| Accent | `accent` | Agent | `agent` |
| Pressed | `pressed` | Robot paint | `robot_paint` |
| Border | `border` | Primary pressed | `primary_pressed` |
| Input | `input` | |  |

Values are `#rrggbb` or, with an alpha channel, `#rrggbbaa`; some shipped themes give `border` and `input`
an alpha (Graphite's `border` is `#ffffff1a`), and Settings shows those values as they are. The older tokens
`window`, `rail`, `panel`, `line`, `ink`, `dim`, `violet`, `teal`, `focus`, `ok_ink` and `err_ink` are still
read for files written before the themes. Unknown tokens and other values are errors. Overrides apply after
choosing the light/dark palette; dependent card fills and glows are recomputed.
For example: `<colors><card>#202020</card></colors>`.
Custom overrides can reduce contrast; the shipped palettes are the design defaults.

## Terminal colours

The scene resolves default colours and ANSI indices 0–15 from the active theme at
draw time, including existing cells when the theme switches. Default foreground is
`ink`, and default background shows the `panel`/command-card surface. Indexed colours
16–255 and explicit truecolour remain as supplied by the terminal application.
Legacy rendering and M1/M2 fixture RGB values are unchanged.

The dark 16-colour palette is unchanged. Its historical ANSI 7 foreground uses
`ink`; its indexed backgrounds retain their original values. The former RGB-match
shortcut is gone: truecolour (229,229,229) stays that exact colour, and explicit
truecolour black backgrounds are opaque rather than mistaken for defaults.

| Index | Dark ANSI RGB | Light ANSI RGB | Light contrast on white |
| --- | --- | --- | --- |
| 0 | #000000 | #1a2233 | 15.903:1 |
| 1 | #ff0000 | #c8303f | 5.316:1 |
| 2 | #00cd00 | #1d8048 | 4.960:1 |
| 3 | #cdcd00 | #b54f0b | 5.137:1 |
| 4 | #0000ee | #2f63d6 | 5.417:1 |
| 5 | #cd00cd | #6d4fd8 | 5.585:1 |
| 6 | #00cdcd | #0b7f78 | 4.860:1 |
| 7 | #e5e5e5 | #d6deea | 1.355:1 — white exception |
| 8 | #7f7f7f | #4f5c76 | 6.720:1 |
| 9 | #ff0000 | #a8222f | 7.155:1 |
| 10 | #00ff00 | #166a3b | 6.648:1 |
| 11 | #ffff00 | #994712 | 6.433:1 |
| 12 | #5c5cff | #2b57b9 | 6.632:1 |
| 13 | #ff00ff | #5e47ba | 6.804:1 |
| 14 | #00ffff | #0e6e6c | 6.060:1 |
| 15 | #ffffff | #f3f6fb | 1.083:1 — white exception |

Ratios use WCAG relative luminance: linearize each sRGB channel with the 0.04045
threshold, then L = 0.2126R + 0.7152G + 0.0722B; contrast on white is 1.05/(L+0.05).
White-ish indices 7 and 15 intentionally remain light, for applications choosing
white on a coloured background; they are exempt from the 4.5:1 foreground check.
The minimum among required default light colours is 4.860:1. Light colours derive
from the theme tokens; bright yellow/blue/magenta/cyan mix 18% ink into their normal
token. Custom token overrides can lower contrast.

## Editor colours

Editor panes colour code with tree-sitter for Rust (`.rs`), Python (`.py`, `.pyi`), TypeScript
(`.ts`, `.mts`, `.cts`), TSX (`.tsx`), JavaScript (`.js`, `.mjs`, `.cjs`, `.jsx`), Go (`.go`),
C (`.c`, `.h`), bash (`.sh`, `.bash`, `.bashrc`, `.profile`, or a `#!` line naming bash or sh),
JSON, TOML (and `Cargo.lock`), YAML (`.yaml`, `.yml`) and Markdown (`.md`). Other files stay
plain. The colours come from the window theme (`theme`, `theme_dark`, `theme_light`), never from
`terminal_colors`, so every theme has matching code colours and a theme switch recolours open
files at once:

| Token | Colour |
| --- | --- |
| keyword | ANSI 5 |
| function | ANSI 4 |
| type | ANSI 3 |
| string | ANSI 2 |
| number | 40% ANSI 1 + 60% ANSI 3 |
| constant, macro, attribute | ANSI 6 |
| comment | 51% of the way from `background` to `muted_foreground`, italic |
| everything else | `foreground` |

In Markdown a heading's text is keyword, inline and fenced code are string, and prose is plain.
A font without an italic face (the bundled Geist Mono has none) draws comments slanted from its
upright face. Each keystroke re-parses only what changed, and only the visible lines are
coloured. Files over 2 MiB, or that take longer than 50 ms to parse, stay plain. There is no
setting for this.

## Other settings

Personal spelling words go in `compose-words.txt` beside the selected config file,
or in the normal Clanky config directory. The file is created only on Add.
`compose_words` is an internal path, not a config key. Missing dictionaries leave
compose usable with a visible spelling error. Drafts are memory-only, at most
256 KiB UTF-8; large sends use ordered 64 KiB input pieces around one paste.

Layout metadata goes in the normal `$XDG_CONFIG_HOME/clanky/layout.json` (or HOME
fallback), even with an explicit config file. It stores directories, sizes and
layout/focus and split ratios, not commands, environment, output or editor drafts. Only a fresh
local server is restored; SSH attachments do not import local layout state.

Notifications need OSC 133 command blocks and a desktop notification helper.
A newly completed block must meet the threshold and have been unfocused at its
completion marker. Retained completions are not replayed on attachment; notices
contain exit status, not command text/output. See [getting started](getting-started.md).

OSC 52 is handled by attached owner windows using `wl-copy`/`wl-paste` or `xclip`;
headless servers do not touch a desktop clipboard. Only the clipboard selection
is supported. With several windows, the first to drain an event applies its policy.
