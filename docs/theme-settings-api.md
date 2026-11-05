# Settings theme API

Module: `clanky::theme_store`. No UI or background threads; no new dependencies.

- `default_directory()` — returns `<config dir>/clanky/themes`, or None without a config location.
- `ThemeStore::open_default()` / `open(path)` — load user themes; missing folders stay absent until saving.
- `ThemeStore::reload()` — detect content changes, additions and deletions; return true when the input changes.
- `ThemeStore::issues()` — report invalid files while retaining each file's last-good theme.
- `ThemeStore::picker(appearance)` — list built-in/user IDs, names, descriptions, preview swatches, gradients and glow.
- `ThemeStore::get(id)` — return an independent, editable theme copy.
- `ThemeStore::save_new(slug, theme)` — save a custom copy and return its `user:<slug>` ID.
- `ThemeStore::export(id, path)` — export last-good bytes to a new file.
- `ThemeStore::import(path, slug)` — validate and copy exact source bytes into a new user file; return its ID.
- `ThemeStore::directory()` — return the user-theme folder.
- `validate_id(id)` — validate built-in IDs or safe `user:<slug>` syntax without requiring the user file yet.
- `Config::selected_theme_id(system_light)` — choose `theme_dark` or `theme_light` using the appearance preference.
- `Config::resolved_document(store, system_light)` — resolve the selected theme and apply semantic RGBA overrides.
- `Config::from_json(json)` — also accepts `theme_dark`, `theme_light` and boolean `post_ai` (default false).
- `StartupOverrides::parse_flag(arg)` — consume `--post-ai` when called from the CLI flag parser.
- `StartupOverrides::apply(config)` — keep the CLI override on after every config load/reload.
- `Settings::set(key, value)` / `set_color(token, value)` — persist those new fields and semantic colour overrides.

## Caller responsibilities and limits

Call reload from the existing settings timer (for example every 500 ms), not the draw loop.
Only refresh the picker/re-resolve the active theme when reload returns true. Show issues;
directory errors retain the previous snapshot and are returned as Err. Deletions remove
user entries; a selected deleted theme returns an error, so the app chooses and reports
its fallback. Invalid edits keep the last-good display and export bytes.

User IDs are `user:<filename stem>`; names are display labels, never filesystem paths.
Stems have 1–64 ASCII letters, digits, `_` or `-`, starting with a letter or digit.
User themes never shadow built-ins. Only `.json` files are loaded, at most 128, each at most
64 KiB. File reads refuse final symlinks and nonregular files on Linux. Filesystem ancestors
are trusted; this is not a sandbox against another process changing parent directories.

All writes are **new-file only**: an existing destination is refused, never replaced.
A private temporary sibling is fully written and synced, then hard-linked into place.
A save copies the caller's Theme; import/export preserve user-file bytes, including formatting.
The picker includes background/foreground/muted/agent and all 16 ANSI swatches.

**App integration remains with clanky-editor:** wire the CLI parser helper into main,
apply overrides after reloads, and enforce Post-AI at the server/MCP boundary. This API
parses the setting; it does not itself disable agents or claim the executable accepts the flag yet.
Legacy-only colour tokens remain handled by `Config::resolved_theme`; semantic tokens use
`resolved_document`. Built-in selections with the wrong appearance are refused in config;
user selections are checked at resolution time.
