# Settings: all five rebuilt pages are live

General, Appearance, Terminal, Agents and Keys use the new 780×600 frame.
Open it from the sidebar or Ctrl+comma. No legacy Settings fallback exists.

- General applies Post-AI server policy and hides agent UI. Pane A becomes
  (268,76,952,326), with 116 columns; normal mode returns it to 81 columns.
- Appearance selects themes, edits/resets colours, and saves/exports/imports through
  ThemeStore. Built-ins remain unchanged; exported/imported bytes are preserved.
- Terminal applies font, size, opacity, side columns and animations immediately.
- Agents displays masking and named pane permissions. Stop revokes access and
  blocks new agents. Resume does not silently restore pane permissions.
- Keys lists only available shortcuts in key chips. Agents and agent shortcuts
  disappear in Post-AI. Long pages support the wheel and Page Up/Down.

## Checks

All three private-window checks passed, once each:
- `clanky-settings-live-check.py`: 81→116→81 columns; font, size, opacity, columns
  and animation settings saved; shell input resumed after Settings closed.
- `clanky-appearance-live-check.py`: pinned Light/Graphite window pixels, colour
  (1,2,3), Reset, and byte-identical save→export→import.
- `clanky-access-live-check.py`: a real agent read after a pane grant, revocation,
  blocked creation, explicit resume, hidden Agents entry and scrolling Keys.

These scripts/logs are under `/srv/devops/build/`. Frame geometry and clipping
pixel checks passed after their planted faults failed. The release build and
documentation gate passed. Full regression was not run.

## Visual review

Native 2× crops: `/srv/devops/build/clanky-editor-m12-reference/`
`settings-{general,appearance,terminal,agents,keys}-live-2x.png` and
`settings-terminal-light-live-2x.png`. The approved browser reference is unchanged.
Keyboard rings are 2 pixels; ordinary mouse clicks do not show keyboard focus.

The lead still owns the 2× comparison and full regression. Unconverted palette,
search, compose and editor entries remain hidden in either mode.
