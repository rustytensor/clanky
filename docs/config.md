# Configuration

Clanky reads a JSON object from `$XDG_CONFIG_HOME/clanky/config.json`, or
`$HOME/.config/clanky/config.json` if XDG_CONFIG_HOME is unset or empty.
`clanky --config PATH` selects another file. A missing default file uses defaults;
a missing explicit file, invalid value or unknown key is an error.

| Key | Type | Default | Allowed values / effect |
| --- | --- | --- | --- |
| `font_size` | number | 16 | Finite, 8–32 logical pixels inclusive |
| `font_size_pt` | number | 12 | Legacy: finite, 6–48 points inclusive; font_size wins when both exist |
| `theme` | string | `system` | `dark`, `light` or `system`; system follows desktop changes live, falling back to dark |
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
| `opacity` | number | 0.94 | Finite, 0.5–1.0; M11 scene background alpha |
| `animations` | boolean | true | Enable M11 cursor, panel and scroll motion |
| `rails` | boolean | true | Initially show both Studio rails |
| `font_family` | string | `JetBrains Mono` | Bundled terminal default; otherwise name an installed family |
| `ui_font_family` | string | `Inter` | Bundled UI default; otherwise name an installed family |

```json
{"font_size_pt":12,"columns":80,"rows":24,"compose_dictionary":"en_US","compose_send_enter":false,"osc52_read":false,"notify_after_seconds":10,"restore_layout":true}
```

Settings are loaded at startup; zoom changes are window-local and do not rewrite
this file. Headless terminals use their socket size defaults. Font family names
must be nonempty, at most 256 UTF-8 bytes, and contain no control characters.
The bundled scene fonts are JetBrains Mono and Inter. Legacy M1/M2 offscreen
fixtures retain DejaVu Sans Mono. Legacy font_size_pt remains accepted without
clamping it to the new key's narrower range; 12 points equals 16 logical pixels.

Colour tokens: `window`, `rail`, `panel`, `line`, `ink`, `dim`, `accent`, `violet`,
`teal`, `ok`, `err`, `focus`, `ok_ink`, `err_ink`. Unknown tokens and values other
than six hexadecimal digits prefixed by `#` are errors. Overrides apply after
choosing the light/dark palette; dependent card fills and glows are recomputed.
For example: `{"theme":"light","colors":{"panel":"#202020"},"font_size":16}`.
Custom overrides can reduce contrast; the shipped palettes are the design defaults.

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
