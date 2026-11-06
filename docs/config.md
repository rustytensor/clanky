# Configuration

M12 preview: rebuilt Settings and live theme selection are available. Palette,
search, compose and editor entries remain hidden, including their opening shortcuts,
until their replacement screens are ready. Settings and hand edits use the same JSON.

Clanky reads a JSON object from `$XDG_CONFIG_HOME/clanky/config.json`, or
`$HOME/.config/clanky/config.json` if XDG_CONFIG_HOME is unset or empty.
`clanky --config PATH` selects another file. A missing default file uses defaults;
a missing explicit file, invalid value or unknown key is an error.

| Key | Type | Default | Allowed values / effect |
| --- | --- | --- | --- |
| `font_size` | number | 13 | Finite, 8–32 logical pixels inclusive |
| `font_size_pt` | number | 9.75 | Legacy: finite, 6–48 points inclusive; font_size wins when both exist |
| `theme` | string | `system` | `dark`, `light` or `system`; system follows desktop changes live, falling back to dark |
| `theme_dark` | string | `stone-dark` | Dark built-in ID or safe `user:<slug>` ID |
| `theme_light` | string | `stone-light` | Light built-in ID or safe `user:<slug>` ID |
| `post_ai` | boolean | false | Plain terminal: agents refused, `clanky mcp` exits 2; see [Post-AI mode](post-ai.md) |
| `startup_animation` | boolean | true | Show the robot greeting; `animations: false` also disables it |
| `colors` | object | `{}` | Overrides the active theme's named tokens with `#rrggbb` strings |
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
| `rails` | boolean | true | Initially show both Studio rails |
| `font_family` | string | `Geist Mono` | Bundled terminal default; otherwise name an installed family |
| `ui_font_family` | string | `Geist` | Bundled UI default; otherwise name an installed family |
| `history` | boolean | true | Save every command to the [history file](history.md); false creates no file and writes nothing |
| `history_days` | integer | 90 | 0–3650; saved commands older than this are deleted at server start and daily; 0 keeps everything |
| `history_output_kib` | integer | 256 | 1–65536; masked output stored per command, cut on a UTF-8 boundary |

```json
{"font_size_pt":12,"columns":80,"rows":24,"compose_dictionary":"en_US","compose_send_enter":false,"osc52_read":false,"notify_after_seconds":10,"restore_layout":true}
```

Settings load at startup and apply live when the file changes (checked every
500 ms). Invalid edits retain the last good settings and show the error once.
Ctrl+, or the top-bar Settings button opens the same file as a point-and-click
panel; see [Settings](settings.md). Zoom changes remain window-local and do not
rewrite the file. Headless terminals use their socket size defaults. Font family names
must be nonempty, at most 256 UTF-8 bytes, and contain no control characters.
The default scene fonts are Geist Mono and Geist. JetBrains Mono and Inter remain
bundled and explicit font settings are preserved. Legacy M1/M2 offscreen
fixtures retain DejaVu Sans Mono. Legacy font_size_pt remains accepted without
clamping it to the new key's narrower range; 12 points equals 16 logical pixels.

Colour tokens: `window`, `rail`, `panel`, `line`, `ink`, `dim`, `accent`, `violet`,
`teal`, `ok`, `err`, `focus`, `ok_ink`, `err_ink`. Unknown tokens and values other
than six hexadecimal digits prefixed by `#` are errors. Overrides apply after
choosing the light/dark palette; dependent card fills and glows are recomputed.
For example: `{"theme":"light","colors":{"panel":"#202020"},"font_size":16}`.
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
