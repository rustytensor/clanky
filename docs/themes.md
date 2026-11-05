# Themes

**Branch status:** all 13 built-ins, the JSON loader and user-theme storage are present.
The window's new theme pickers and Save/Export/Import buttons are not connected yet.
The settings below describe M12's format, not a promise that they already change the window.

## Choose a theme

`theme` chooses `dark`, `light` or `system`. `theme_dark` and `theme_light` name the
palette for each appearance; both default to Stone. Each picker accepts only matching
appearances. Config `colors` overrides individual theme colours.

| Theme | Appearance | Config ID |
| --- | --- | --- |
| Stone | Dark / light | `stone-dark`, `stone-light` |
| Paper | Dark / light | `paper-dark`, `paper-light` |
| Graphite | Dark / light | `graphite-dark`, `graphite-light` |
| Catppuccin Latte | Light | `catppuccin-latte` |
| Catppuccin Frappé | Dark | `catppuccin-frappe` |
| Catppuccin Macchiato | Dark | `catppuccin-macchiato` |
| Catppuccin Mocha | Dark | `catppuccin-mocha` |
| AI Bedazzled | Dark | `ai-bedazzled` |
| Unicorn Tears | Light | `unicorn-tears` |
| Unicorn Vomit | Dark | `unicorn-vomit` |

Stone is warm grey; Paper is warmer; Graphite is neutral. Catppuccin keeps its upstream
palette. The last three use gradients and glows: violet/blue, soft pastels and a rainbow.
Not every theme or custom colour has the same contrast.

```json
{"theme":"system","theme_dark":"catppuccin-mocha","theme_light":"paper-light"}
```

## Your own themes

Use `$XDG_CONFIG_HOME/clanky/themes/`, or `$HOME/.config/clanky/themes/` when
XDG_CONFIG_HOME is empty or unset. `--config PATH` does not move this folder.
`workshop.json` has the ID `user:workshop`; its display name comes from `name`.
Filename stems use 1–64 letters, digits, `-` or `_`, starting with a letter or digit.

The Appearance actions being connected are:
- **Save as new theme:** save a complete edited copy under a new filename. Built-ins never change.
- **Export theme:** write a theme to a new file for sharing. Existing files are not replaced.
- **Import theme:** validate a file and copy it into the themes folder under a new filename.

User import/export preserves file bytes. Invalid edits keep the last successfully loaded
version; deleted files leave the picker on reload. The store accepts up to 128 regular `.json`
files, each at most 64 KiB, and rejects symlinks. UI reload/error reporting is still pending.

See the [complete JSON example](theme-format.md), [configuration](config.md), or
[theme storage API](theme-settings-api.md) for developer details.
