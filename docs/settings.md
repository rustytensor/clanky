# Settings

Open Settings from the sidebar or with **Ctrl+,**. Escape closes it; when editing a
value, Escape cancels that edit first. Settings uses the same [JSON config file](config.md)
as hand editing.

- **Appearance:** fonts, size, opacity, sidebar visibility, animations and colour controls.
- **Terminal:** window size, compose spelling, notifications and layout restoration.
- **Agents / access:** clipboard policy and per-terminal agent permission.
- **Keys:** a read-only shortcut list.

**Still being connected for M12:** the new dark/light theme pickers, semantic colour pickers,
Save as new theme, Export theme, Import theme, and General's Post-AI switch. See
[themes](themes.md) and [Post-AI mode](post-ai.md) for their format and current limits.
Do not confuse whole-settings import/export below with theme import/export.

## Edit a value

Click a control or field. Text fields support typing, IME, Backspace, Ctrl+A and Ctrl+V.
Enter applies the value; Escape cancels. Scroll for more rows.

Valid edits save immediately. Invalid values leave the file unchanged. Hand edits are checked
every 500 ms; a bad file keeps the last good settings and shows an error. Panel writes keep
key order, but reformat whitespace. Zoom is window-local; a saved font-size change resets it.

## Move your settings

**Export** writes the current settings to a chosen absolute path. Replacing an existing
settings export needs an explicit **Replace** click. **Import** checks a file before replacing
your settings. **Open settings file** uses your desktop's file association.

These operations reject symlinks, non-files and files over 1 MiB. Per-terminal access
switches change the running server's permission, not a permanent blanket grant in the file.
Ctrl+Shift+Escape still revokes agents while Settings is open.
