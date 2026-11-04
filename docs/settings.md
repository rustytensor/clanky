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
  invalid content keeps the last good state and reports once per distinct edit.
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
`settings_panel::SettingsPanel` now contains the WIP form controls, colour picker,
sliders, import/export confirmation and mouse/keyboard handling. It compiles but is
not yet connected to the window: Ctrl+comma/top-bar entry, live reload/application,
agent-access callbacks and modal input routing remain to do. Visual review and UI
checks have not run. Settings work is paused for the lead's M11 polish list.
