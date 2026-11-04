# clanky-vt

Forked from [alacritty_terminal](https://crates.io/crates/alacritty_terminal) 0.26.0, the terminal engine of
[Alacritty](https://github.com/alacritty/alacritty), by Christian Duerr, Joe Wilm and the Alacritty contributors.
Licensed under Apache-2.0 (LICENSE-APACHE). Upstream's CHANGELOG.md is kept as it was.

## Changes by the Clanky project

- Renamed the package to `clanky-vt` and the library to `clanky_vt`; `publish = false`.
- Depends on `clanky-vte` (our fork of vte) through the dependency name `vte`, so the source still says `vte::`.
- `tests/ref.rs` and one doc example in `src/term/mod.rs`: `alacritty_terminal::` renamed to `clanky_vt::`.

## Upstream fixes

Check Alacritty's releases every release or two and port relevant fixes here, noting each one in this file.
