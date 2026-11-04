<!-- Modified by the Clanky project; see FORK.md in this crate. -->
# clanky-vt

Forked from [alacritty_terminal](https://crates.io/crates/alacritty_terminal) 0.26.0, the terminal engine of
[Alacritty](https://github.com/alacritty/alacritty), by Christian Duerr, Joe Wilm and the Alacritty contributors.
Licensed under Apache-2.0 (LICENSE-APACHE). Upstream's CHANGELOG.md is kept as it was.

## Changes by the Clanky project

- Renamed the package to `clanky-vt` and the library to `clanky_vt`; `publish = false`.
- Depends on `clanky-vte` (our fork of vte) through the dependency name `vte`, so the source still says `vte::`.
- `tests/ref.rs` and one doc example in `src/term/mod.rs`: `alacritty_terminal::` renamed to `clanky_vt::`.

- `src/term/mod.rs`: bound OSC 52 stores to 1 MiB decoded (including a pre-decode encoded bound); ignore non-clipboard selections for stores and loads.
- `src/event.rs`: redact clipboard store values from Debug output.
- `FORK.md`: record these local modifications and retain the fork notice.

## Upstream fixes

Check Alacritty's releases every release or two and port relevant fixes here, noting each one in this file.
