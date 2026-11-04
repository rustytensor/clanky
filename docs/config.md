# Configuration

Clanky reads a JSON object from `$XDG_CONFIG_HOME/clanky/config.json`, or
`$HOME/.config/clanky/config.json` if XDG_CONFIG_HOME is unset or empty. Use
`clanky --config PATH` to choose a file. A missing default file uses defaults;
a missing explicit file, invalid JSON, or an unknown key is an error.

| Key | Type | Default | Allowed values |
| --- | --- | --- | --- |
| `font_size_pt` | number | 12 | Finite, 6 through 48 points inclusive |
| `columns` | integer | 80 | 1 through 1000 inclusive |
| `rows` | integer | 24 | 1 through 1000 inclusive |

Columns multiplied by rows must not exceed 250,000. For example:

```json
{"font_size_pt": 12, "columns": 80, "rows": 24}
```

Zoom changes are window-local and do not rewrite the file. Headless operation
uses its socket terminal-size defaults. See [shortcuts](shortcuts.md).
