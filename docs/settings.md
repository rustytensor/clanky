# Settings file API

`clanky::settings::Settings` is the panel and gate interface. It validates with
`Config::from_json`; there is no second settings store.

- `Settings::open(path: impl Into<PathBuf>) -> Result<Settings, String>`: read a file;
  a missing file means defaults, without creating it.
- `config(&self) -> &Config`, `path(&self) -> &Path`: last-good state and destination.
- `set(&mut self, key: &str, value: serde_json::Value) -> Result<&Config, String>`.
- `reset(&mut self, key: &str) -> Result<&Config, String>`: remove that override.
- `set_color(&mut self, token: &str, color: Option<&str>) -> Result<&Config, String>`:
  set a `#rrggbb` override; `None` resets that colour.
- `reload(&mut self) -> Result<bool, String>`: true for new valid disk content;
  invalid content or removal keeps the last good state and reports once per distinct edit.
- `import(&mut self, source: &Path) -> Result<&Config, String>`: validate and atomically
  replace the destination with the source's exact bytes.
- `export(&self, destination: &Path) -> Result<(), String>`: atomically write the
  last-good bytes. Callers must confirm replacing an existing export destination.

Panel edits preserve JSON key order, including colour key order. Unchanged values
are retained; whitespace is reformatted. Writes use a same-directory temporary
file, sync it, then rename. Existing permissions are retained; new Linux files
start at 0600. The library rejects symlinks, non-files and files over 1 MiB.
It re-reads before each edit and checks for intervening changes before rename;
this is not a cross-process transaction lock. Invalid values never modify the file.

Focused check: `cargo test --offline -p clanky --lib ordered_settings_round_trip -j 8`.
Use the shared build lock and build environment from the team brief. Its planted
fault is `CLANKY_SETTINGS_FAULT=1`; expected alpha 204 becomes 255 and fails.
## Window panel

Ctrl+, or the top-bar Settings button opens the panel. Appearance includes theme,
opacity/font-size sliders, font families, rails, animations and all 14 colour
pickers with RGB sliders, hex entry and per-colour reset. Scroll to see more rows.
Terminal contains geometry, compose, notification and layout settings. Agents / access
contains clipboard policies and current-pane access switches. Keys is a scrollable,
read-only view of the documented bindings.

Edits apply through the same Settings methods and reload the window immediately.
The app checks the selected file every 500 ms; an invalid edit retains last-good
settings and reports once. Compose dictionary changes retain every draft. Session
zoom survives unrelated settings changes; changing font_size resets it to that value.
Per-pane access switches call the existing owner permission API, not a persistent
blanket grant in JSON. Geometry changes request a new window size.

Text fields support typing, Backspace, Ctrl+A, Ctrl+V, IME, Enter to apply and
Escape to cancel. Import/export ask for an absolute path; replacing an existing export needs
an explicit Replace click. Open settings file uses the Linux desktop's file
association (xdg-open), creating defaults first when needed. All lookups stay in
the platform layer. Settings captures input above other panels; the agent-stop
shortcut remains active.

`SettingsPanel::draw` uses bundled metrics for headless callers;
`draw_with_measure` accepts the window renderer's configured-font measurement.
The focused `settings_mouse_keyboard_and_pixels` check drives actual form clicks
and keys, verifies writes/rejection/import/export confirmation, and checks rendered
panel/background pixels. `CLANKY_SETTINGS_UI_FAULT=1` changes expected alpha 204 to
255 and must fail. This check and the app binary build passed. Live-window IME,
clipboard, desktop file opening and visual acceptance still require owner review.
