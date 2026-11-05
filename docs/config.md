# Configuration

Clanky reads `$XDG_CONFIG_HOME/clanky/config.json`, or `$HOME/.config/clanky/config.json`
when XDG_CONFIG_HOME is empty or unset. `clanky --config PATH` chooses another file.
A missing default file uses defaults. A missing explicit file, invalid value or unknown key is an error.

**M12 integration note:** theme selection and Post-AI behaviour are still being connected.
The table records this branch's actual loader defaults, including its older fonts and opacity.
See [themes](themes.md) and [Post-AI mode](post-ai.md) before relying on those new settings.

| Key | Type | Default | Allowed values / effect |
| --- | --- | --- | --- |
| `font_size` | number | 16 | 8–32 logical pixels; wins over font_size_pt |
| `font_size_pt` | number | 12 | Legacy setting, 6–48 points |
| `font_family` | string | `JetBrains Mono` | Bundled or installed terminal font family |
| `ui_font_family` | string | `Inter` | Bundled or installed interface font family |
| `theme` | string | `system` | `dark`, `light` or `system`; system falls back to dark |
| `theme_dark` | string | `stone-dark` | Dark built-in ID or `user:<filename stem>` |
| `theme_light` | string | `stone-light` | Light built-in ID or `user:<filename stem>` |
| `colors` | object | `{}` | Named colour overrides, `#rrggbb` or `#rrggbbaa` |
| `opacity` | number | 0.94 | 0.5–1.0; 1 is opaque. Unsupported compositor transparency stays opaque |
| `animations` | boolean | true | Motion on/off; false also disables the startup robot |
| `startup_animation` | boolean | true | Show the robot at startup; independent of explicit `+hello` replay |
| `rails` | boolean | true | Initially show the sidebar and activity column |
| `post_ai` | boolean | false | Intended plain-terminal mode; enforcement is pending on this branch |
| `columns` | integer | 80 | 1–1000 |
| `rows` | integer | 24 | 1–1000; columns × rows must not exceed 250,000 |
| `compose_dictionary` | string | `en_US` | 1–64 ASCII letters, digits, `_` or `-`; Hunspell dictionary name |
| `compose_dictionary_dir` | string | `/usr/share/hunspell` | Absolute folder containing `.aff` and `.dic` files |
| `compose_send_enter` | boolean | false | Add one Enter after sending a compose draft |
| `osc52_write` | boolean | true | Let programs copy up to 1 MiB of UTF-8 clipboard text, masked before export |
| `osc52_read` | boolean | false | Allow clipboard replies, capped at 64 KiB and masked before delivery |
| `notify_after_seconds` | integer | 10 | Nonnegative seconds; 0 turns command-finished notices off |
| `restore_layout` | boolean | true | Restore folders and splits on a fresh local server, not old processes or output |

To request the M12 fonts, size and opaque finish explicitly:

```json
{"font_family":"Geist Mono","ui_font_family":"Geist","font_size":13,"opacity":1}
```

Geist, Geist Mono, JetBrains Mono and Inter are bundled. Font names must be nonempty,
at most 256 UTF-8 bytes and contain no control characters. Numbers must be finite.
12 points equals 16 logical pixels. Zoom changes only the current window, not this file.

Open [Settings](settings.md) with Ctrl+, or its sidebar item. The file is checked every
500 ms; invalid edits retain the last good settings and show an error. Panel edits save
to the same file, not a second store.

## Colours

The [theme JSON example](theme-format.md) lists all 19 M12 colour names, including
`robot_paint` and `primary_pressed`. Config overrides can name only the colours they change;
a theme file must supply them all. Changing an override does not edit a built-in theme.

Older `window`, `rail`, `panel`, `line`, `ink`, `dim`, `violet`, `teal`, `focus`,
`ok_ink` and `err_ink` overrides remain accepted for the older palette path. They do not
replace the corresponding M12 names; use the semantic names in new themes.

Default terminal colours and ANSI indices 0–15 come from the active palette on display.
Indices 16–255 and explicit truecolour stay as supplied by terminal programs. Explicit
truecolour black is not treated as a transparent default. Custom colours can reduce contrast.

## Stored files and helpers

- User themes: the normal config folder's `clanky/themes/`, even with `--config PATH`.
- Robot marker: `$XDG_STATE_HOME/clanky/first-meeting`, or
  `$HOME/.local/state/clanky/first-meeting`. See [the robot](robot.md).
- Layout: the normal config folder's `clanky/layout.json`. It stores directories, sizes,
  focus and split ratios, not commands, secrets, output or editor drafts. Existing server state wins.
- Personal spelling words: `compose-words.txt` beside the selected config file, created on Add.
  `compose_words` is an internal path, not a config key. Drafts stay in memory only.

Notifications need shell command markers (OSC 133) and a desktop helper. They report status,
not command text. OSC 52 uses `wl-copy`/`wl-paste` or `xclip` through an attached owner window;
headless servers do not access the desktop clipboard. With several windows, the first to drain
an event applies its clipboard policy. See [getting started](getting-started.md).
