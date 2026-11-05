# Settings rebuild: General, Appearance and Terminal

General, Appearance and Terminal are live in the rebuilt 780×600 Settings panel.
Open it from the sidebar or Ctrl+comma. Agents and Keys remain hidden until rebuilt;
there is no fallback to the legacy form.

General's Post-AI switch saves immediately and applies the merged server policy.
The window hides Agents, the right column, badges and access switches. Pane A uses
the existing checked layout: (268,76,952,326), with 116 columns at 13 px. Turning
Post-AI off restores the column and 81-column panes; revoked permissions stay revoked.

Terminal's font selector, size stepper, opacity slider, side-column switch and
animation switch save and apply. The selector offers the three bundled monospace
families. Escape/close dismisses the panel; the footer opens the settings file with
the system editor. Tab/Enter operate controls; arrow keys adjust a focused slider.
The window also draws the theme runtime's last-good active theme.

Check: `python3 /srv/devops/build/clanky-settings-live-check.py` passed in a private
Xvfb window. It clicked General on/off (81→116→81 columns), changed font and size,
set opacity to 75% then 100%, toggled columns and animations, checked saved JSON,
and confirmed that shell input resumed after closing Settings. The release build
passed. No full regression suite was run.

Screenshots under `/srv/devops/build/clanky-editor-m12-reference/`:
`post-ai-live.png` and `settings-terminal-live.png`.

Appearance uses ThemeStore for selection, save, export and import, with config
colour overrides and Reset. Its scrollable contents are clipped below the title
and above the footer. The clipping pixel check passed after a planted missing clip
failed. All binary/example callers compile with the added clip directive.

`python3 /srv/devops/build/clanky-appearance-live-check.py` passed: Light and Graphite
changed actual window pixels to the pinned colours; a sidebar edit produced
(1,2,3); Reset removed it; save/export/import produced identical theme bytes.
Screenshot: `settings-appearance-live.png` in the same comparison folder.

Next: Agents and Keys. Existing unrelated
palette, search, compose and editor entries remain hidden in this review build.
