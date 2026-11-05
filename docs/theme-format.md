# Theme JSON format

A theme is a UTF-8 JSON file. All 19 colours and all 16 ANSI entries are required;
missing colours and unknown fields are errors. Colours are `#rrggbb` or `#rrggbbaa`.
The last two digits are opacity; `ff` is fully opaque. `appearance` is `dark` or `light`.

This complete example uses Stone's dark colours:

```json
{
  "name": "Workshop",
  "appearance": "dark",
  "colors": {
    "sidebar": "#141210", "background": "#1a1816", "card": "#201d1a",
    "muted": "#26221f", "foreground": "#ebe6df", "muted_foreground": "#a0978d",
    "accent": "#2b2724", "pressed": "#332e2a", "border": "#fff0e114",
    "input": "#fff0e11f", "ring": "#8a8178", "primary": "#ebe6df",
    "primary_foreground": "#1a1816", "primary_pressed": "#d6d0c8",
    "sidebar_accent": "#221f1c", "ok": "#93b89a", "err": "#d98b7e",
    "agent": "#d2a476", "robot_paint": "#6b5236"
  },
  "ansi": [
    "#a0978d", "#d98b7e", "#93b89a", "#d6b27c", "#9faeb8", "#c5a39b", "#95b8ae", "#ebe6df",
    "#b3aaa0", "#e8a194", "#a8c9ae", "#e4c596", "#b5c2ca", "#d8b8af", "#accdc3", "#f5f0e9"
  ],
  "window_gradient": null,
  "glow": false
}
```

- `window_gradient`: `null` or 2–8 evenly spaced colours, top to bottom.
- `glow`: whether pane glows are permitted.
- `description`: optional text for the picker.
- `ansi`: black, red, green, yellow, blue, magenta, cyan, white, then their bright variants.
  Explicit terminal truecolour is not replaced.
- `robot_paint`: the robot's body panel. Its lights use `agent`.
- `primary_pressed`: the fill of a pressed primary button; `pressed` is for other controls.

Config colour overrides may contain only the names you change; a theme file must be complete.
See [themes](themes.md) for file locations, built-in IDs, import/export and current integration limits.
